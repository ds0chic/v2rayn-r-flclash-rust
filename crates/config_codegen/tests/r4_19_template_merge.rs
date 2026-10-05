//! R4-19: full-config template merge order, coverage rules and conflict
//! handling, checked against the frozen `V2rayConfigTemplateService` and
//! `SingboxConfigTemplateService` separately.
mod common;

use common::*;
use config_codegen::input::*;
use config_codegen::{generate_singbox, generate_xray};
use serde_json::json;

fn vless(id: &str) -> CodegenProfile {
    let mut p = profile(ConfigType::Vless, "192.0.2.60", 443);
    p.index_id = id.into();
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p
}

fn template(text: &str, add_proxy_only: bool, detour: Option<&str>) -> CodegenTemplate {
    CodegenTemplate {
        enabled: true,
        config: Some(text.into()),
        add_proxy_only,
        proxy_detour: detour.map(ToString::to_string),
        ..Default::default()
    }
}

#[test]
fn singbox_template_outbounds_precede_generated() {
    // `SingboxConfigTemplateService.cs:106-125`: template outbounds are the
    // base array; generated outbounds are appended after them.
    let template_json = r#"{
      "log": {"level": "debug"},
      "inbounds": [],
      "outbounds": [{"type": "socks", "tag": "template-socks", "server": "192.0.2.250", "server_port": 11890}],
      "route": {"rules": []}
    }"#;
    let mut input = codegen_input(vless("n1"));
    input.template = Some(template(template_json, true, Some("corp-detour")));

    let main = generate_singbox(&input).expect("template").main;
    let tags: Vec<String> = main["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| {
            o.get("tag")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
        })
        .collect();
    assert_eq!(
        tags,
        vec!["template-socks".to_string(), "proxy".to_string()],
        "template outbounds must come first, generated appended; AddProxyOnly drops direct/block"
    );
    assert_eq!(string_at(&main, "/outbounds/1/detour"), "corp-detour");
}

#[test]
fn singbox_template_endpoints_precede_generated() {
    // `:127-141`: generated endpoints are appended to the template endpoints.
    let template_json = r#"{
      "log": {"level": "debug"},
      "inbounds": [],
      "outbounds": [],
      "endpoints": [{"type": "wireguard", "tag": "tmpl-ep", "address": ["10.9.0.2/32"]}]
    }"#;
    let mut p = profile(ConfigType::Outbound, "custom-endpoint.json", 1);
    p.index_id = "custom-ep".into();
    p.proto_extra.is_singbox_endpoint = Some(true);
    let content = r#"{"type": "wireguard", "tag": "{{tag}}", "address": ["10.7.0.2/32"], "private_key": "synthetic-key", "peers": []}"#;
    let mut input = codegen_input(p);
    input
        .custom_outbound_content
        .insert("custom-ep".into(), content.into());
    input.template = Some(template(template_json, false, None));

    let main = generate_singbox(&input).expect("template").main;
    assert_eq!(string_at(&main, "/endpoints/0/tag"), "tmpl-ep");
    assert_eq!(string_at(&main, "/endpoints/1/tag"), "proxy");
}

#[test]
fn xray_template_outbounds_follow_generated() {
    // Contrast case: `V2rayConfigTemplateService.cs:177-220` emits generated
    // outbounds first, then appends the template outbounds.
    let template_json = r#"{
      "log": {"loglevel": "warning"},
      "inbounds": [],
      "outbounds": [{"protocol": "freedom", "tag": "tmpl-free"}],
      "routing": {"domainStrategy": "IPIfNonMatch", "rules": []}
    }"#;
    let mut input = codegen_input(vless("n2"));
    input.template = Some(template(template_json, true, Some("corp-detour")));

    let main = generate_xray(&input).expect("template").main;
    let tags: Vec<String> = main["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| {
            o.get("tag")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
        })
        .collect();
    assert_eq!(
        tags,
        vec!["proxy".to_string(), "tmpl-free".to_string()],
        "generated outbounds first, template appended (Xray service)"
    );
}

#[test]
fn template_wins_on_shared_sections_and_keeps_unknown_keys() {
    // The template node is the final document: shared top-level sections the
    // template provides override the generated skeleton, and unknown/extra
    // keys are preserved verbatim (no silent field loss).
    let template_json = r#"{
      "log": {"level": "template-level"},
      "inbounds": [{"type": "mixed", "tag": "template-in", "listen_port": 11877}],
      "outbounds": [],
      "route": {"rules": []},
      "experimental": {"clash_api": {"external_controller": "127.0.0.1:19090"}},
      "r4_19_unknown_section": {"keep": true}
    }"#;
    let mut input = codegen_input(vless("n3"));
    input.template = Some(template(template_json, true, None));

    let main = generate_singbox(&input).expect("template").main;
    assert_eq!(string_at(&main, "/log/level"), "template-level");
    assert_eq!(string_at(&main, "/inbounds/0/tag"), "template-in");
    assert_eq!(
        string_at(&main, "/experimental/clash_api/external_controller"),
        "127.0.0.1:19090"
    );
    assert_eq!(json_at(&main, "/r4_19_unknown_section/keep"), json!(true));
}

#[test]
fn singbox_proxy_detour_skips_private_and_existing_detour() {
    let template_json = r#"{"log": {"level": "debug"}, "inbounds": [], "outbounds": []}"#;
    let mut private = vless("priv");
    private.address = "127.0.0.1".into();
    let mut input = codegen_input(private);
    input.template = Some(template(template_json, false, Some("corp-detour")));

    let main = generate_singbox(&input).expect("template").main;
    let proxy = main["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o.get("tag").and_then(|v| v.as_str()) == Some("proxy"))
        .unwrap();
    assert!(
        proxy.get("detour").is_none(),
        "private-network outbounds must not receive ProxyDetour"
    );
}

#[test]
fn invalid_template_json_is_a_readable_error() {
    let mut input = codegen_input(vless("n4"));
    input.template = Some(template("{not json", true, None));
    let err = generate_singbox(&input).expect_err("bad template must fail");
    assert_eq!(err.code, "invalid_template");
}
