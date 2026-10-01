mod common;

use common::*;
use config_codegen::generate_xray;
use config_codegen::input::*;
use serde_json::json;

fn vless_base() -> CodegenProfile {
    let mut p = profile(ConfigType::Vless, "192.0.2.90", 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p
}

#[test]
fn xray_full_template_injection_from_fixture() {
    let template_text = read_sample("SampleClientConfig");
    let template = CodegenTemplate {
        enabled: true,
        config: Some(template_text),
        add_proxy_only: true,
        proxy_detour: Some("corp-detour".into()),
        ..Default::default()
    };
    let mut input = codegen_input(vless_base());
    input.template = Some(template);

    let generated = generate_xray(&input).expect("template");
    let main = &generated.main;
    // Template provides the skeleton (log from the fixture).
    assert_eq!(string_at(main, "/log/access"), "Vaccess.log");
    // Generated proxy first, then the template freedom/blackhole appended.
    assert_eq!(string_at(main, "/outbounds/0/tag"), "proxy");
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/sockopt/dialerProxy"),
        "corp-detour"
    );
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
    assert_eq!(tags, vec!["proxy", "direct", "block"]);
    assert_eq!(string_at(main, "/routing/domainStrategy"), "IPIfNonMatch");
}

#[test]
fn xray_template_balancer_rewrite() {
    let mut group = profile(ConfigType::PolicyGroup, "", 0);
    group.index_id = "group-1".into();
    group.proto_extra.child_items = Some("c1,c2".into());
    group.proto_extra.multiple_load = Some(MultipleLoad::LeastLoad);
    let mut c1 = profile(ConfigType::Vless, "192.0.2.91", 443);
    c1.index_id = "c1".into();
    c1.remarks = "c1".into();
    c1.password = "11111111-2222-3333-4444-555555555555".into();
    let mut c2 = profile(ConfigType::Vless, "192.0.2.92", 443);
    c2.index_id = "c2".into();
    c2.remarks = "c2".into();
    c2.password = "11111111-2222-3333-4444-555555555555".into();

    let template_json = r#"{
      "log": {"loglevel": "warning"},
      "inbounds": [],
      "outbounds": [],
      "routing": {
        "domainStrategy": "IPIfNonMatch",
        "rules": [{"type": "field", "network": "tcp,udp", "outboundTag": "proxy"}]
      }
    }"#;
    let template = CodegenTemplate {
        enabled: true,
        config: Some(template_json.into()),
        add_proxy_only: false,
        proxy_detour: None,
        ..Default::default()
    };
    let mut input = codegen_input(group);
    input.profiles.insert(c1.index_id.clone(), c1);
    input.profiles.insert(c2.index_id.clone(), c2);
    input.template = Some(template);

    let generated = generate_xray(&input).expect("template balancer");
    let main = &generated.main;
    assert_eq!(
        string_at(main, "/routing/rules/0/balancerTag"),
        "proxy-balancer"
    );
    assert!(json_at(main, "/routing/rules/0/outboundTag").is_null());
    assert_eq!(
        string_at(main, "/routing/balancers/0/tag"),
        "proxy-balancer"
    );
    assert_eq!(
        string_at(main, "/routing/balancers/0/strategy/type"),
        "leastLoad"
    );
}

#[test]
fn xray_custom_outbound_placeholders() {
    let mut p = profile(ConfigType::Outbound, "custom-outbound-file.json", 1);
    p.index_id = "custom-1".into();
    let content = r#"{
      "tag": "{{tag}}",
      "protocol": "socks",
      "settings": {"address": "192.0.2.99", "port": 11840},
      "streamSettings": {"sockopt": {"interface": "{{interface}}", "dialerProxy": "{{detour}}"}}
    }"#;
    let mut input = codegen_input(p);
    input
        .custom_outbound_content
        .insert("custom-1".into(), content.into());

    let generated = generate_xray(&input).expect("custom outbound");
    let main = &generated.main;
    assert_eq!(string_at(main, "/outbounds/0/tag"), "proxy");
    assert_eq!(string_at(main, "/outbounds/0/protocol"), "socks");
    assert_eq!(
        string_at(main, "/outbounds/0/settings/address"),
        "192.0.2.99"
    );
    // Empty placeholders behave like upstream: `{{detour}}` is removed by the
    // empty-detour branch, `{{interface}}` stays as an empty string value.
    assert_eq!(
        json_at(main, "/outbounds/0/streamSettings/sockopt/interface"),
        json!("")
    );
    assert!(json_at(main, "/outbounds/0/streamSettings/sockopt/dialerProxy").is_null());
}

#[test]
fn xray_custom_config_passthrough() {
    let mut p = profile(ConfigType::Custom, "", 0);
    p.custom_config =
        Some(r#"{"log": {"loglevel": "debug"}, "inbounds": [], "outbounds": []}"#.into());
    let generated = generate_xray(&codegen_input(p)).expect("passthrough");
    assert_eq!(string_at(&generated.main, "/log/loglevel"), "debug");
    assert!(generated
        .diagnostics
        .iter()
        .any(|d| d.code == "custom_passthrough"));
    assert!(generated.files.is_empty());
}

#[test]
fn xray_custom_outbound_missing_content() {
    let mut p = profile(ConfigType::Outbound, "custom-outbound-file.json", 1);
    p.index_id = "custom-1".into();
    let err = generate_xray(&codegen_input(p)).expect_err("must fail");
    assert_eq!(err.code, "custom_outbound_missing");
}

#[test]
fn xray_outbound_placeholder_is_neutral() {
    // ISSUE-10: the placeholder for a not-yet-replaced custom outbound must not
    // embed a real external domain/port.
    let mut p = profile(ConfigType::Outbound, "custom-outbound-file.json", 1);
    p.index_id = "custom-1".into();
    let mut input = codegen_input(p);
    input
        .custom_outbound_content
        .insert("custom-1".into(), r#"{"type": "socks"}"#.into());
    let generated = generate_xray(&input).expect("custom outbound");
    let text = generated.main.to_string();
    assert!(!text.contains("v2ray.cool"), "{text}");
    assert!(!text.contains("10086"), "{text}");
}
