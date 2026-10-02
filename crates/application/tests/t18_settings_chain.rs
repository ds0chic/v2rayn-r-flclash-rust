//! T18 T12b-lite settings chain assertions.
//!
//! Verifies that persisted settings really reach the generated kernel config
//! (`store -> AppEngine::build_codegen_input -> settings_from_app -> generate`).
//! Also pins the *current* behaviour of fields that are persisted but not yet
//! consumed by codegen, so the gap is visible in the gate instead of silently
//! passing.

use std::sync::Arc;

use application::codegen::CodegenOptions;
use application::runtime_client::NullRuntimeClient;
use application::AppEngine;
use domain::{ConfigType, CoreType, DesiredRevision, Profile};

fn free_port() -> u16 {
    let mut base = 11811;
    loop {
        if std::net::TcpListener::bind(("127.0.0.1", base)).is_ok() {
            return base;
        }
        base += 1;
    }
}

fn leaf(mux: bool) -> Profile {
    let mut profile = Profile {
        index_id: "n1".into(),
        config_type: ConfigType::Vless,
        core_type: Some(CoreType::Xray),
        remarks: "n1".into(),
        address: "192.0.2.10".into(),
        port: 443,
        password: "11111111-2222-3333-4444-555555555555".into(),
        mux_enabled: if mux { Some(true) } else { None },
        ..Default::default()
    };
    profile.proto_extra.vless_encryption = Some("none".into());
    profile
}

fn engine_with_seed() -> (tempfile::TempDir, AppEngine) {
    let dir = tempfile::tempdir().unwrap();
    let engine =
        AppEngine::open_with_runtime(dir.path(), Arc::new(NullRuntimeClient::new())).unwrap();
    engine
        .save_profile(leaf(true), DesiredRevision::ZERO)
        .expect("save seed profile");
    (dir, engine)
}

fn opts() -> CodegenOptions {
    CodegenOptions {
        local_port: free_port() as i32,
        state_port: 11899,
        state_port2: 11898,
        ..Default::default()
    }
}

/// Apply a batch of distinctive settings, then assert the generated configs.
#[test]
fn settings_reach_generated_config() {
    let (_dir, engine) = engine_with_seed();
    let loaded = engine.load_settings().unwrap();
    let mut settings = loaded.settings;
    settings.core_basic_item.loglevel = Some("debug".into());
    settings.core_basic_item.log_enabled = true;
    if let Some(first) = settings.inbound.first_mut() {
        first.udp_enabled = false;
        first.sniffing_enabled = true;
        first.route_only = true;
        first.dest_override = Some(vec!["http".into(), "tls".into()]);
    }
    settings.routing_basic_item.domain_strategy = Some("IPIfNonMatch".into());
    settings.mux4_ray_item.concurrency = Some(7);
    settings.mux4_sbox_item.protocol = Some("smux".into());
    settings.tun_mode_item.enable_tun = true;
    settings.tun_mode_item.mtu = 1400;
    settings.tun_mode_item.stack = Some("system".into());
    engine
        .save_settings(settings, loaded.revision)
        .expect("save distinctive settings");

    let options = opts();
    let xray_input = engine
        .build_codegen_input("n1", CoreType::Xray, &options)
        .unwrap();
    let xray = serde_json::to_string(
        &application::codegen::generate(CoreType::Xray, &xray_input)
            .unwrap()
            .main,
    )
    .unwrap();

    // CoreBasic: log level reaches the log block.
    assert!(
        xray.contains("\"loglevel\":\"debug\""),
        "loglevel debug missing: {xray}"
    );
    // Inbound: udp/sniffing/routeOnly/destOverride.
    assert!(xray.contains("\"udp\":false"), "udp false missing");
    assert!(
        xray.contains("\"destOverride\":[\"http\",\"tls\"]"),
        "destOverride missing"
    );
    // Routing: domain strategy.
    assert!(
        xray.contains("\"domainStrategy\":\"IPIfNonMatch\""),
        "domainStrategy missing"
    );
    // TUN: enabled + mtu.
    assert!(xray.contains("\"protocol\":\"tun\""), "tun inbound missing");
    assert!(xray.contains("\"MTU\":1400"), "tun mtu missing");
    // Mux (Xray): concurrency.
    assert!(
        xray.contains("\"concurrency\":7"),
        "mux concurrency missing"
    );

    let singbox_input = engine
        .build_codegen_input("n1", CoreType::SingBox, &options)
        .unwrap();
    let singbox = serde_json::to_string(
        &application::codegen::generate(CoreType::SingBox, &singbox_input)
            .unwrap()
            .main,
    )
    .unwrap();
    assert!(
        singbox.contains("\"stack\":\"system\""),
        "singbox tun stack missing"
    );
    assert!(
        singbox.contains("\"protocol\":\"smux\""),
        "singbox mux protocol missing"
    );

    println!("T18_SETTINGS_CHAIN {{\"verified\":[\"CoreBasic.Loglevel\",\"Inbound.UdpEnabled\",\"Inbound.DestOverride\",\"Routing.DomainStrategy\",\"Tun.EnableTun\",\"Tun.Mtu\",\"Tun.Stack\",\"Mux4Ray.Concurrency\",\"Mux4Sbox.Protocol\"],\"status\":\"pass\"}}");
}

/// Characterization for a persisted-but-unwired field: `Inbound.Protocol` is
/// stored and shown in the UI, but `settings_from_app` never maps it and the
/// generator always emits `"protocol":"mixed"`. This test documents the gap
/// (it will need to change when the field is actually wired).
#[test]
fn inbound_protocol_is_persisted_but_generator_always_emits_mixed() {
    let (_dir, engine) = engine_with_seed();
    let loaded = engine.load_settings().unwrap();
    let mut settings = loaded.settings;
    if let Some(first) = settings.inbound.first_mut() {
        first.protocol = domain::InboundProtocol::Socks2;
    }
    engine
        .save_settings(settings, loaded.revision)
        .expect("save inbound protocol");

    let xray_input = engine
        .build_codegen_input("n1", CoreType::Xray, &opts())
        .unwrap();
    let xray = serde_json::to_string(
        &application::codegen::generate(CoreType::Xray, &xray_input)
            .unwrap()
            .main,
    )
    .unwrap();
    assert!(
        xray.contains("\"protocol\":\"mixed\""),
        "expected the generator to still emit mixed (known gap): {xray}"
    );
    println!("T18_SETTINGS_GAP {{\"field\":\"FLD-CFG-036\",\"name\":\"Inbound.Protocol\",\"observed\":\"generator_always_mixed\",\"status\":\"unwired\"}}");
}
