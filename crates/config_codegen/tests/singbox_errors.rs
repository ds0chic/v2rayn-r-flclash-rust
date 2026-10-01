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
