//! SP-24 / FLD-CFG-103: TUN route-exclude warn-filter at the plan boundary.
//!
//! Upstream `CoreConfigContextBuilder.Build` (frozen `7d6a967`,
//! `CoreConfigContextBuilder.cs:100-118`) keeps valid
//! `TunModeItem.RouteExcludeAddress` entries and records one warning per
//! invalid entry (`ResUI.MsgTunRouteExcludeInvalidAddress`) instead of
//! rejecting the whole list. This suite proves the Rust plan build matches:
//! a mixed list still produces a plan carrying only the valid excludes, and
//! an all-invalid list produces a plan with an empty exclude set (the core
//! generator then falls back to the whole-internet route table, as upstream
//! `V2rayInboundService` does when the filtered list is empty).
//!
//! Synthetic fixtures only; ports `>= 11808` (never 10808); no OS effects.

use std::sync::Arc;

use application::tun_plan::{self, TunPlanHints};
use application::{AppEngine, NullRuntimeClient};
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

fn configure(engine: &AppEngine, base: u16, exclude: Vec<String>) {
    let loaded = engine.load_settings().expect("load settings");
    let mut settings = loaded.settings;
    settings.tun_mode_item.enable_tun = true;
    settings.tun_mode_item.route_exclude_address = Some(exclude);
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

fn plan_spec_exclude(engine: &AppEngine, target: &str) -> Vec<String> {
    let revision = engine.desired_revision();
    let plan = engine
        .build_runtime_plan_with_hints(target, revision, &hints())
        .expect("mixed/all-invalid exclude list must not reject the plan");
    plan.validate().expect("plan must validate");
    assert!(
        plan.process_graph
            .nodes
            .iter()
            .any(|node| node.id == TUN_PROCESS_ID),
        "tun node missing"
    );
    tun_spec_from_plan(&plan)
        .expect("parse")
        .expect("some")
        .route_exclude
}

#[test]
fn mixed_exclude_list_builds_plan_with_valid_entries_only() {
    let engine = engine();
    let saved = engine
        .save_profile(
            vless_leaf("n1"),
            DesiredRevision::new(engine.desired_revision()),
        )
        .expect("save profile")
        .0;
    configure(
        &engine,
        free_port(11908),
        vec!["10.0.0.0/8".into(), "nope".into(), "fc00::/7".into()],
    );
    assert_eq!(
        plan_spec_exclude(&engine, &saved.index_id),
        vec!["10.0.0.0/8".to_string(), "fc00::/7".to_string()]
    );
}

#[test]
fn all_invalid_exclude_list_builds_plan_with_empty_set() {
    let engine = engine();
    let saved = engine
        .save_profile(
            vless_leaf("n1"),
            DesiredRevision::new(engine.desired_revision()),
        )
        .expect("save profile")
        .0;
    configure(&engine, free_port(11918), vec!["nope".into()]);
    assert!(plan_spec_exclude(&engine, &saved.index_id).is_empty());
}

#[test]
fn filter_reports_one_structured_warning_per_invalid_entry() {
    let (kept, warnings) =
        tun_plan::filter_route_exclude(&["10.0.0.0/8".to_string(), "nope".to_string()]);
    assert_eq!(kept, vec!["10.0.0.0/8".to_string()]);
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].code, tun_plan::TUN_ROUTE_EXCLUDE_INVALID_CODE);
    assert_eq!(
        warnings[0].field_path.as_deref(),
        Some(tun_plan::TUN_ROUTE_EXCLUDE_FIELD)
    );
}
