mod common;

use common::*;
use config_codegen::generate_singbox;
use config_codegen::input::*;

fn vless_base() -> CodegenProfile {
    let mut p = profile(ConfigType::Vless, "192.0.2.150", 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p
}

#[test]
fn singbox_rejects_kcp_and_xhttp() {
    for network in ["kcp", "xhttp"] {
        let mut p = vless_base();
        p.network = network.into();
        let err = generate_singbox(&codegen_input(p)).expect_err("rejected network");
        assert_eq!(err.code, "unsupported_combination", "{network}");
        assert_eq!(err.field_path.as_deref(), Some("profile.network"));
    }
}

#[test]
fn singbox_wireguard_requires_address() {
    let p = profile(ConfigType::WireGuard, "", 0);
    let err = generate_singbox(&codegen_input(p)).expect_err("missing address");
    assert_eq!(err.code, "missing_required_field");
    assert_eq!(err.field_path.as_deref(), Some("profile.address"));
}

#[test]
fn singbox_missing_required_fields() {
    let p = profile(ConfigType::Vmess, "192.0.2.151", 443);
    let err = generate_singbox(&codegen_input(p)).expect_err("password");
    assert_eq!(err.code, "missing_required_field");
    assert_eq!(err.field_path.as_deref(), Some("profile.password"));

    let mut p = profile(ConfigType::Tuic, "192.0.2.152", 443);
    p.password = "pw".into();
    let err = generate_singbox(&codegen_input(p)).expect_err("uuid");
    assert_eq!(err.code, "missing_required_field");
}

#[test]
fn singbox_dangling_child_reference() {
    let mut group = profile(ConfigType::ProxyChain, "", 0);
    group.proto_extra.child_items = Some("nope".into());
    let err = generate_singbox(&codegen_input(group)).expect_err("dangling");
    assert_eq!(err.code, "dangling_reference");
    assert_eq!(
        err.field_path.as_deref(),
        Some("profile.protoExtra.childItems")
    );
}

#[test]
fn singbox_unsupported_config_type() {
    let p = profile(ConfigType::Custom, "192.0.2.153", 443);
    // Custom without content is a structured missing-field error.
    let err = generate_singbox(&codegen_input(p)).expect_err("custom");
    assert_eq!(err.code, "missing_required_field");
    assert_eq!(err.field_path.as_deref(), Some("profile.customConfig"));
}

#[test]
fn singbox_reality_requires_public_key_for_all_protocols() {
    // M-014: reality without a public key fails for any protocol type.
    for config_type in [ConfigType::Vmess, ConfigType::Tuic, ConfigType::Anytls] {
        let mut p = profile(config_type, "192.0.2.154", 443);
        p.password = "synthetic-pass".into();
        p.username = "11111111-2222-3333-4444-555555555555".into();
        p.stream_security = "reality".into();
        let err = generate_singbox(&codegen_input(p)).expect_err("reality publicKey");
        assert_eq!(err.code, "missing_required_field", "{config_type:?}");
        assert_eq!(
            err.field_path.as_deref(),
            Some("profile.publicKey"),
            "{config_type:?}"
        );
    }
}

#[test]
fn singbox_rejects_reserved_live_port() {
    // ISSUE-01.
    let mut p = vless_base();
    p.port = 443;
    let mut input = codegen_input(p);
    input.settings.inbound.local_port = 10808;
    let err = generate_singbox(&input).expect_err("reserved port");
    assert_eq!(err.code, "reserved_port");
    assert_eq!(
        err.field_path.as_deref(),
        Some("settings.inbound.localPort")
    );

    let mut input = codegen_input(vless_base());
    input.settings.state_port2 = 10808;
    let err = generate_singbox(&input).expect_err("reserved state port2");
    assert_eq!(err.code, "reserved_port");
    assert_eq!(err.field_path.as_deref(), Some("settings.statePort2"));
}

#[test]
fn singbox_reports_ignored_transport() {
    // M-012: a transport sing-box cannot carry is dropped, but must warn.
    let mut p = profile(ConfigType::Tuic, "192.0.2.155", 443);
    p.username = "11111111-2222-3333-4444-555555555555".into();
    p.password = "pw".into();
    p.network = "ws".into();
    let generated = generate_singbox(&codegen_input(p)).expect("tuic ws");
    let warning = generated
        .diagnostics
        .iter()
        .find(|d| d.code == "singbox_transport_ignored")
        .expect("transport warning");
    assert!(warning.message.contains("ws"), "{}", warning.message);
    // The bogus transport must not leak into the outbound.
    assert!(json_at(&generated.main, "/outbounds/0/transport").is_null());
}

#[test]
fn singbox_reports_dangling_reference() {
    // ISSUE-04 (sing-box side).
    let routing = CodegenRouting {
        rule_set: vec![CodegenRule {
            enabled: true,
            rule_type: RuleType::Routing,
            outbound_tag: "Missing Node".into(),
            domain: Some(vec!["full:missing.test".into()]),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut input = codegen_input(vless_base());
    input.routing = Some(routing);
    let generated = generate_singbox(&input).expect("routing");
    let warning = generated
        .diagnostics
        .iter()
        .find(|d| d.code == "routing_dangling_reference")
        .expect("dangling warning");
    assert!(
        warning.message.contains("Missing Node"),
        "{}",
        warning.message
    );
}

#[test]
fn singbox_omits_empty_endpoints() {
    // M-013: `endpoints` must not be emitted when there is nothing to put in it.
    let generated = generate_singbox(&codegen_input(vless_base())).expect("plain outbound");
    assert!(
        generated.main.get("endpoints").is_none(),
        "empty endpoints must be omitted: {}",
        generated.main
    );
}

#[test]
fn singbox_deterministic_output() {
    let mut p = vless_base();
    p.network = "ws".into();
    p.transport_extra.host = Some("ws.test".into());
    p.transport_extra.path = Some("/p?ed=2048".into());
    let input = codegen_input(p);
    let first = generate_singbox(&input).expect("first");
    let second = generate_singbox(&input).expect("second");
    assert_eq!(first.main, second.main);
}
