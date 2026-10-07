//! Wave A acceptance (SP-23, 2026-10-07) for the application package.
//!
//! Plan-level assertions for FLD-CFG-023/024/042/057/058/114/115/123/129/130/159.
//! Synthetic data only; every listener port is loopback-probed `>= 11808`
//! (never 10808); no kernel is started and no OS state is touched.

use std::sync::Arc;

use application::codegen::{
    dns_to_codegen, mihomo_body_for_plan, CodegenOptions, CUSTOM_CONFIG_KEY,
};
use application::monitor::{InMemoryTrafficStore, StatsService};
use application::{AppEngine, NullRuntimeClient};
use domain::{ConfigType, CoreType, CoreTypeBinding, DesiredRevision, DnsProfile, Profile};

fn free_port(mut base: u16) -> u16 {
    loop {
        assert!(base < 60000, "port scan exhausted");
        if base == 10808 {
            base += 1;
            continue;
        }
        if std::net::TcpListener::bind(("127.0.0.1", base)).is_ok() {
            assert!(base >= 11808, "test port must stay >= 11808");
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

fn vless_leaf(id: &str, core: Option<CoreType>, mux: bool) -> Profile {
    let mut profile = Profile {
        index_id: id.into(),
        config_type: ConfigType::Vless,
        core_type: core,
        remarks: id.into(),
        address: "192.0.2.10".into(),
        port: 443,
        password: "11111111-2222-3333-4444-555555555555".into(),
        mux_enabled: mux.then_some(true),
        ..Default::default()
    };
    profile.proto_extra.vless_encryption = Some("none".into());
    profile
}

fn set_base_port(engine: &AppEngine, port: u16) {
    let loaded = engine.load_settings().expect("load settings");
    let mut settings = loaded.settings;
    if let Some(first) = settings.inbound.first_mut() {
        first.local_port = port as i32;
    }
    engine
        .save_settings(settings, loaded.revision)
        .expect("save settings");
}

fn plan_body(engine: &AppEngine, target: &str) -> (CoreType, String) {
    let revision = engine.desired_revision();
    let plan = engine
        .build_runtime_plan(target, revision)
        .expect("build runtime plan");
    let core = plan.target.core_type;
    let body = match &plan.target.config {
        domain::runtime_plan::ConfigSource::Inline { body } => body.clone(),
        other => panic!("expected inline config, got {other:?}"),
    };
    assert!(!body.contains("10808"), "plan must never emit 10808");
    (core, body)
}

/// FLD-CFG-023: the stored entry builds a core-start plan from the canonical
/// settings — `CoreTypeItem` binding decides the core for a row without an
/// explicit core, on loopback ports `>= 11808`.
#[test]
fn fld023_entry_core_start_plan_follows_canonical_binding() {
    let engine = engine();
    save(&engine, vless_leaf("n1", None, false));
    let base = free_port(11808);
    set_base_port(&engine, base);

    // Canonical default: no explicit binding -> `init_core_type_items`
    // completes the group and every row falls back to Xray.
    let loaded = engine.load_settings().expect("load");
    let mut settings = loaded.settings;
    settings.core_type_item = None;
    settings.init_core_type_items();
    engine
        .save_settings(settings, loaded.revision)
        .expect("save bindings");
    let (core, body) = plan_body(&engine, "n1");
    assert_eq!(core, CoreType::Xray, "default binding must stay Xray");
    let value: serde_json::Value = serde_json::from_str(&body).expect("json");
    let inbound = &value["inbounds"][0];
    assert_eq!(inbound["listen"], "127.0.0.1", "loopback only");
    assert_eq!(inbound["port"], base);

    // Canonical edit: bind Vless rows to SingBox -> the same entry now plans
    // a SingBox core start without touching the row itself.
    let loaded = engine.load_settings().expect("reload");
    let mut settings = loaded.settings;
    settings.core_type_item = Some(vec![CoreTypeBinding::new(
        ConfigType::Vless,
        CoreType::SingBox,
    )]);
    settings.init_core_type_items();
    engine
        .save_settings(settings, loaded.revision)
        .expect("save singbox binding");
    let (core, body) = plan_body(&engine, "n1");
    assert_eq!(core, CoreType::SingBox, "binding must move the plan core");
    assert!(
        body.contains(&base.to_string()),
        "plan keeps the probed port"
    );
}

/// FLD-CFG-024: existing `t11_routing_dns.rs` covers persist+reopen
/// (`settings_save_preserves_simple_dns_global_fake_ip_and_extra`) and the
/// plan mapping from a fresh struct (`dns_config_feeds_generation_input`),
/// but not save -> reopen -> plan. This closes that loop.
#[test]
fn fld024_dns_global_fake_ip_reaches_plan_after_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let engine =
        AppEngine::open_with_runtime(dir.path(), Arc::new(NullRuntimeClient::new())).expect("open");
    let loaded = engine.load_settings().expect("load");
    let mut settings = loaded.settings;
    settings.simple_dns_item.global_fake_ip = Some(false);
    engine
        .save_settings(settings, loaded.revision)
        .expect("save dns flag");
    drop(engine);
    let reopened = AppEngine::open_with_runtime(dir.path(), Arc::new(NullRuntimeClient::new()))
        .expect("reopen");
    let simple = reopened
        .load_settings()
        .expect("reload")
        .settings
        .simple_dns_item;
    assert_eq!(simple.global_fake_ip, Some(false), "reopen keeps the flag");
    let row = DnsProfile {
        enabled: true,
        normal_dns: Some("8.8.8.8".into()),
        core_type: CoreType::Xray,
        ..Default::default()
    };
    let dns = dns_to_codegen(Some(&row), &simple, Default::default(), Vec::new());
    assert!(dns.enabled);
    assert_eq!(
        dns.simple.global_fake_ip,
        Some(false),
        "SimpleDNS global_fake_ip present in plan after reopen"
    );
}

/// FLD-CFG-159: `SimpleDNSItem.UseSystemHosts` survives reopen and gates the
/// hosts-merge plan branch (`merge_hosts`, consumed by the xray/sing-box DNS
/// emitters). Existing `dns_hosts_merge_semantics` only pins the
/// common-wins/custom-overwrites order, not this switch.
#[test]
fn fld159_use_system_hosts_reaches_plan_after_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let engine =
        AppEngine::open_with_runtime(dir.path(), Arc::new(NullRuntimeClient::new())).expect("open");
    let loaded = engine.load_settings().expect("load");
    let mut settings = loaded.settings;
    settings.simple_dns_item.use_system_hosts = Some(true);
    settings.simple_dns_item.hosts = Some("custom.example.com 93.184.216.34".into());
    engine
        .save_settings(settings, loaded.revision)
        .expect("save hosts switch");
    drop(engine);
    let reopened = AppEngine::open_with_runtime(dir.path(), Arc::new(NullRuntimeClient::new()))
        .expect("reopen");
    let simple = reopened
        .load_settings()
        .expect("reload")
        .settings
        .simple_dns_item;
    assert_eq!(simple.use_system_hosts, Some(true));

    let mut system = std::collections::BTreeMap::new();
    system.insert("sys.example.com".into(), "192.0.2.9".into());
    let merged = domain::dns::merge_hosts(&simple, &system);
    assert!(
        merged.contains_key("sys.example.com"),
        "system hosts enter the plan when the switch is on"
    );
    assert!(
        merged.contains_key("custom.example.com"),
        "custom hosts still win"
    );

    let mut off = simple.clone();
    off.use_system_hosts = Some(false);
    let merged = domain::dns::merge_hosts(&off, &system);
    assert!(
        !merged.contains_key("sys.example.com"),
        "system hosts stay out of the plan when the switch is off"
    );
    assert!(merged.contains_key("custom.example.com"));
}

/// FLD-CFG-042: existing `t18b_runtime_plan.rs:506` sets `new_port4_lan`
/// but only asserts loglevel/auth — the on -> second-inbound /
/// off -> absent assertion is missing, so it lives here.
#[test]
fn fld042_new_port4_lan_second_inbound_on_off() {
    for (flag, expect_second) in [(true, true), (false, false)] {
        let engine = engine();
        save(&engine, vless_leaf("n1", Some(CoreType::Xray), false));
        let base = free_port(11840);
        let loaded = engine.load_settings().expect("load");
        let mut settings = loaded.settings;
        if let Some(first) = settings.inbound.first_mut() {
            first.local_port = base as i32;
            first.allow_lan_conn = true;
            first.new_port4_lan = flag;
        }
        engine
            .save_settings(settings, loaded.revision)
            .expect("save lan flag");
        let (_, body) = plan_body(&engine, "n1");
        let value: serde_json::Value = serde_json::from_str(&body).expect("json");
        let inbounds = value["inbounds"].as_array().expect("inbounds");
        for inbound in inbounds {
            let port = inbound["port"].as_u64().expect("port") as u16;
            assert!(port >= 11808, "listener port stays >= 11808");
        }
        if expect_second {
            assert_eq!(inbounds.len(), 2, "on -> LAN second inbound: {body}");
            assert_eq!(inbounds[0]["listen"], "127.0.0.1");
            assert_eq!(inbounds[0]["port"], base);
            assert_eq!(inbounds[1]["tag"], "socks3");
            assert_eq!(inbounds[1]["listen"], "0.0.0.0");
        } else {
            assert_eq!(inbounds.len(), 1, "off -> no second inbound: {body}");
            assert_eq!(inbounds[0]["listen"], "0.0.0.0");
        }
    }
}

/// FLD-CFG-057/058: the consumer exists — `AppEngine::monitor_settings`
/// (persisted `GuiItem` flags) feeding `StatsService`. The persisted flags
/// reach that consumer-visible structure and survive reopen.
#[test]
fn fld057_058_monitor_flags_reach_consumer() {
    let dir = tempfile::tempdir().expect("tempdir");
    let engine =
        AppEngine::open_with_runtime(dir.path(), Arc::new(NullRuntimeClient::new())).expect("open");
    assert_eq!(
        engine.monitor_settings(),
        (false, false),
        "frozen default off"
    );

    let loaded = engine.load_settings().expect("load");
    let mut settings = loaded.settings;
    settings.gui_item.enable_statistics = true;
    settings.gui_item.display_real_time_speed = true;
    engine
        .save_settings(settings, loaded.revision)
        .expect("save monitor flags");
    assert_eq!(
        engine.monitor_settings(),
        (true, true),
        "persisted 057/058 flags reach the monitor consumer"
    );
    let stats = StatsService::new(
        Box::new(InMemoryTrafficStore::new()),
        engine.monitor_settings().0,
        engine.monitor_settings().1,
    );
    assert!(stats.is_enabled(), "057 EnableStatistics visible");
    assert!(stats.display_speed(), "058 DisplayRealTimeSpeed visible");
    assert!(stats.active());

    drop(engine);
    let reopened = AppEngine::open_with_runtime(dir.path(), Arc::new(NullRuntimeClient::new()))
        .expect("reopen");
    assert_eq!(
        reopened.monitor_settings(),
        (true, true),
        "reopen keeps flags"
    );
    let off = StatsService::new(Box::new(InMemoryTrafficStore::new()), false, false);
    assert!(
        !off.active(),
        "both off -> collection inactive, no forged zeros"
    );
}

/// FLD-CFG-114: `RoutingBasicItem.DomainStrategy` reaches the xray plan
/// (`routing.domainStrategy`). Existing `t18_settings_chain` pins the
/// codegen-input level; this asserts the runtime-plan level.
#[test]
fn fld114_domain_strategy_reaches_xray_plan() {
    let engine = engine();
    save(&engine, vless_leaf("n1", Some(CoreType::Xray), false));
    set_base_port(&engine, free_port(11860));
    let loaded = engine.load_settings().expect("load");
    let mut settings = loaded.settings;
    settings.routing_basic_item.domain_strategy = Some("IPIfNonMatch".into());
    engine
        .save_settings(settings, loaded.revision)
        .expect("save strategy");
    let (core, body) = plan_body(&engine, "n1");
    assert_eq!(core, CoreType::Xray);
    assert!(
        body.contains("\"domainStrategy\":\"IPIfNonMatch\""),
        "xray plan carries the strategy: {body}"
    );
}

/// FLD-CFG-115: `RoutingBasicItem.DomainStrategy4Singbox` reaches the
/// sing-box plan resolve rule (gated on `IPIfNonMatch`, upstream
/// `SingboxRoutingService` parity).
#[test]
fn fld115_domain_strategy4singbox_reaches_singbox_plan() {
    let engine = engine();
    save(&engine, vless_leaf("n1", Some(CoreType::SingBox), false));
    set_base_port(&engine, free_port(11870));
    let loaded = engine.load_settings().expect("load");
    let mut settings = loaded.settings;
    settings.routing_basic_item.domain_strategy = Some("IPIfNonMatch".into());
    settings.routing_basic_item.domain_strategy4_singbox = Some("prefer_ipv4".into());
    engine
        .save_settings(settings, loaded.revision)
        .expect("save sbox strategy");
    let (core, body) = plan_body(&engine, "n1");
    assert_eq!(core, CoreType::SingBox);
    assert!(
        body.contains("\"strategy\":\"prefer_ipv4\""),
        "sing-box plan carries the resolve strategy: {body}"
    );
}

/// FLD-CFG-123: existing `t18_settings_chain.rs:77` sets
/// `Mux4Sbox.Protocol = smux` and the chain assertion covers the
/// codegen-input level; this asserts the runtime-plan level
/// (`multiplex.protocol` in the sing-box body).
#[test]
fn fld123_mux_sbox_protocol_reaches_singbox_plan() {
    let engine = engine();
    save(&engine, vless_leaf("n1", Some(CoreType::SingBox), true));
    set_base_port(&engine, free_port(11875));
    let loaded = engine.load_settings().expect("load");
    let mut settings = loaded.settings;
    settings.mux4_sbox_item.protocol = Some("smux".into());
    engine
        .save_settings(settings, loaded.revision)
        .expect("save mux protocol");
    let (core, body) = plan_body(&engine, "n1");
    assert_eq!(core, CoreType::SingBox);
    assert!(
        body.contains("\"protocol\":\"smux\""),
        "sing-box multiplex protocol reaches the plan: {body}"
    );
}

fn mihomo_settings(
    engine: &AppEngine,
    ipv6: bool,
    mixin: bool,
) -> (domain::AppSettings, CodegenOptions) {
    let base = free_port(11880);
    let state2 = free_port(11900);
    let loaded = engine.load_settings().expect("load");
    let mut settings = loaded.settings;
    if let Some(first) = settings.inbound.first_mut() {
        first.local_port = base as i32;
    }
    settings.clash_ui_item.enable_ipv6 = ipv6;
    settings.clash_ui_item.enable_mixin_content = mixin;
    settings.tun_mode_item.enable_tun = false;
    let opts = CodegenOptions {
        local_port: base as i32,
        state_port: state2 as i32,
        state_port2: state2 as i32,
        ..Default::default()
    };
    (settings, opts)
}

const MIHOMO_BASE: &str =
    "mixed-port: 11808\nmode: rule\nsecret: hunter2\nipv6: false\nproxies: []\nunknown-kept: 7\n";

/// FLD-CFG-129: `ClashUIItem.EnableIPv6` gates the unconditional `ipv6`
/// rewrite in the mihomo custom-plan merge (G-06 generator side).
#[test]
fn fld129_ipv6_rewrite_reaches_mihomo_plan() {
    let engine = engine();
    let (settings, opts) = mihomo_settings(&engine, true, false);
    let body = mihomo_body_for_plan(MIHOMO_BASE, None, None, &settings, &opts).expect("merge");
    assert!(
        body.contains("ipv6: true"),
        "switch on rewrites ipv6: {body}"
    );
    assert!(!body.contains("hunter2"), "secret is stripped");
    assert!(!body.contains("10808"), "never the live proxy port");
    assert!(body.contains("unknown-kept"), "unknown keys retained");

    let (settings, opts) = mihomo_settings(&engine, false, false);
    let body = mihomo_body_for_plan(MIHOMO_BASE, None, None, &settings, &opts).expect("merge");
    assert!(
        body.contains("ipv6: false"),
        "switch off leaves ipv6 false: {body}"
    );

    let (settings, opts) = mihomo_settings(&engine, true, false);
    let error = mihomo_body_for_plan("- just\n- a-list\n", None, None, &settings, &opts)
        .expect_err("bad base YAML must fail");
    assert_eq!(error.code, domain::codes::FIELD_FORMAT);
}

/// FLD-CFG-130: `ClashUIItem.EnableMixinContent` gates the user-mixin merge;
/// off ignores the same file, a bad mixin errors instead of half-merging.
#[test]
fn fld130_mixin_merge_gated_by_switch() {
    let engine = engine();
    let mixin = "unknown-mixin-kept: 42\n";
    let (settings, opts) = mihomo_settings(&engine, false, true);
    let body =
        mihomo_body_for_plan(MIHOMO_BASE, Some(mixin), None, &settings, &opts).expect("merge");
    assert!(
        body.contains("unknown-mixin-kept"),
        "switch on merges the mixin: {body}"
    );

    let (settings, opts) = mihomo_settings(&engine, false, false);
    let body =
        mihomo_body_for_plan(MIHOMO_BASE, Some(mixin), None, &settings, &opts).expect("merge");
    assert!(
        !body.contains("unknown-mixin-kept"),
        "switch off ignores the same mixin: {body}"
    );

    let (settings, opts) = mihomo_settings(&engine, false, true);
    let error = mihomo_body_for_plan(
        MIHOMO_BASE,
        Some("- just\n- a-list\n"),
        None,
        &settings,
        &opts,
    )
    .expect_err("bad mixin must fail");
    assert_eq!(error.code, domain::codes::FIELD_FORMAT);
}

/// FLD-CFG-129/130: the G-06 engine wiring from
/// `SP-24/G-06-engine-wiring.md` is reachable — a mihomo `Custom` entry
/// builds through the `native_custom` merge branch (not verbatim), per the
/// existing `r3_03_native_custom.rs::mihomo_yaml_custom_merges_runtime_rewrites`
/// contract, asserted here end to end.
#[test]
fn fld129_130_engine_wiring_builds_merged_mihomo_plan() {
    let engine = engine();
    let base = free_port(11890);
    let loaded = engine.load_settings().expect("load");
    let mut settings = loaded.settings;
    if let Some(first) = settings.inbound.first_mut() {
        first.local_port = base as i32;
    }
    settings.clash_ui_item.enable_ipv6 = true;
    engine
        .save_settings(settings, loaded.revision)
        .expect("save clash flags");
    let mut profile = Profile {
        index_id: "native".into(),
        config_type: ConfigType::Custom,
        core_type: Some(CoreType::Mihomo),
        remarks: "native".into(),
        address: "inline".into(),
        ..Default::default()
    };
    profile.proto_extra.extra.insert(
        CUSTOM_CONFIG_KEY.to_string(),
        serde_json::json!("mixed-port: 11808\nmode: rule\nsecret: hunter2\nproxies: []\n"),
    );
    save(&engine, profile);
    let revision = engine.desired_revision();
    let plan = engine
        .build_runtime_plan("native", revision)
        .expect("mihomo custom plan must build");
    assert_eq!(plan.target.core_type, CoreType::Mihomo);
    let body = match plan.target.config {
        domain::runtime_plan::ConfigSource::Inline { body } => body,
        other => panic!("expected inline config, got {other:?}"),
    };
    assert!(
        body.contains(&format!("mixed-port: {base}")),
        "engine merge rewrites the port: {body}"
    );
    assert!(
        body.contains("ipv6: true"),
        "engine merge applies ipv6: {body}"
    );
    assert!(!body.contains("hunter2"), "secret stripped by engine merge");
    assert!(!body.contains("10808"), "never the live proxy port");
}
