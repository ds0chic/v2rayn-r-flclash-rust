//! FIX-13: TUN + pre-SOCKS plan wiring.
//!
//! Verifies that `AppEngine::build_runtime_plan(_with_hints)` actually writes
//! the TUN descriptor and the pre-SOCKS/LegacyProtect process topology into the
//! immutable [`RuntimePlan`] (upstream `CoreConfigContextBuilder::BuildAll` /
//! `CoreManager.LoadCore`). No helper, core or OS is touched, and every port is
//! `>= 11808` (never 10808).

use std::sync::Arc;

use application::{AppEngine, NullRuntimeClient, TunPlanHints, PRE_SOCKS_PROCESS_ID};
use domain::{ConfigType, CoreType, DesiredRevision, Profile};
use runtime::tun::{tun_spec_from_plan, TUN_PROCESS_ID};

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

fn vless_leaf(id: &str) -> Profile {
    let mut profile = Profile {
        index_id: id.into(),
        config_type: ConfigType::Vless,
        core_type: Some(CoreType::Xray),
        remarks: id.into(),
        address: "192.0.2.10".into(),
        port: 443,
        password: "11111111-2222-3333-4444-555555555555".into(),
        ..Default::default()
    };
    profile.proto_extra.vless_encryption = Some("none".into());
    profile
}

fn configure(engine: &AppEngine, base: u16, tun: bool, legacy: bool) {
    let loaded = engine.load_settings().expect("load settings");
    let mut settings = loaded.settings;
    settings.tun_mode_item.enable_tun = tun;
    settings.tun_mode_item.enable_legacy_protect = legacy;
    if let Some(first) = settings.inbound.first_mut() {
        first.local_port = base as i32;
    }
    engine
        .save_settings(settings, loaded.revision)
        .expect("save settings");
}

fn hints() -> TunPlanHints {
    TunPlanHints {
        adapter_name: "v2rayn-tun".into(),
        interface_index: 9,
        routes: Vec::new(),
    }
}

#[allow(clippy::result_large_err)]
fn plan_with(
    engine: &AppEngine,
    target: &str,
    hints: &TunPlanHints,
) -> Result<domain::RuntimePlan, domain::DomainError> {
    let revision = engine.desired_revision();
    engine.build_runtime_plan_with_hints(target, revision, hints)
}

fn has_process(plan: &domain::RuntimePlan, id: &str) -> bool {
    plan.process_graph.nodes.iter().any(|node| node.id == id)
}

#[test]
fn tun_disabled_plan_has_no_tun_node_or_privilege() {
    let engine = engine();
    save(&engine, vless_leaf("n1"));
    configure(&engine, free_port(11808), false, false);
    let plan = plan_with(&engine, "n1", &hints()).expect("plan");
    assert!(!plan.network_policy.tun_enabled);
    assert!(!has_process(&plan, TUN_PROCESS_ID));
    assert!(tun_spec_from_plan(&plan).unwrap().is_none());
    assert!(!plan
        .privileges
        .contains(&domain::runtime_plan::RequiredPrivilege::Tun));
}

#[test]
fn tun_enabled_without_interface_hint_is_a_structured_error() {
    let engine = engine();
    save(&engine, vless_leaf("n1"));
    configure(&engine, free_port(11818), true, false);
    let error = plan_with(&engine, "n1", &TunPlanHints::default())
        .expect_err("missing interface index must not silently drop TUN");
    assert_eq!(error.code, domain::codes::INVALID_ARGUMENT);
    assert_eq!(error.field_path.as_deref(), Some("interface_index"));
}

#[test]
fn tun_enabled_with_hint_attaches_a_round_tripping_tun_node() {
    let engine = engine();
    save(&engine, vless_leaf("n1"));
    configure(&engine, free_port(11828), true, false);
    let plan = plan_with(&engine, "n1", &hints()).expect("plan");
    assert!(plan.network_policy.tun_enabled);
    assert!(has_process(&plan, TUN_PROCESS_ID));
    assert!(plan
        .privileges
        .contains(&domain::runtime_plan::RequiredPrivilege::Tun));
    plan.validate().expect("plan must validate");
    let spec = tun_spec_from_plan(&plan).expect("parse").expect("some");
    assert_eq!(spec.interface_index, 9);
    assert_eq!(spec.adapter_name, "v2rayn-tun");
}

#[test]
fn legacy_protect_writes_presocks_graph_and_start_order() {
    let engine = engine();
    save(&engine, vless_leaf("n1"));
    let base = free_port(11838);
    configure(&engine, base, true, true);
    let plan = plan_with(&engine, "n1", &hints()).expect("plan");

    let main_id = plan
        .process_graph
        .nodes
        .iter()
        .find(|node| node.id != PRE_SOCKS_PROCESS_ID && node.id != TUN_PROCESS_ID)
        .map(|node| node.id.clone())
        .expect("main core node missing");
    assert!(
        has_process(&plan, PRE_SOCKS_PROCESS_ID),
        "pre-socks sidecar node missing"
    );
    let sidecar = plan
        .process_graph
        .nodes
        .iter()
        .find(|node| node.id == PRE_SOCKS_PROCESS_ID)
        .unwrap();
    assert_eq!(sidecar.core_type, CoreType::SingBox);
    assert_eq!(sidecar.ports.len(), 1);
    assert_eq!(sidecar.ports[0].port, base);
    assert!(!sidecar.ports[0].exclusive);

    // Frozen CoreManager order: start main core, then wait for the proxy port,
    // then the pre-service (the TUN node is order-independent and may follow).
    let start = plan.process_graph.start_order().unwrap();
    let main_at = start.iter().position(|id| id == &main_id).unwrap();
    let sidecar_at = start
        .iter()
        .position(|id| id == PRE_SOCKS_PROCESS_ID)
        .unwrap();
    assert!(
        main_at < sidecar_at,
        "main core must start first: {start:?}"
    );
    let stop = plan.stop_order().unwrap();
    assert!(
        stop.iter()
            .position(|id| id == PRE_SOCKS_PROCESS_ID)
            .unwrap()
            < stop.iter().position(|id| id == &main_id).unwrap(),
        "pre-service must stop first: {stop:?}"
    );

    // The shared user port is recorded at plan level but never as an exclusive
    // double-claim, so a valid two-process plan still validates.
    assert!(plan
        .ports
        .iter()
        .any(|port| port.owner == PRE_SOCKS_PROCESS_ID && port.port == base && !port.exclusive));
    plan.validate().expect("two-process plan must validate");
}

#[test]
fn no_presocks_sidecar_when_legacy_protect_is_off() {
    let engine = engine();
    save(&engine, vless_leaf("n1"));
    configure(&engine, free_port(11848), true, false);
    let plan = plan_with(&engine, "n1", &hints()).expect("plan");
    assert!(!has_process(&plan, PRE_SOCKS_PROCESS_ID));
}

#[test]
fn custom_presocks_port_is_recorded_for_the_sidecar() {
    let engine = engine();
    let pre_port = free_port(11858);
    let mut custom = Profile {
        index_id: "custom1".into(),
        config_type: ConfigType::Custom,
        core_type: Some(CoreType::Xray),
        remarks: "custom1".into(),
        address: "custom.json".into(),
        pre_socks_port: Some(pre_port as i32),
        ..Default::default()
    };
    custom.proto_extra.extra.insert(
        application::codegen::CUSTOM_CONFIG_KEY.to_string(),
        serde_json::json!(
            r#"{"log":{"loglevel":"warning"},"inbounds":[],"outbounds":[{"protocol":"freedom","tag":"direct"}]}"#
        ),
    );
    save(&engine, custom);
    configure(&engine, free_port(11868), false, false);
    let plan = plan_with(&engine, "custom1", &hints()).expect("plan");
    assert!(has_process(&plan, PRE_SOCKS_PROCESS_ID));
    assert!(plan
        .ports
        .iter()
        .any(|port| port.owner == PRE_SOCKS_PROCESS_ID && port.port == pre_port));
    plan.validate().expect("plan must validate");
}
