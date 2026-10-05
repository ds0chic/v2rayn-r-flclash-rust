mod common;

use common::*;
use config_codegen::generate_singbox;
use config_codegen::input::*;

#[test]
fn singbox_template_injection() {
    let template_json = r#"{
      "log": {"level": "debug", "timestamp": true},
      "inbounds": [],
      "outbounds": [{"type": "socks", "tag": "template-socks", "server": "192.0.2.250", "server_port": 11890}],
      "endpoints": [],
      "route": {"rules": []}
    }"#;
    let template = CodegenTemplate {
        enabled: true,
        config: Some(template_json.into()),
        add_proxy_only: true,
        proxy_detour: Some("corp-detour".into()),
        ..Default::default()
    };
    let mut p = profile(ConfigType::Vless, "192.0.2.130", 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    let mut input = codegen_input(p);
    input.template = Some(template);

    let generated = generate_singbox(&input).expect("template");
    let main = &generated.main;
    assert_eq!(string_at(main, "/log/level"), "debug");
    // Upstream `SingboxConfigTemplateService`: template outbounds first, then
    // the generated outbounds appended (opposite of the Xray service).
    assert_eq!(string_at(main, "/outbounds/0/tag"), "template-socks");
    assert_eq!(string_at(main, "/outbounds/1/tag"), "proxy");
    assert_eq!(string_at(main, "/outbounds/1/detour"), "corp-detour");
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
    // direct/block generated members are skipped by AddProxyOnly.
    assert!(!tags.contains(&"direct".to_string()));
    assert!(!tags.contains(&"block".to_string()));
}

#[test]
fn singbox_custom_endpoint_marker() {
    let mut p = profile(ConfigType::Outbound, "custom-endpoint.json", 1);
    p.index_id = "custom-ep".into();
    p.proto_extra.is_singbox_endpoint = Some(true);
    let content = r#"{
      "type": "wireguard",
      "tag": "{{tag}}",
      "address": ["10.7.0.2/32"],
      "private_key": "synthetic-private-key",
      "peers": [{"address": "192.0.2.140", "port": 51820, "public_key": "synthetic-public-key", "allowed_ips": ["0.0.0.0/0", "::/0"]}]
    }"#;
    let mut input = codegen_input(p);
    input
        .custom_outbound_content
        .insert("custom-ep".into(), content.into());

    let generated = generate_singbox(&input).expect("endpoint");
    let main = &generated.main;
    assert_eq!(string_at(main, "/endpoints/0/type"), "wireguard");
    assert_eq!(string_at(main, "/endpoints/0/tag"), "proxy");
    assert_eq!(
        string_at(main, "/endpoints/0/private_key"),
        "synthetic-private-key"
    );
    let outbounds = json_at(main, "/outbounds");
    assert!(!outbounds
        .as_array()
        .unwrap()
        .iter()
        .any(|o| o.get("tag").and_then(|v| v.as_str()) == Some("proxy")));
}

#[test]
fn singbox_custom_outbound_replacement() {
    let mut p = profile(ConfigType::Outbound, "custom-outbound.json", 1);
    p.index_id = "custom-1".into();
    let content =
        r#"{"tag": "{{tag}}", "type": "socks", "server": "192.0.2.141", "server_port": 11891}"#;
    let mut input = codegen_input(p);
    input
        .custom_outbound_content
        .insert("custom-1".into(), content.into());

    let generated = generate_singbox(&input).expect("outbound");
    let main = &generated.main;
    assert_eq!(string_at(main, "/outbounds/0/tag"), "proxy");
    assert_eq!(string_at(main, "/outbounds/0/type"), "socks");
    assert_eq!(string_at(main, "/outbounds/0/server"), "192.0.2.141");
    // endpoints is removed when no endpoint remains.
    assert!(main.get("endpoints").is_none());
}

#[test]
fn singbox_custom_passthrough() {
    let mut p = profile(ConfigType::Custom, "", 0);
    p.custom_config = Some(r#"{"log":{"level":"debug"},"inbounds":[],"outbounds":[]}"#.into());
    let generated = generate_singbox(&codegen_input(p)).expect("passthrough");
    assert_eq!(string_at(&generated.main, "/log/level"), "debug");
    assert!(generated
        .diagnostics
        .iter()
        .any(|d| d.code == "custom_passthrough"));
}
