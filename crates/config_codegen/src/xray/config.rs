//! Xray orchestration (`CoreConfigV2rayService` + `V2rayConfigTemplateService`).

use serde_json::{json, Map, Value};

use crate::input::{CodegenInput, CodegenSettings, MultipleLoad};
use crate::util::*;
use crate::xray::outbound::{
    build_all_proxy_outbounds, diagnostic_sink_reset, diagnostic_sink_take, gen_balancer,
    gen_observatory, outbound_dialer_proxy,
};
use crate::xray::{dns, inbound, log, routing, stat, XrayState};
use crate::{CodegenError, GeneratedConfigs};

pub(crate) fn build(input: &CodegenInput) -> Result<GeneratedConfigs, CodegenError> {
    let mut state = XrayState {
        input,
        config: obj(),
        outbounds: Vec::new(),
        custom_tags: Vec::new(),
        diagnostics: Vec::new(),
    };
    state.config = obj();
    diagnostic_sink_reset();

    // Base skeleton (SampleClientConfig + GenLog/GenInbounds).
    state.config.insert("log".into(), log::build_log(input));
    state.config.insert(
        "inbounds".into(),
        Value::Array(inbound::build_inbounds(input)),
    );
    let mut routing_map = obj();
    routing_map.insert(
        "domainStrategy".into(),
        json!(if input.settings.routing_basic.domain_strategy.is_empty() {
            AS_IS
        } else {
            &input.settings.routing_basic.domain_strategy
        }),
    );
    routing_map.insert("rules".into(), json!([]));
    state
        .config
        .insert("routing".into(), Value::Object(routing_map));

    // GenOutbounds: proxies first, then the sample freedom/blackhole pair.
    let built = build_all_proxy_outbounds(input, &input.profile, PROXY_TAG)?;
    state.custom_tags = built.custom_tags;
    state.outbounds = built.outbounds;
    state.diagnostics.extend(diagnostic_sink_take());
    let proxy_count = state
        .outbounds
        .iter()
        .filter(|o| {
            o.get("tag")
                .and_then(Value::as_str)
                .is_some_and(|t| t.starts_with(PROXY_TAG))
        })
        .count();
    if proxy_count > 1 {
        let multiple_load = input
            .profile
            .proto_extra
            .multiple_load
            .unwrap_or(MultipleLoad::LeastPing);
        gen_observatory(multiple_load, PROXY_TAG, &mut state);
        gen_balancer(multiple_load, PROXY_TAG, &mut state);
    }
    state
        .outbounds
        .push(json!({"protocol": "freedom", "tag": DIRECT_TAG}));
    state
        .outbounds
        .push(json!({"protocol": "blackhole", "tag": BLOCK_TAG}));
    if input.settings.tun.enabled {
        state
            .outbounds
            .push(json!({"tag": DNS_OUTBOUND_TAG, "protocol": "dns"}));
    }

    // GenRouting (may append outbounds/balancers).
    routing::build_routing(&mut state)?;

    // GenDns (mutates dns/fakedns and outbound strategies).
    dns::build_dns(&mut state)?;

    // GenStatistic.
    let (stats, metrics, policy) = stat::build_statistic(input);
    if let Some(stats) = stats {
        state.config.insert("stats".into(), stats);
    }
    if let Some(metrics) = metrics {
        state.config.insert("metrics".into(), metrics);
    }
    if let Some(policy) = policy {
        state.config.insert("policy".into(), policy);
    }

    // Fragment passes.
    if input.settings.core_basic.enable_fragment {
        apply_outbound_fragment(&mut state);
    }
    if input.settings.core_basic.enable_final_fragment {
        apply_final_fragment(&mut state);
    }

    // Final rule (only when a proxy balancer exists).
    let routing_value = state
        .config
        .get("routing")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let final_rule = routing::build_final_rule(&routing_value);
    if final_rule.get("balancerTag").is_some() {
        if let Some(rules) = state
            .config
            .get_mut("routing")
            .and_then(|r| r.get_mut("rules"))
            .and_then(Value::as_array_mut)
        {
            rules.push(final_rule);
        }
    }

    // ApplyFinalConfigModifiers.
    apply_outbound_bind_interface(&mut state.outbounds, &input.settings);
    apply_outbound_send_through(&mut state.outbounds, &input.settings);

    let mut main_map = state.config;
    main_map.insert("outbounds".into(), Value::Array(state.outbounds));
    if let Some(fakedns) = take(&mut main_map, "fakedns") {
        main_map.insert("fakedns".into(), fakedns);
    }
    let mut main = Value::Object(main_map);
    apply_custom_outbound_replace(&mut main, input, &state.custom_tags)?;
    if let Some(template) = &input.template {
        full_config_template(&mut main, input, template)?;
    }
    Ok(GeneratedConfigs::new(main, state.diagnostics))
}

fn take(map: &mut Map<String, Value>, key: &str) -> Option<Value> {
    map.remove(key)
}

fn apply_outbound_fragment(state: &mut XrayState<'_>) {
    let fragment_mask = build_fragments_mask(state.input);
    for outbound in state.outbounds.iter_mut() {
        let has_security = outbound
            .get("streamSettings")
            .and_then(|s| s.get("security"))
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty());
        if !has_security {
            continue;
        }
        if outbound_dialer_proxy(outbound).is_some() {
            continue;
        }
        let Some(map) = outbound.as_object_mut() else {
            continue;
        };
        let stream = map
            .entry("streamSettings")
            .or_insert_with(|| Value::Object(obj()));
        let mut finalmask = stream
            .get("finalmask")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_else(obj);
        let tcp_empty = finalmask
            .get("tcp")
            .and_then(Value::as_array)
            .map(|a| a.is_empty())
            .unwrap_or(true);
        if tcp_empty {
            finalmask.insert("tcp".into(), json!([fragment_mask]));
        }
        let Some(stream_map) = stream.as_object_mut() else {
            continue;
        };
        stream_map.insert("finalmask".into(), Value::Object(finalmask));
    }
}

fn apply_final_fragment(state: &mut XrayState<'_>) {
    let fragment_mask = build_fragments_mask(state.input);
    let indices: Vec<usize> = state
        .outbounds
        .iter()
        .enumerate()
        .filter(|(_, o)| {
            o.get("tag")
                .and_then(Value::as_str)
                .is_some_and(|t| t.starts_with(PROXY_TAG))
        })
        .map(|(i, _)| i)
        .collect();
    for (offset, index) in indices.into_iter().enumerate() {
        let index = index + offset;
        let original_tag = state.outbounds[index]
            .get("tag")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let after_tag = format!("fragment-{original_tag}");
        let mut clone = json!({
            "tag": original_tag,
            "protocol": "freedom",
            "streamSettings": {
                "finalmask": {"tcp": [fragment_mask]}
            }
        });
        if let Some(map) = clone.as_object_mut() {
            let stream = map
                .entry("streamSettings")
                .or_insert_with(|| Value::Object(obj()));
            if let Some(stream_map) = stream.as_object_mut() {
                let sockopt = stream_map
                    .entry("sockopt")
                    .or_insert_with(|| Value::Object(obj()));
                if let Some(sockopt_map) = sockopt.as_object_mut() {
                    sockopt_map.insert("dialerProxy".into(), json!(after_tag));
                }
            }
        }
        if let Some(map) = state.outbounds[index].as_object_mut() {
            map.insert("tag".into(), json!(after_tag));
        }
        state.outbounds.insert(index, clone);
    }
}

fn build_fragments_mask(input: &CodegenInput) -> Value {
    fragment_mask(&input.settings.fragment4_ray)
}

pub(crate) fn fragment_mask(item: &crate::input::Fragment4Ray) -> Value {
    let packets = item
        .packets
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "tlshello".into());
    let lengths = if item.lengths.is_empty() {
        vec!["50-100".to_string()]
    } else {
        item.lengths.clone()
    };
    let delays = if item.delays.is_empty() {
        vec!["10-20".to_string()]
    } else {
        item.delays.clone()
    };
    let max_split_text = item
        .max_split
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "0".into());
    let max_split = max_split_text
        .split('-')
        .next()
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    json!({
        "type": "fragment",
        "settings": {
            "packets": packets,
            "lengths": lengths,
            "delays": delays,
            "maxSplit": max_split,
            "length": lengths.first().cloned().unwrap_or_default(),
            "delay": delays.first().cloned().unwrap_or_default()
        }
    })
}

fn should_bind_net(outbound: &Value) -> bool {
    let protocol = outbound
        .get("protocol")
        .and_then(Value::as_str)
        .unwrap_or("");
    if ["freedom", "blackhole", "dns", "loopback"].contains(&protocol) {
        return false;
    }
    let has_dialer = outbound
        .get("streamSettings")
        .and_then(|s| s.get("sockopt"))
        .and_then(|s| s.get("dialerProxy"))
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty());
    if has_dialer {
        return false;
    }
    let address = outbound
        .get("settings")
        .and_then(|s| s.get("address"))
        .and_then(Value::as_str)
        .or_else(|| {
            outbound
                .get("settings")
                .and_then(|s| s.get("peers"))
                .and_then(Value::as_array)
                .and_then(|p| p.first())
                .and_then(|p| p.get("endpoint"))
                .and_then(Value::as_str)
        })
        .unwrap_or("");
    if address.eq_ignore_ascii_case("localhost") {
        return false;
    }
    !is_loopback_address(address)
}

pub(crate) fn apply_outbound_bind_interface(outbounds: &mut [Value], settings: &CodegenSettings) {
    let bind_interface = settings
        .core_basic
        .bind_interface
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    if bind_interface.is_empty() {
        return;
    }
    for outbound in outbounds.iter_mut() {
        if !should_bind_net(outbound) {
            continue;
        }
        let Some(map) = outbound.as_object_mut() else {
            continue;
        };
        let stream = map
            .entry("streamSettings")
            .or_insert_with(|| Value::Object(obj()));
        let Some(stream_map) = stream.as_object_mut() else {
            continue;
        };
        let sockopt = stream_map
            .entry("sockopt")
            .or_insert_with(|| Value::Object(obj()));
        if let Some(sockopt_map) = sockopt.as_object_mut() {
            sockopt_map.insert("interface".into(), json!(bind_interface));
        }
        if let Some(extra) = stream_map
            .get_mut("xhttpSettings")
            .and_then(|s| s.get_mut("extra"))
            .and_then(Value::as_object_mut)
        {
            if let Some(download) = extra
                .get_mut("downloadSettings")
                .and_then(Value::as_object_mut)
            {
                let sockopt = download
                    .entry("sockopt")
                    .or_insert_with(|| Value::Object(obj()));
                if let Some(sockopt_map) = sockopt.as_object_mut() {
                    sockopt_map.insert("interface".into(), json!(bind_interface));
                }
            }
        }
    }
}

pub(crate) fn apply_outbound_send_through(outbounds: &mut [Value], settings: &CodegenSettings) {
    let send_through = settings
        .core_basic
        .send_through
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    if send_through.is_empty() {
        return;
    }
    for outbound in outbounds.iter_mut() {
        let should = should_bind_net(outbound);
        if let Some(map) = outbound.as_object_mut() {
            if should {
                map.insert("sendThrough".into(), json!(send_through));
            } else {
                map.remove("sendThrough");
            }
        }
    }
}

fn apply_custom_outbound_replace(
    main: &mut Value,
    input: &CodegenInput,
    custom_tags: &[(String, String)],
) -> Result<(), CodegenError> {
    if custom_tags.is_empty() {
        return Ok(());
    }
    let Some(outbounds) = main.get_mut("outbounds").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    for (tag, index_id) in custom_tags {
        let Some(position) = outbounds
            .iter()
            .position(|o| o.get("tag").and_then(Value::as_str) == Some(tag.as_str()))
        else {
            continue;
        };
        let placeholder = outbounds[position].clone();
        let detour = outbound_dialer_proxy(&placeholder).unwrap_or_default();
        let interface = placeholder
            .get("streamSettings")
            .and_then(|s| s.get("sockopt"))
            .and_then(|s| s.get("interface"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let content = input.custom_outbound_content.get(index_id).ok_or_else(|| {
            CodegenError::custom_outbound_missing(
                format!("no custom outbound content for index {index_id}"),
                "customOutboundContent",
            )
        })?;
        let contain_tag = content.contains(TAG_PLACEHOLDER);
        let contain_detour = content.contains(DETOUR_PLACEHOLDER);
        let contain_interface = content.contains(INTERFACE_PLACEHOLDER);
        let replaced = content
            .replace(TAG_PLACEHOLDER, tag)
            .replace(DETOUR_PLACEHOLDER, &detour)
            .replace(INTERFACE_PLACEHOLDER, &interface);
        let mut custom: Value = serde_json::from_str(&replaced).map_err(|error| {
            CodegenError::new(
                "custom_outbound_invalid",
                format!("custom outbound content is not valid JSON: {error}"),
                Some("customOutboundContent"),
            )
        })?;
        let Some(custom_map) = custom.as_object_mut() else {
            return Err(CodegenError::new(
                "custom_outbound_invalid",
                "custom outbound content must be a JSON object",
                Some("customOutboundContent"),
            ));
        };
        if !contain_tag {
            custom_map.insert("tag".into(), json!(tag));
        }
        if !contain_detour && !detour.is_empty() {
            ensure_stream_sockopt(custom_map, "dialerProxy", &detour);
        } else if detour.is_empty() {
            if let Some(sockopt) = custom_map
                .get_mut("streamSettings")
                .and_then(|s| s.get_mut("sockopt"))
                .and_then(Value::as_object_mut)
            {
                sockopt.remove("dialerProxy");
            }
        }
        if !contain_interface && !interface.is_empty() {
            ensure_stream_sockopt(custom_map, "interface", &interface);
        }
        outbounds[position] = custom;
    }
    Ok(())
}

fn ensure_stream_sockopt(map: &mut Map<String, Value>, key: &str, value: &str) {
    let stream = map
        .entry("streamSettings")
        .or_insert_with(|| Value::Object(obj()));
    let Some(stream_map) = stream.as_object_mut() else {
        return;
    };
    let sockopt = stream_map
        .entry("sockopt")
        .or_insert_with(|| Value::Object(obj()));
    if let Some(sockopt_map) = sockopt.as_object_mut() {
        sockopt_map.insert(key.into(), json!(value));
    }
    if let Some(extra) = stream_map
        .get_mut("xhttpSettings")
        .and_then(|s| s.get_mut("extra"))
        .and_then(Value::as_object_mut)
    {
        if let Some(download) = extra
            .get_mut("downloadSettings")
            .and_then(Value::as_object_mut)
        {
            let sockopt = download
                .entry("sockopt")
                .or_insert_with(|| Value::Object(obj()));
            if let Some(sockopt_map) = sockopt.as_object_mut() {
                sockopt_map.insert(key.into(), json!(value));
            }
        }
    }
}

pub(crate) fn full_config_template(
    main: &mut Value,
    input: &CodegenInput,
    template: &crate::input::CodegenTemplate,
) -> Result<(), CodegenError> {
    if !template.enabled {
        return Ok(());
    }
    let text = if input.settings.tun.enabled {
        template.tun_config.as_ref()
    } else {
        template.config.as_ref()
    };
    let Some(text) = text.filter(|t| !t.is_empty()) else {
        return Ok(());
    };
    let mut template_value: Value = serde_json::from_str(text).map_err(|error| {
        CodegenError::new(
            "invalid_template",
            format!("template is not valid JSON: {error}"),
            Some("template"),
        )
    })?;
    let Some(template_map) = template_value.as_object_mut() else {
        return Err(CodegenError::new(
            "invalid_template",
            "template must be a JSON object",
            Some("template"),
        ));
    };

    // balancer rewrite + append
    let generated_balancers = main
        .get("routing")
        .and_then(|r| r.get("balancers"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !generated_balancers.is_empty() {
        let proxy_balancer = generated_balancers.iter().find(|b| {
            b.get("tag").and_then(Value::as_str)
                == Some(&format!("{PROXY_TAG}{BALANCER_TAG_SUFFIX}"))
        });
        if let Some(balancer) = proxy_balancer {
            let balancer_tag = balancer.get("tag").cloned().unwrap_or(Value::Null);
            if let Some(rules) = template_map
                .get_mut("routing")
                .and_then(|r| r.get_mut("rules"))
                .and_then(Value::as_array_mut)
            {
                for rule in rules.iter_mut() {
                    if rule.get("outboundTag").and_then(Value::as_str) == Some(PROXY_TAG) {
                        if let Some(rule_map) = rule.as_object_mut() {
                            rule_map.remove("outboundTag");
                            rule_map.insert("balancerTag".into(), balancer_tag.clone());
                        }
                    }
                }
            }
        }
        let template_routing = template_map
            .entry("routing")
            .or_insert_with(|| Value::Object(obj()));
        if let Some(routing_map) = template_routing.as_object_mut() {
            let existing = routing_map.entry("balancers").or_insert_with(|| json!([]));
            if let Some(list) = existing.as_array_mut() {
                list.extend(generated_balancers);
            }
        }
    }

    // observatory merge
    if let Some(generated) = main.get("observatory").cloned() {
        merge_selector(template_map, "observatory", "subjectSelector", &generated);
    }
    if let Some(generated) = main.get("burstObservatory").cloned() {
        merge_selector(
            template_map,
            "burstObservatory",
            "subjectSelector",
            &generated,
        );
    }

    // outbounds merge
    let generated_outbounds = main
        .get("outbounds")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut merged: Vec<Value> = Vec::new();
    for mut outbound in generated_outbounds {
        let protocol = outbound
            .get("protocol")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase();
        if matches!(protocol.as_str(), "blackhole" | "dns" | "freedom") && template.add_proxy_only {
            continue;
        } else if let Some(detour) = template.proxy_detour.as_deref().filter(|s| !s.is_empty()) {
            let has_dialer = outbound_dialer_proxy(&outbound).is_some();
            if !has_dialer {
                let address = outbound_address(&outbound);
                if !is_private_network(&address) {
                    if let Some(map) = outbound.as_object_mut() {
                        ensure_stream_sockopt(map, "dialerProxy", detour);
                    }
                }
            }
        }
        merged.push(outbound);
    }
    if let Some(template_outbounds) = template_map.get("outbounds").and_then(Value::as_array) {
        merged.extend(template_outbounds.iter().cloned());
    }
    template_map.insert("outbounds".into(), Value::Array(merged));

    *main = template_value;
    Ok(())
}

fn merge_selector(
    template_map: &mut Map<String, Value>,
    key: &str,
    selector: &str,
    generated: &Value,
) {
    let Some(generated_list) = generated.get(selector).and_then(Value::as_array) else {
        return;
    };
    match template_map.get_mut(key) {
        None => {
            template_map.insert(key.into(), generated.clone());
        }
        Some(existing) => {
            if let Some(list) = existing.get_mut(selector).and_then(Value::as_array_mut) {
                let mut values: Vec<String> = generated_list
                    .iter()
                    .filter_map(|v| v.as_str().map(ToString::to_string))
                    .collect();
                values.extend(
                    list.iter()
                        .filter_map(|v| v.as_str().map(ToString::to_string)),
                );
                let mut dedup: Vec<String> = Vec::new();
                for value in values {
                    if !dedup.contains(&value) {
                        dedup.push(value);
                    }
                }
                *list = dedup.into_iter().map(Value::String).collect();
            }
        }
    }
}

pub(crate) fn outbound_address(outbound: &Value) -> String {
    let settings = outbound.get("settings");
    settings
        .and_then(|s| s.get("address"))
        .and_then(Value::as_str)
        .or_else(|| {
            settings
                .and_then(|s| s.get("servers"))
                .and_then(Value::as_array)
                .and_then(|s| s.first())
                .and_then(|s| s.get("address"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            settings
                .and_then(|s| s.get("vnext"))
                .and_then(Value::as_array)
                .and_then(|s| s.first())
                .and_then(|s| s.get("address"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            settings
                .and_then(|s| s.get("peers"))
                .and_then(Value::as_array)
                .and_then(|s| s.first())
                .and_then(|s| s.get("endpoint"))
                .and_then(Value::as_str)
        })
        .unwrap_or("")
        .to_string()
}
