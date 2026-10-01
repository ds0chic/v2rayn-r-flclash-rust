//! Xray routing generation (`V2rayRoutingService`).

use serde_json::{json, Value};

use crate::input::{CodegenProfile, ConfigType};
use crate::util::*;
use crate::xray::outbound::{build_all_proxy_outbounds, gen_balancer, gen_observatory};
use crate::xray::XrayState;
use crate::{CodegenError, Diagnostic};

/// `GenRouting`; mutates `state.config["routing"]` and may append outbounds.
pub(crate) fn build_routing(state: &mut XrayState<'_>) -> Result<(), CodegenError> {
    let input = state.input;
    if state.config.get("routing").is_none() {
        state.config.insert(
            "routing".into(),
            json!({"domainStrategy": AS_IS, "rules": []}),
        );
    }
    let strategy = input.settings.routing_basic.domain_strategy.clone();
    if let Some(routing) = state
        .config
        .get_mut("routing")
        .and_then(Value::as_object_mut)
    {
        routing.insert(
            "domainStrategy".into(),
            json!(if strategy.is_empty() {
                AS_IS
            } else {
                &strategy
            }),
        );
        if routing.get("rules").is_none() {
            routing.insert("rules".into(), json!([]));
        }
    }

    if input.settings.tun.enabled {
        append_tun_rules(state);
    }

    let override_strategy = input
        .routing
        .as_ref()
        .and_then(|r| r.domain_strategy.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);
    if let Some(strategy) = override_strategy {
        if let Some(routing) = state
            .config
            .get_mut("routing")
            .and_then(Value::as_object_mut)
        {
            routing.insert("domainStrategy".into(), json!(strategy));
        }
    }

    if let Some(active) = &input.routing {
        for rule in &active.rule_set {
            if !rule.enabled || rule.is_dns() {
                continue;
            }
            let copies = build_user_rule(state, rule)?;
            push_rules(state, copies);
        }
    }

    // balancer rewrite
    let balancer_tags: Vec<String> = state
        .config
        .get("routing")
        .and_then(|r| r.get("balancers"))
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|b| {
                    b.get("tag")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                })
                .collect()
        })
        .unwrap_or_default();
    if !balancer_tags.is_empty() {
        if let Some(rules) = state
            .config
            .get_mut("routing")
            .and_then(|r| r.get_mut("rules"))
            .and_then(Value::as_array_mut)
        {
            for rule in rules.iter_mut() {
                let outbound = rule
                    .get("outboundTag")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let candidate = format!("{outbound}{BALANCER_TAG_SUFFIX}");
                if balancer_tags.contains(&candidate) {
                    if let Some(map) = rule.as_object_mut() {
                        map.remove("outboundTag");
                        map.insert("balancerTag".into(), json!(candidate));
                    }
                }
            }
        }
    }

    Ok(())
}

fn push_rules(state: &mut XrayState<'_>, rules: Vec<Value>) {
    if let Some(list) = state
        .config
        .get_mut("routing")
        .and_then(|r| r.get_mut("rules"))
        .and_then(Value::as_array_mut)
    {
        list.extend(rules);
    }
}

fn append_tun_rules(state: &mut XrayState<'_>) {
    let input = state.input;
    let tun_rules = input.tun_rules.clone().unwrap_or_else(sample_tun_rules);
    if let Value::Array(list) = tun_rules {
        push_rules(state, list);
    }
    let direct_exe = &input.settings.protect_core_executables;
    if !direct_exe.is_empty() {
        push_rules(
            state,
            vec![
                json!({"port": "53", "process": direct_exe, "outboundTag": DNS_OUTBOUND_TAG}),
                json!({"process": direct_exe, "outboundTag": DIRECT_TAG}),
            ],
        );
    }
    push_rules(
        state,
        vec![json!({"inboundTag": ["tun"], "port": "53", "outboundTag": DNS_OUTBOUND_TAG})],
    );
}

/// `GenRoutingUserRule`; returns the rule copies to append.
pub(crate) fn build_user_rule(
    state: &mut XrayState<'_>,
    rule: &crate::input::CodegenRule,
) -> Result<Vec<Value>, CodegenError> {
    let outbound = gen_user_rule_outbound(state, &rule.outbound_tag)?;
    let mut base = obj();
    base.insert("type".into(), json!("field"));
    put_opt_str(&mut base, "port", non_empty_opt(rule.port.as_deref()));
    put_opt_str(&mut base, "network", non_empty_opt(rule.network.as_deref()));
    if let Some(protocol) = rule.protocol.as_ref().filter(|v| !v.is_empty()) {
        base.insert("protocol".into(), json!(protocol));
    }
    if let Some(inbound) = rule.inbound_tag.as_ref().filter(|v| !v.is_empty()) {
        base.insert("inboundTag".into(), json!(inbound));
    }
    base.insert("outboundTag".into(), json!(outbound));
    let mut copies: Vec<Value> = Vec::new();

    let mut has_domain_ip = false;
    if let Some(domains) = rule.domain.as_ref().filter(|v| !v.is_empty()) {
        let mut it = base.clone();
        let normalized: Vec<String> = domains
            .iter()
            .filter(|d| !d.starts_with('#'))
            .map(|d| d.replace(ROUTING_RULE_COMMA, ","))
            .collect();
        if !normalized.is_empty() {
            it.insert("domain".into(), json!(normalized));
            copies.push(Value::Object(it));
            has_domain_ip = true;
        }
    }
    if let Some(ips) = rule.ip.as_ref().filter(|v| !v.is_empty()) {
        let mut it = base.clone();
        it.insert("ip".into(), json!(ips));
        copies.push(Value::Object(it));
        has_domain_ip = true;
    }
    if let Some(process) = rule.process.as_ref().filter(|v| !v.is_empty()) {
        let mut it = base.clone();
        it.insert("process".into(), json!(process));
        copies.push(Value::Object(it));
        has_domain_ip = true;
    }
    if !has_domain_ip {
        let has_plain = non_empty_opt(rule.port.as_deref()).is_some()
            || rule.protocol.as_ref().is_some_and(|v| !v.is_empty())
            || rule.inbound_tag.as_ref().is_some_and(|v| !v.is_empty())
            || non_empty_opt(rule.network.as_deref()).is_some();
        if has_plain {
            copies.push(Value::Object(base));
        }
    }
    Ok(copies)
}

fn non_empty_opt(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|s| !s.is_empty())
}

/// `GenRoutingUserRuleOutbound`, may generate referenced outbounds.
fn gen_user_rule_outbound(
    state: &mut XrayState<'_>,
    outbound_tag: &str,
) -> Result<String, CodegenError> {
    let tag = if outbound_tag.is_empty() {
        PROXY_TAG
    } else {
        outbound_tag
    };
    if [PROXY_TAG, DIRECT_TAG, BLOCK_TAG].contains(&tag) {
        return Ok(tag.to_string());
    }
    let node: Option<CodegenProfile> = state
        .input
        .profiles
        .values()
        .find(|p| p.remarks == tag)
        .cloned();
    let Some(node) = node else {
        warn_dangling(state, tag, "no profile matches the remarks");
        return Ok(PROXY_TAG.to_string());
    };
    if !xray_supported_type(node.config_type)
        && !node.config_type.is_group()
        && node.config_type != ConfigType::Outbound
    {
        warn_dangling(
            state,
            tag,
            &format!(
                "config type {:?} is not in Global.XraySupportConfigType",
                node.config_type
            ),
        );
        return Ok(PROXY_TAG.to_string());
    }
    let generated_tag = format!("{}-{PROXY_TAG}-{}", node.index_id, node.remarks);
    if state.outbounds.iter().any(|o| {
        o.get("tag")
            .and_then(Value::as_str)
            .is_some_and(|existing| existing.starts_with(&generated_tag))
    }) {
        return Ok(generated_tag);
    }
    let built = build_all_proxy_outbounds(state.input, &node, &generated_tag)?;
    let count = built.outbounds.len();
    state.custom_tags.extend(built.custom_tags);
    state.outbounds.extend(built.outbounds);
    if count > 1 {
        let multiple_load = node
            .proto_extra
            .multiple_load
            .unwrap_or(crate::input::MultipleLoad::LeastPing);
        gen_observatory(multiple_load, &generated_tag, state);
        gen_balancer(multiple_load, &generated_tag, state);
    }
    Ok(generated_tag)
}

fn xray_supported_type(config_type: ConfigType) -> bool {
    matches!(
        config_type,
        ConfigType::Vmess
            | ConfigType::Vless
            | ConfigType::Shadowsocks
            | ConfigType::Trojan
            | ConfigType::Hysteria2
            | ConfigType::WireGuard
            | ConfigType::Socks
            | ConfigType::Http
    )
}

/// Issue T06b/ISSUE-04: an unresolved routing reference still falls back to
/// `Global.ProxyTag` (upstream behavior) but must not do so silently.
fn warn_dangling(state: &mut XrayState<'_>, tag: &str, reason: &str) {
    state.diagnostics.push(Diagnostic::warning(
        "routing_dangling_reference",
        format!(
            "routing rule references outbound '{tag}' that cannot be resolved ({reason}); \
             falling back to '{PROXY_TAG}'"
        ),
        Some("routing.ruleSet[].outboundTag"),
    ));
}

/// `BuildFinalRule`.
pub(crate) fn build_final_rule(routing: &Value) -> Value {
    let balancer_tag = routing
        .get("balancers")
        .and_then(Value::as_array)
        .and_then(|list| {
            list.iter().find(|b| {
                b.get("tag").and_then(Value::as_str)
                    == Some(&format!("{PROXY_TAG}{BALANCER_TAG_SUFFIX}"))
            })
        })
        .and_then(|b| b.get("tag").and_then(Value::as_str))
        .map(ToString::to_string);
    let domain_strategy = routing
        .get("domainStrategy")
        .and_then(Value::as_str)
        .unwrap_or(AS_IS);

    let mut rule = obj();
    rule.insert("type".into(), json!("field"));
    rule.insert("network".into(), json!("tcp,udp"));
    rule.insert("outboundTag".into(), json!(PROXY_TAG));
    if let Some(balancer) = balancer_tag {
        rule.remove("outboundTag");
        rule.insert("balancerTag".into(), json!(balancer));
    }
    if domain_strategy == IP_IF_NON_MATCH {
        rule.remove("network");
        rule.insert("ip".into(), json!(["0.0.0.0/0", "::/0"]));
    }
    Value::Object(rule)
}

/// `BuildFinalRule` outbound/balancer tag pair (used by DNS routing).
pub(crate) fn final_rule_tags(routing: &Value) -> (Option<String>, Option<String>) {
    let rule = build_final_rule(routing);
    (
        rule.get("outboundTag")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        rule.get("balancerTag")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    )
}
