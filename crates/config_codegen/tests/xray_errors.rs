mod common;

use common::*;
use config_codegen::generate_xray;
use config_codegen::input::*;

#[test]
fn xray_rejects_unsupported_config_types() {
    for config_type in [ConfigType::Tuic, ConfigType::Anytls, ConfigType::Naive] {
        let p = profile(config_type, "192.0.2.100", 443);
        let err = generate_xray(&codegen_input(p)).expect_err("unsupported");
        assert_eq!(err.code, "unsupported_combination", "{config_type:?}");
        assert_eq!(err.field_path.as_deref(), Some("profile.configType"));
    }
}

#[test]
fn xray_rejects_quic_network() {
    let mut p = profile(ConfigType::Vless, "192.0.2.101", 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p.network = "quic".into();
    let err = generate_xray(&codegen_input(p)).expect_err("quic");
    assert_eq!(err.code, "unsupported_combination");
    assert_eq!(err.field_path.as_deref(), Some("profile.network"));
}

#[test]
fn xray_missing_required_fields() {
    let p = profile(ConfigType::Vless, "", 443);
    let err = generate_xray(&codegen_input(p)).expect_err("address");
    assert_eq!(err.code, "missing_required_field");
    assert_eq!(err.field_path.as_deref(), Some("profile.address"));

    let mut p = profile(ConfigType::Vmess, "192.0.2.102", 443);
    p.password = String::new();
    let err = generate_xray(&codegen_input(p)).expect_err("password");
    assert_eq!(err.code, "missing_required_field");
    assert_eq!(err.field_path.as_deref(), Some("profile.password"));

    let p = profile(ConfigType::Vless, "192.0.2.103", 0);
    let err = generate_xray(&codegen_input(p)).expect_err("port");
    assert_eq!(err.code, "missing_required_field");
    assert_eq!(err.field_path.as_deref(), Some("profile.port"));
}

#[test]
fn xray_reality_requires_public_key() {
    let mut p = profile(ConfigType::Vless, "192.0.2.104", 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p.stream_security = "reality".into();
    let err = generate_xray(&codegen_input(p)).expect_err("publicKey");
    assert_eq!(err.code, "missing_required_field");
    assert_eq!(err.field_path.as_deref(), Some("profile.publicKey"));
}

#[test]
fn xray_group_dangling_child_reference() {
    let mut group = profile(ConfigType::PolicyGroup, "", 0);
    group.proto_extra.child_items = Some("missing-id".into());
    let err = generate_xray(&codegen_input(group)).expect_err("dangling");
    assert_eq!(err.code, "dangling_reference");
    assert_eq!(
        err.field_path.as_deref(),
        Some("profile.protoExtra.childItems")
    );
}

#[test]
fn xray_group_empty_child_items() {
    let group = profile(ConfigType::PolicyGroup, "", 0);
    let err = generate_xray(&codegen_input(group)).expect_err("empty");
    assert_eq!(err.code, "missing_required_field");
}

#[test]
fn generated_structure_is_deterministic() {
    let mut p = profile(ConfigType::Vmess, "192.0.2.105", 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p.network = "ws".into();
    p.transport_extra.host = Some("ws.test".into());
    let input = codegen_input(p);
    let first = generate_xray(&input).expect("first");
    let second = generate_xray(&input).expect("second");
    assert_eq!(first.main, second.main);
    assert!(!json_at(&first.main, "/outbounds").is_null());
}
