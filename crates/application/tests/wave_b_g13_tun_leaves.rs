//! Wave B G-13 TUN leaves (FLD-CFG-100/102/103/105) at the plan level.
//!
//! Verifies each canonical `TunModeItem` leaf reaches the TUN plan (desired
//! state) and that the actual-state path stays fail-closed without a real
//! adapter. Synthetic fixtures only; every test builds the plan with explicit
//! [`TunPlanHints`] (adapter name/index from test hints, never OS discovery
//! via `tun_hints_from_env` / `discover_interface_index`, never env vars), so
//! no OS call happens. The `enable_tun` master gate is respected everywhere.
//! Ports are loopback pre-probed `>= 11808` (never 10808); no listener is kept.

use std::sync::Arc;

use application::tun_plan::{
    self, TunPlanHints, TUN_DEFERRED_PROCESS_ID, TUN_IPV6_ADDRESS_FIELD,
    TUN_IPV6_ADDRESS_MISSING_CODE, TUN_ROUTE_EXCLUDE_FIELD, TUN_ROUTE_EXCLUDE_INVALID_CODE,
};
use application::{AppEngine, NullRuntimeClient, PRE_SOCKS_PROCESS_ID};
use domain::{codes, ConfigType, CoreType, DesiredRevision, Profile, TunModeItem};
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

fn save_leaf(engine: &AppEngine, id: &str) -> Profile {
    let revision = engine.desired_revision();
    engine
        .save_profile(vless_leaf(id), DesiredRevision::new(revision))
        .expect("save profile")
        .0
}

/// Distinctive hint values: asserting them back proves the plan used the test
/// hints, not OS discovery (which is never called on this path).
fn hints() -> TunPlanHints {
    TunPlanHints {
        adapter_name: "g13-synth-tun".into(),
        interface_index: 29,
        routes: Vec::new(),
    }
}

fn configure(engine: &AppEngine, base: u16, tun: impl FnOnce(&mut TunModeItem)) {
    let loaded = engine.load_settings().expect("load settings");
    let mut settings = loaded.settings;
    tun(&mut settings.tun_mode_item);
    if let Some(first) = settings.inbound.first_mut() {
        first.local_port = base as i32;
    }
    engine
        .save_settings(settings, loaded.revision)
        .expect("save settings");
}

fn build(
    engine: &AppEngine,
    target: &str,
    hints: &TunPlanHints,
) -> (domain::RuntimePlan, Vec<config_codegen::Diagnostic>) {
    let revision = engine.desired_revision();
    engine
        .build_runtime_plan_with_diagnostics(target, revision, hints)
        .expect("plan must build")
}

fn has_process(plan: &domain::RuntimePlan, id: &str) -> bool {
    plan.process_graph.nodes.iter().any(|node| node.id == id)
}

#[test]
fn g13_100_ipv6_on_with_address_reaches_desired_plan() {
    let engine = engine();
    let saved = save_leaf(&engine, "n1");
    configure(&engine, free_port(11928), |tun| {
        tun.enable_tun = true;
        tun.enable_ipv6_address = true;
        tun.ipv6_address = Some("fd00:7::1/64".into());
    });
    let (plan, diagnostics) = build(&engine, &saved.index_id, &hints());
    plan.validate().expect("plan must validate");
    let spec = tun_spec_from_plan(&plan)
        .expect("parse")
        .expect("tun work expected");
    // Hint origin, not OS discovery.
    assert_eq!(spec.adapter_name, "g13-synth-tun");
    assert_eq!(spec.interface_index, 29);
    assert_eq!(spec.addresses.len(), 2);
    assert_eq!(spec.addresses[1].address, "fd00:7::1");
    assert_eq!(spec.addresses[1].prefix_len, 64);
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == TUN_IPV6_ADDRESS_MISSING_CODE),
        "addressed IPv6 must not warn"
    );
}

#[test]
fn g13_100_ipv6_on_without_address_warns_instead_of_silence() {
    let engine = engine();
    let saved = save_leaf(&engine, "n1");
    configure(&engine, free_port(11938), |tun| {
        tun.enable_tun = true;
        tun.enable_ipv6_address = true;
        tun.ipv6_address = None;
    });
    let (plan, diagnostics) = build(&engine, &saved.index_id, &hints());
    plan.validate().expect("plan must validate");
    let spec = tun_spec_from_plan(&plan)
        .expect("parse")
        .expect("tun work expected");
    assert_eq!(spec.addresses.len(), 1, "IPv4-only body");
    let warning = diagnostics
        .iter()
        .find(|d| d.code == TUN_IPV6_ADDRESS_MISSING_CODE)
        .expect("enabled-but-unset IPv6 must warn, never stay silent");
    assert_eq!(warning.field_path.as_deref(), Some(TUN_IPV6_ADDRESS_FIELD));
}

#[test]
fn g13_100_off_keeps_value_but_excludes_it() {
    let engine = engine();
    let saved = save_leaf(&engine, "n1");
    configure(&engine, free_port(11948), |tun| {
        tun.enable_tun = true;
        tun.enable_ipv6_address = false;
        tun.ipv6_address = Some("fd00::1/64".into());
    });
    let (plan, diagnostics) = build(&engine, &saved.index_id, &hints());
    let spec = tun_spec_from_plan(&plan)
        .expect("parse")
        .expect("tun work expected");
    assert_eq!(spec.addresses.len(), 1, "switch off excludes the value");
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == TUN_IPV6_ADDRESS_MISSING_CODE),
        "switch off is a clean state, not a gap"
    );
    // Closing the switch must not delete the user's address value.
    let reloaded = engine.load_settings().expect("reload").settings;
    assert_eq!(
        reloaded.tun_mode_item.ipv6_address.as_deref(),
        Some("fd00::1/64")
    );
}

#[test]
fn g13_105_address_value_reaches_plan_and_bad_cidr_is_rejected() {
    let engine = engine();
    let saved = save_leaf(&engine, "n1");
    configure(&engine, free_port(11958), |tun| {
        tun.enable_tun = true;
        tun.enable_ipv6_address = true;
        tun.ipv6_address = Some("fd00:9::5/126".into());
    });
    let (plan, _) = build(&engine, &saved.index_id, &hints());
    let spec = tun_spec_from_plan(&plan)
        .expect("parse")
        .expect("tun work expected");
    assert_eq!(spec.addresses[1].address, "fd00:9::5");
    assert_eq!(spec.addresses[1].prefix_len, 126);

    configure(&engine, free_port(11968), |tun| {
        tun.enable_tun = true;
        tun.enable_ipv6_address = true;
        tun.ipv6_address = Some("not-a-cidr".into());
    });
    let revision = engine.desired_revision();
    let error = engine
        .build_runtime_plan_with_hints(&saved.index_id, revision, &hints())
        .expect_err("bad IPv6 CIDR must fail closed, never build a plan");
    assert_eq!(error.field_path.as_deref(), Some("IPv6Address"));
}

#[test]
fn g13_103_exclude_reaches_plan_with_warn_filter_and_diagnostics() {
    let engine = engine();
    let saved = save_leaf(&engine, "n1");
    configure(&engine, free_port(11978), |tun| {
        tun.enable_tun = true;
        tun.route_exclude_address =
            Some(vec!["10.0.0.0/8".into(), "nope".into(), "fc00::/7".into()]);
    });
    let (plan, diagnostics) = build(&engine, &saved.index_id, &hints());
    plan.validate().expect("plan must validate");
    let spec = tun_spec_from_plan(&plan)
        .expect("parse")
        .expect("tun work expected");
    assert_eq!(
        spec.route_exclude,
        vec!["10.0.0.0/8".to_string(), "fc00::/7".to_string()]
    );
    let warning = diagnostics
        .iter()
        .find(|d| d.code == TUN_ROUTE_EXCLUDE_INVALID_CODE)
        .expect("invalid entry must surface, not vanish");
    assert_eq!(warning.field_path.as_deref(), Some(TUN_ROUTE_EXCLUDE_FIELD));
    assert!(warning.message.contains("nope"));

    configure(&engine, free_port(11988), |tun| {
        tun.route_exclude_address = Some(vec!["nope".into()]);
    });
    let (plan, _) = build(&engine, &saved.index_id, &hints());
    let spec = tun_spec_from_plan(&plan)
        .expect("parse")
        .expect("all-invalid list must still build");
    assert!(spec.route_exclude.is_empty());
}

#[test]
fn g13_102_legacy_protect_reaches_plan_topology() {
    let engine = engine();
    let saved = save_leaf(&engine, "n1");
    let base = free_port(11998);
    configure(&engine, base, |tun| {
        tun.enable_tun = true;
        tun.enable_legacy_protect = true;
    });
    let (plan, _) = build(&engine, &saved.index_id, &hints());
    plan.validate().expect("plan must validate");
    let sidecar = plan
        .process_graph
        .nodes
        .iter()
        .find(|node| node.id == PRE_SOCKS_PROCESS_ID)
        .expect("legacy protect must write the pre-socks sidecar");
    assert_eq!(sidecar.core_type, CoreType::SingBox);
    assert_eq!(sidecar.ports.len(), 1);
    assert_eq!(sidecar.ports[0].port, base);
    assert!(!sidecar.ports[0].exclusive);

    configure(&engine, free_port(12008), |tun| {
        tun.enable_tun = true;
        tun.enable_legacy_protect = false;
    });
    let (plan, _) = build(&engine, &saved.index_id, &hints());
    assert!(
        !has_process(&plan, PRE_SOCKS_PROCESS_ID),
        "protect off must leave no sidecar"
    );
    plan.validate().expect("plan must validate");
}

#[test]
fn g13_master_gate_tun_off_suppresses_all_leaves() {
    let engine = engine();
    let saved = save_leaf(&engine, "n1");
    configure(&engine, free_port(12018), |tun| {
        tun.enable_tun = false;
        tun.enable_ipv6_address = true;
        tun.ipv6_address = Some("fd00::1/64".into());
        tun.route_exclude_address = Some(vec!["10.0.0.0/8".into()]);
        tun.enable_legacy_protect = true;
    });
    let (plan, _) = build(&engine, &saved.index_id, &hints());
    assert!(!plan.network_policy.tun_enabled);
    assert!(!has_process(&plan, TUN_PROCESS_ID));
    assert!(!has_process(&plan, TUN_DEFERRED_PROCESS_ID));
    assert!(!has_process(&plan, PRE_SOCKS_PROCESS_ID));
    assert!(tun_spec_from_plan(&plan).expect("parse").is_none());
    plan.validate().expect("plan must validate");
}

#[test]
fn g13_actual_state_fail_closed_without_adapter() {
    // Unknown interface: the resolved builder refuses loudly; the plan path
    // defers instead of fabricating an index.
    let item = TunModeItem {
        enable_tun: true,
        ..TunModeItem::default()
    };
    let zero = TunPlanHints::default();
    assert_eq!(zero.interface_index, 0);
    let error = tun_plan::tun_spec_from_settings(&item, &zero).expect_err("must refuse");
    assert_eq!(error.code, codes::INVALID_ARGUMENT);
    assert_eq!(error.field_path.as_deref(), Some("interface_index"));

    let engine = engine();
    let saved = save_leaf(&engine, "n1");
    configure(&engine, free_port(12028), |tun| {
        tun.enable_tun = true;
    });
    let (plan, _) = build(&engine, &saved.index_id, &zero);
    plan.validate().expect("deferred plan must validate");
    assert!(has_process(&plan, TUN_DEFERRED_PROCESS_ID));
    assert!(!has_process(&plan, TUN_PROCESS_ID));
    // The strict actual-state resolver must not mistake the deferred node for
    // ready work: fail closed, never silently claim or skip.
    assert!(tun_spec_from_plan(&plan).is_err());

    // Tampered desired/actual mismatch: flag on but descriptor stripped.
    let (mut resolved, _) = build(&engine, &saved.index_id, &hints());
    resolved
        .process_graph
        .nodes
        .retain(|node| node.id != TUN_PROCESS_ID);
    assert!(resolved.network_policy.tun_enabled);
    let error = tun_spec_from_plan(&resolved).expect_err("missing node must fail");
    assert_eq!(error.code, codes::INVALID_PLAN);
}
