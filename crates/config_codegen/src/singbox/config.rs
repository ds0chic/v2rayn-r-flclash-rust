//! sing-box orchestration (`CoreConfigSingboxService` + `SingboxConfigTemplateService`).

use serde_json::{json, Map, Value};

use crate::input::CodegenInput;
use crate::singbox::outbound::{build_all_proxy_outbounds, outbound_detour};
use crate::singbox::{dns, inbound, log, routing, ruleset, stat, SboxState};
use crate::util::*;
use crate::{CodegenError, GeneratedConfigs};

pub(crate) fn build(input: &CodegenInput) -> Result<GeneratedConfigs, CodegenError> {
    let mut state = SboxState {
        input,
        config: obj(),
        outbounds: Vec::new(),
        endpoints: Vec::new(),
        custom_tags: Vec::new(),
        diagnostics: Vec::new(),
    };

    state.config.insert("log".into(), log::build_log(input));
    state.config.insert(
        "inbounds".into(),
        Value::Array(inbound::build_inbounds(input)),
    );
    state.config.insert("route".into(), json!({"rules": []}));

    // GenOutbounds: proxy servers prepended in front of the sample `direct`.
    let built = build_all_proxy_outbounds(input, &input.profile, PROXY_TAG)?;
    let mut outbounds = vec![json!({"type": "direct", "tag": DIRECT_TAG})];
    outbounds.splice(0..0, built.outbounds);
    state.outbounds = outbounds;
    state.endpoints = built.endpoints;
    state.custom_tags = built.custom_tags;

    routing::build_routing(&mut state)?;
    dns::build_dns(&mut state)?;
    state
        .config
        .insert("experimental".into(), stat::build_experimental(input));
    ruleset::convert_geo2_ruleset(&mut state);

    apply_outbound_bind_interface(
        &mut state.outbounds,
        input.settings.core_basic.bind_interface.as_deref(),
    );
    apply_outbound_send_through(
        &mut state.outbounds,
        input.settings.core_basic.send_through.as_deref(),
    );

    let mut main_map = state.config;
    main_map.insert("outbounds".into(), Value::Array(state.outbounds));
    main_map.insert("endpoints".into(), Value::Array(state.endpoints));
    let mut main = Value::Object(main_map);
    let has_custom = !state.custom_tags.is_empty();
    apply_custom_outbound_replace(&mut main, input, &state.custom_tags)?;
    if has_custom {
        let endpoints_empty = main
            .get("endpoints")
            .and_then(Value::as_array)
            .map(|a| a.is_empty())
            .unwrap_or(false);
        if endpoints_empty {
            if let Some(map) = main.as_object_mut() {
                map.remove("endpoints");
            }
        }
    }
    if let Some(template) = &input.template {
        full_config_template(&mut main, input, template)?;
    }
    Ok(GeneratedConfigs::new(main, state.diagnostics))
}

fn should_bind_net(server: &Value) -> bool {
    let server_type = server.get("type").and_then(Value::as_str).unwrap_or("");
    if ["direct", "block", "dns", "selector", "urltest"].contains(&server_type) {
        return false;
    }
    if server
        .get("detour")
        .and_then(Value::as_str)
        .is_some_and(|d| !d.is_empty())
    {
        return false;
    }
    let address = server.get("server").and_then(Value::as_str).unwrap_or("");
    if address.eq_ignore_ascii_case("localhost") {
        return false;
    }
    !is_loopback_address(address)
}

pub(crate) fn apply_outbound_bind_interface(outbounds: &mut [Value], bind_interface: Option<&str>) {
    let bind_interface = bind_interface.map(str::trim).unwrap_or("");
    if bind_interface.is_empty() {
        return;
    }
    for server in outbounds.iter_mut() {
        let should = should_bind_net(server);
        if let Some(map) = server.as_object_mut() {
            if should {
                map.insert("bind_interface".into(), json!(bind_interface));
            } else {
                map.remove("bind_interface");
            }
        }
    }
}

pub(crate) fn apply_outbound_send_through(outbounds: &mut [Value], send_through: Option<&str>) {
    let send_through = send_through.map(str::trim).unwrap_or("");
    if send_through.is_empty() {
        return;
    }
    for server in outbounds.iter_mut() {
        let should = should_bind_net(server);
        if let Some(map) = server.as_object_mut() {
            if should {
                map.insert("inet4_bind_address".into(), json!(send_through));
            } else {
                map.remove("inet4_bind_address");
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
    replace_in_array(main, "outbounds", input, custom_tags)?;
    replace_in_array(main, "endpoints", input, custom_tags)?;
    Ok(())
}

fn replace_in_array(
    main: &mut Value,
    key: &str,
    input: &CodegenInput,
    custom_tags: &[(String, String)],
) -> Result<(), CodegenError> {
    let Some(array) = main.get_mut(key).and_then(Value::as_array_mut) else {
        return Ok(());
    };
    for (tag, index_id) in custom_tags {
        let Some(position) = array
            .iter()
            .position(|o| o.get("tag").and_then(Value::as_str) == Some(tag.as_str()))
        else {
            continue;
        };
        let placeholder = array[position].clone();
        let detour = outbound_detour(&placeholder).unwrap_or_default();
        let interface = placeholder
            .get("bind_interface")
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
            custom_map.insert("detour".into(), json!(detour));
        } else if detour.is_empty() {
            custom_map.remove("detour");
        }
        if !contain_interface && !interface.is_empty() {
            custom_map.insert("bind_interface".into(), json!(interface));
        }
        array[position] = custom;
    }
    Ok(())
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

    // Frozen T08 contract §5: generated outbounds first (AddProxyOnly skips
    // direct/block), template outbounds appended after.
    let generated_outbounds = main
        .get("outbounds")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut merged: Vec<Value> = Vec::new();
    for mut outbound in generated_outbounds {
        let server_type = outbound
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase();
        if matches!(server_type.as_str(), "direct" | "block") && template.add_proxy_only {
            continue;
        }
        if outbound.get("detour").is_none() {
            if let Some(detour) = template.proxy_detour.as_deref().filter(|s| !s.is_empty()) {
                let server = outbound
                    .get("server")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if !is_private_network(&server) {
                    if let Some(map) = outbound.as_object_mut() {
                        map.insert("detour".into(), json!(detour));
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

    // endpoints: generated endpoints are appended to the template's endpoints.
    let generated_endpoints = main
        .get("endpoints")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !generated_endpoints.is_empty() {
        let mut endpoint_merged: Vec<Value> = Vec::new();
        for mut endpoint in generated_endpoints {
            if endpoint.get("detour").is_none() {
                if let Some(detour) = template.proxy_detour.as_deref().filter(|s| !s.is_empty()) {
                    if let Some(map) = endpoint.as_object_mut() {
                        map.insert("detour".into(), json!(detour));
                    }
                }
            }
            endpoint_merged.push(endpoint);
        }
        if let Some(template_endpoints) = template_map.get("endpoints").and_then(Value::as_array) {
            endpoint_merged.extend(template_endpoints.iter().cloned());
        }
        template_map.insert("endpoints".into(), Value::Array(endpoint_merged));
    }

    *main = template_value;
    Ok(())
}

/// Helper to keep `Map` import used across modules.
#[allow(dead_code)]
pub(crate) fn empty_map() -> Map<String, Value> {
    obj()
}
