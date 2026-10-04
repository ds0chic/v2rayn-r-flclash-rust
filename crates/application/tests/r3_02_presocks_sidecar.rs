//! R3-02: the pre-SOCKS sidecar config consumes the real settings and the
//! engine derives its ports from the two distinct roles:
//!
//! - user-facing inbound = the local port (`AppManager.GetLocalPort`),
//! - `socks` outbound = the main core proxy port (`GetPreSocksItem` address).
//!
//! Pure generation / plan assertions only: no core or OS is touched, every port
//! is `>= 11808` and never `10808`.

use std::sync::Arc;

use application::{AppEngine, NullRuntimeClient, TunPlanHints, PRE_SOCKS_PROCESS_ID};
use domain::{ConfigType, CoreType, DesiredRevision, Profile};

fn free_port(mut base: u16) -> u16 {
    loop {
        assert!(base < 60000, "port scan exhausted");
        if base == 10808 {
            base += 1;
            continue;
        }
        if std::net::TcpListener::bind(("127.0.0.1", base)).is_ok() {
            return base;
        }
        base += 1;
    }
}

fn engine() -> AppEngine {
    AppEngine::with_runtime(Arc::new(NullRuntimeClient::new()))
}

fn save(engine: &AppEngine, profile: Profile) -> Profile {
    let revision = engine.desired_revision();
    engine
        .save_profile(profile, DesiredRevision::new(revision))
        .expect("save profile")
        .0
}

fn configure(engine: &AppEngine, local_port: u16, tun: bool, legacy: bool) {
    let loaded = engine.load_settings().expect("load settings");
    let mut settings = loaded.settings;
    settings.tun_mode_item.enable_tun = tun;
    settings.tun_mode_item.enable_legacy_protect = legacy;
    if let Some(first) = settings.inbound.first_mut() {
        first.local_port = local_port as i32;
    }
    engine
        .save_settings(settings, loaded.revision)
        .expect("save settings");
}

fn collect_numbers(value: &serde_json::Value, out: &mut Vec<u64>) {
    match value {
        serde_json::Value::Number(number) => {
            if let Some(number) = number.as_u64() {
                out.push(number);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_numbers(item, out);
            }
        }
        serde_json::Value::Object(map) => {
            for item in map.values() {
                collect_numbers(item, out);
            }
        }
        _ => {}
    }
}

fn sidecar_body(plan: &domain::RuntimePlan) -> serde_json::Value {
    let node = plan
        .process_graph
        .nodes
        .iter()
        .find(|node| node.id == PRE_SOCKS_PROCESS_ID)
        .expect("pre-socks sidecar node");
    match &node.config {
        domain::ConfigSource::Inline { body } => {
            serde_json::from_str(body).expect("sidecar body is JSON")
        }
        _ => panic!("inline sidecar config"),
    }
}

fn custom_profile(id: &str, pre_port: u16) -> Profile {
    let mut custom = Profile {
        index_id: id.into(),
        config_type: ConfigType::Custom,
        core_type: Some(CoreType::Xray),
        remarks: id.into(),
        address: "custom.json".into(),
        pre_socks_port: Some(pre_port as i32),
        ..Default::default()
    };
    custom.proto_extra.extra.insert(
        application::codegen::CUSTOM_CONFIG_KEY.to_string(),
        serde_json::json!(
            r#"{"inbounds":[{"port":11867,"protocol":"socks"}],"outbounds":[{"protocol":"freedom","tag":"direct"}]}"#
        ),
    );
    custom
}

#[test]
fn custom_sidecar_inbound_is_local_port_and_outbound_dials_main_core() {
    let engine = engine();
    let pre_port = free_port(11858);
    let local_port = free_port(11868);
    save(&engine, custom_profile("custom1", pre_port));
    configure(&engine, local_port, false, false);

    let revision = engine.desired_revision();
    let plan = engine
        .build_runtime_plan("custom1", revision)
        .expect("native/custom plan builds");

    let body = sidecar_body(&plan);
    let mut inbound_numbers = Vec::new();
    collect_numbers(&body["inbounds"], &mut inbound_numbers);
    let mut outbound_numbers = Vec::new();
    collect_numbers(&body["outbounds"], &mut outbound_numbers);

    assert!(
        outbound_numbers.contains(&(pre_port as u64)),
        "sidecar outbound must dial the main core port {pre_port}: {body}"
    );
    assert!(
        inbound_numbers.contains(&(local_port as u64)),
        "sidecar inbound must listen on the local port {local_port}: {body}"
    );
    assert!(
        !inbound_numbers.contains(&(pre_port as u64)),
        "sidecar inbound must not reuse the main core port: {body}"
    );
}

#[test]
fn legacy_protect_sidecar_reflects_tun_and_real_local_port() {
    let engine = engine();
    let base = free_port(11878);
    let mut leaf = Profile {
        index_id: "n1".into(),
        config_type: ConfigType::Vless,
        core_type: Some(CoreType::Xray),
        remarks: "n1".into(),
        address: "192.0.2.10".into(),
        port: 443,
        password: "11111111-2222-3333-4444-555555555555".into(),
        ..Default::default()
    };
    leaf.proto_extra.vless_encryption = Some("none".into());
    save(&engine, leaf);
    configure(&engine, base, true, true);

    let revision = engine.desired_revision();
    let plan = engine
        .build_runtime_plan_with_hints(
            "n1",
            revision,
            &TunPlanHints {
                adapter_name: "v2rayn-tun".into(),
                interface_index: 9,
                routes: Vec::new(),
            },
        )
        .expect("legacy protect plan builds");

    let body = sidecar_body(&plan);
    let inbounds = body["inbounds"].as_array().expect("inbounds");
    assert!(
        inbounds
            .iter()
            .any(|inbound| inbound["type"].as_str() == Some("tun")),
        "TUN switch must reach the sidecar config: {body}"
    );
    // The sidecar's socks outbound dials the main core's proxy port. In the
    // legacy-protect topology the main core keeps the shared listener, so the
    // sidecar must not re-bind the same local port.
    let mut outbound_numbers = Vec::new();
    collect_numbers(&body["outbounds"], &mut outbound_numbers);
    assert!(
        outbound_numbers.contains(&(base as u64)),
        "sidecar outbound must dial the main core port {base}: {body}"
    );
    let mut inbound_numbers = Vec::new();
    collect_numbers(&body["inbounds"], &mut inbound_numbers);
    assert!(
        !inbound_numbers.contains(&(base as u64)),
        "sidecar must not re-bind the main core's shared port: {body}"
    );
}
