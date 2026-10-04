//! T18b: real `RuntimePlan` construction from persisted state (T18-F03).
//!
//! Assembles the active node (with policy-group expansion), `AppSettings`,
//! the active routing profile + rules, the DNS row and the rule mode through
//! `AppEngine::build_runtime_plan`, then checks the generated kernel body per
//! node `CoreType`. No kernel is started; every emitted port is `>= 11808`
//! (never 10808). Also pins FLD-CFG-036 (stored inbound protocol drives the
//! base-port lookup and the `mixed` kernel token, upstream parity) and
//! FLD-CFG-102 (the `EnableLegacyProtect` pre-SOCKS decision).

use std::sync::Arc;

use application::{AppEngine, NullRuntimeClient, PreSocksDecision};
use domain::{ConfigType, CoreType, DesiredRevision, Profile, RoutingProfile, RoutingRule};

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

fn socks_node(id: &str, port: u16) -> Profile {
    Profile {
        index_id: id.into(),
        config_type: ConfigType::Socks,
        core_type: Some(CoreType::Xray),
        remarks: id.into(),
        address: "127.0.0.1".into(),
        port: port as i32,
        ..Default::default()
    }
}

fn http_node(id: &str, port: u16) -> Profile {
    Profile {
        index_id: id.into(),
        config_type: ConfigType::Http,
        core_type: Some(CoreType::Xray),
        remarks: id.into(),
        address: "127.0.0.1".into(),
        port: port as i32,
        ..Default::default()
    }
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

#[test]
fn freedom_plan_has_mixed_inbound_and_freedom_outbound() {
    let engine = engine();
    save(&engine, vless_leaf("n1"));
    let base = free_port(11808);
    set_base_port(&engine, base);
    let (core, body) = plan_body(&engine, "n1");
    assert_eq!(core, CoreType::Xray);
    let value: serde_json::Value = serde_json::from_str(&body).expect("json");
    let inbound = &value["inbounds"][0];
    assert_eq!(inbound["listen"], "127.0.0.1");
    assert_eq!(inbound["port"], base);
    assert_eq!(inbound["protocol"], "mixed");
    let outbounds = value["outbounds"].as_array().expect("outbounds");
    assert!(outbounds.iter().any(|o| o["protocol"] == "freedom"));
}

#[test]
fn socks_and_http_nodes_generate_matching_outbounds() {
    let engine = engine();
    let upstream = free_port(11830);
    save(&engine, socks_node("s1", upstream));
    save(&engine, http_node("h1", upstream));
    set_base_port(&engine, free_port(11840));
    let (_, socks_body) = plan_body(&engine, "s1");
    assert!(
        socks_body.contains("\"protocol\":\"socks\""),
        "socks outbound missing: {socks_body}"
    );
    let (_, http_body) = plan_body(&engine, "h1");
    assert!(
        http_body.contains("\"protocol\":\"http\""),
        "http outbound missing: {http_body}"
    );
}

#[test]
fn singbox_plan_uses_singbox_shape() {
    let engine = engine();
    let mut node = vless_leaf("sb1");
    node.core_type = Some(CoreType::SingBox);
    save(&engine, node);
    set_base_port(&engine, free_port(11850));
    let (core, body) = plan_body(&engine, "sb1");
    assert_eq!(core, CoreType::SingBox);
    let value: serde_json::Value = serde_json::from_str(&body).expect("json");
    let inbounds = value["inbounds"].as_array().expect("inbounds");
    assert_eq!(inbounds[0]["type"], "mixed");
    assert!(value["outbounds"].as_array().expect("outbounds").len() >= 2);
}

#[test]
fn group_target_expands_children_in_plan() {
    let engine = engine();
    save(&engine, vless_leaf("c1"));
    save(&engine, vless_leaf("c2"));
    let mut group = Profile {
        index_id: "g1".into(),
        config_type: ConfigType::PolicyGroup,
        core_type: Some(CoreType::Xray),
        remarks: "g1".into(),
        ..Default::default()
    };
    group.proto_extra.child_items = Some("c1,c2".into());
    save(&engine, group);
    set_base_port(&engine, free_port(11860));
    let (_, body) = plan_body(&engine, "g1");
    assert!(
        body.contains("proxy-balancer"),
        "policy group balancer missing"
    );
}

#[test]
fn missing_target_is_a_structured_error_not_a_fallback() {
    let engine = engine();
    save(&engine, vless_leaf("n1"));
    let revision = engine.desired_revision();
    let error = engine
        .build_runtime_plan("no-such-node", revision)
        .expect_err("missing target must fail");
    assert_eq!(error.code, domain::codes::NOT_FOUND);
}

#[test]
fn stale_revision_is_rejected_by_apply() {
    let engine = engine();
    save(&engine, vless_leaf("n1"));
    set_base_port(&engine, free_port(11870));
    let revision = engine.desired_revision();
    let plan = engine.build_runtime_plan("n1", revision).expect("plan");
    // Bump once so `revision` is stale.
    save(&engine, vless_leaf("n2"));
    let error = engine
        .apply_runtime(plan, DesiredRevision::new(revision))
        .expect_err("stale revision must fail");
    assert_eq!(error.code, domain::codes::REVISION_STALE);
}

#[test]
fn settings_save_bumps_desired_so_ui_sees_unapplied() {
    let engine = engine();
    save(&engine, vless_leaf("n1"));
    let before = engine.desired_revision();
    let loaded = engine.load_settings().expect("load");
    let mut settings = loaded.settings;
    settings.core_basic_item.loglevel = Some("debug".into());
    engine
        .save_settings(settings, loaded.revision)
        .expect("save settings");
    assert!(
        engine.desired_revision() > before,
        "settings save must invalidate the applied runtime"
    );
    let rule_before = engine.desired_revision();
    engine
        .set_rule_mode(domain::RuleMode::Global)
        .expect("set rule mode");
    assert!(
        engine.desired_revision() > rule_before,
        "rule-mode switch must invalidate the applied runtime"
    );
}

#[test]
fn inbound_protocol_drives_base_port_and_mixed_token() {
    // FLD-CFG-036: upstream `GetLocalPort` locates the base port on the row
    // whose stored `Protocol` is `socks`; the kernel token is always `mixed`.
    let engine = engine();
    save(&engine, vless_leaf("n1"));
    let loaded = engine.load_settings().expect("load");
    let mut settings = loaded.settings;
    assert_eq!(settings.inbound[0].protocol, domain::InboundProtocol::Socks);
    assert_eq!(engine.runtime_base_port(), settings.inbound[0].local_port);
    // Upstream parity: `LoadConfig` forces the first row back to `socks`
    // on every save, so the locator resolves to row zero here.
    settings.inbound[0].protocol = domain::InboundProtocol::Socks2;
    settings.inbound[0].local_port = 11877;
    settings.inbound.push(domain::InboundListener {
        local_port: 11878,
        protocol: domain::InboundProtocol::Socks,
        ..Default::default()
    });
    engine
        .save_settings(settings, loaded.revision)
        .expect("save settings");
    let reloaded = engine.load_settings().expect("reload").settings;
    assert_eq!(
        reloaded.inbound[0].protocol,
        domain::InboundProtocol::Socks,
        "first row is forced back to socks (upstream parity)"
    );
    assert_eq!(engine.runtime_base_port(), 11877);
    // Every stored identity maps to the `mixed` kernel token (upstream
    // `BuildInbound` hardcodes `mixed`; there is no `http` inbound identity
    // in `EInboundProtocol`).
    for protocol in [
        domain::InboundProtocol::Socks,
        domain::InboundProtocol::Socks2,
        domain::InboundProtocol::Socks3,
        domain::InboundProtocol::Mixed,
        domain::InboundProtocol::Pac,
        domain::InboundProtocol::Api,
        domain::InboundProtocol::Api2,
        domain::InboundProtocol::Speedtest,
    ] {
        assert_eq!(
            application::codegen::inbound_protocol_token(protocol),
            "mixed",
            "token for {protocol:?}"
        );
    }
    let (_, body) = plan_body(&engine, "n1");
    let value: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(value["inbounds"][0]["port"], 11877);
    assert_eq!(value["inbounds"][0]["protocol"], "mixed");
}

#[test]
fn legacy_protect_decision_matches_upstream() {
    // FLD-CFG-102: `ConfigHandler.GetPreSocksItem` parity.
    let engine = engine();
    save(&engine, vless_leaf("n1"));
    let node = engine.profile_by_id("n1").expect("get").expect("present");
    let loaded = engine.load_settings().expect("load");
    let mut settings = loaded.settings;
    settings.tun_mode_item.enable_tun = true;
    settings.tun_mode_item.enable_legacy_protect = true;
    if let Some(first) = settings.inbound.first_mut() {
        first.local_port = free_port(11888) as i32;
    }
    engine
        .save_settings(settings, loaded.revision)
        .expect("save settings");
    let base = engine.runtime_base_port() as u16;
    let decision = engine.pre_socks_decision(&node, CoreType::Xray);
    assert_eq!(
        decision,
        Some(PreSocksDecision {
            core: CoreType::SingBox,
            address: "127.0.0.1".into(),
            port: base,
        })
    );
    // Legacy protect off: no sidecar.
    let loaded = engine.load_settings().expect("load");
    let mut settings = loaded.settings;
    settings.tun_mode_item.enable_legacy_protect = false;
    engine
        .save_settings(settings, loaded.revision)
        .expect("save settings");
    assert_eq!(engine.pre_socks_decision(&node, CoreType::Xray), None);
    // A Custom node with a valid PreSocksPort still gets a sidecar.
    let mut custom = Profile {
        index_id: "custom1".into(),
        config_type: ConfigType::Custom,
        core_type: Some(CoreType::Xray),
        remarks: "custom1".into(),
        address: "custom.json".into(),
        pre_socks_port: Some(11889),
        ..Default::default()
    };
    custom.proto_extra.extra.insert(
        application::codegen::CUSTOM_CONFIG_KEY.to_string(),
        serde_json::json!(r#"{"log":{"loglevel":"warning"},"inbounds":[],"outbounds":[{"protocol":"freedom","tag":"direct"}]}"#),
    );
    save(&engine, custom);
    let custom_node = engine
        .profile_by_id("custom1")
        .expect("get")
        .expect("present");
    let decision = engine.pre_socks_decision(&custom_node, CoreType::Xray);
    assert!(decision.is_some(), "custom pre-socks port must be honored");
}

fn custom_config_node(id: &str, core: CoreType, body: &str) -> Profile {
    let mut profile = Profile {
        index_id: id.into(),
        config_type: ConfigType::Custom,
        core_type: Some(core),
        remarks: id.into(),
        address: format!("{id}.json"),
        ..Default::default()
    };
    profile.proto_extra.extra.insert(
        application::codegen::CUSTOM_CONFIG_KEY.to_string(),
        serde_json::json!(body),
    );
    profile
}

fn plan_ports(engine: &AppEngine, target: &str) -> Vec<(u16, domain::PortTransport)> {
    let revision = engine.desired_revision();
    let plan = engine
        .build_runtime_plan(target, revision)
        .expect("build runtime plan");
    plan.ports.iter().map(|p| (p.port, p.transport)).collect()
}

#[test]
fn custom_config_uses_real_inbound_port_and_metrics() {
    // RR-07: the plan publishes the config's actual inbound port (and protocol
    // scheme), not the settings base port.
    let engine = engine();
    set_base_port(&engine, free_port(11910));
    let body = r#"{"inbounds":[{"port":11911,"listen":"127.0.0.1","protocol":"socks"}],"metrics":{"listen":"127.0.0.1:11915"},"outbounds":[{"protocol":"freedom"}]}"#;
    save(&engine, custom_config_node("c1", CoreType::Xray, body));
    let ports = plan_ports(&engine, "c1");
    assert!(
        ports.contains(&(11911, domain::PortTransport::Tcp)),
        "actual inbound port missing: {ports:?}"
    );
    assert!(
        !ports.iter().any(|(p, _)| *p == 11910),
        "expected base port must not appear: {ports:?}"
    );
}

#[test]
fn custom_config_socks_only_publishes_socks_scheme() {
    // A SOCKS-only Custom config must not be treated as an HTTP download proxy.
    let engine = engine();
    let body =
        r#"{"inbounds":[{"port":11921,"protocol":"socks"}],"outbounds":[{"protocol":"freedom"}]}"#;
    save(&engine, custom_config_node("c2", CoreType::Xray, body));
    let body_json = r#"{"inbounds":[{"port":11925,"protocol":"http"}],"metrics":{"listen":":11926"},"outbounds":[{"protocol":"freedom"}]}"#;
    save(&engine, custom_config_node("c3", CoreType::Xray, body_json));
    // plan for the socks node uses the socks port
    let ports = plan_ports(&engine, "c2");
    assert!(
        ports.iter().any(|(p, _)| *p == 11921),
        "socks port missing: {ports:?}"
    );
}

#[test]
fn custom_config_without_inbound_is_a_structured_error() {
    let engine = engine();
    let body = r#"{"inbounds":[{"port":11930,"protocol":"dokodemo-door"}],"outbounds":[{"protocol":"freedom"}]}"#;
    save(&engine, custom_config_node("c4", CoreType::Xray, body));
    let revision = engine.desired_revision();
    let error = engine
        .build_runtime_plan("c4", revision)
        .expect_err("no proxy inbound must fail");
    assert_eq!(error.code, domain::codes::INVALID_PLAN);
    assert_eq!(error.message_key, "error.custom_endpoint_parse_failed");
}

#[test]
fn custom_config_invalid_json_is_rejected_at_save() {
    // Invalid JSON never reaches storage, so it can never reach the parser.
    let engine = engine();
    let revision = engine.desired_revision();
    let error = engine
        .save_profile(
            custom_config_node("c5", CoreType::Xray, "{not json"),
            DesiredRevision::new(revision),
        )
        .expect_err("invalid json must be rejected");
    assert_eq!(error.code, domain::codes::FIELD_FORMAT);
}

#[test]
fn normal_node_plan_unchanged_by_rr07() {
    // Regression: non-Custom nodes still use the settings base port.
    let engine = engine();
    save(&engine, vless_leaf("n1"));
    let base = free_port(11940);
    set_base_port(&engine, base);
    let ports = plan_ports(&engine, "n1");
    assert!(
        ports.iter().any(|(p, _)| *p == base),
        "base port missing: {ports:?}"
    );
}

fn routing_profile(remarks: &str, rules: &[RoutingRule]) -> RoutingProfile {
    let mut profile = RoutingProfile {
        remarks: remarks.to_string(),
        ..Default::default()
    };
    profile.set_rules(rules).expect("set rules");
    profile
}

#[test]
fn routing_domain_rule_and_dns_switch_reach_plan() {
    let engine = engine();
    save(&engine, vless_leaf("n1"));
    set_base_port(&engine, free_port(11890));
    let saved = engine
        .save_routing(routing_profile(
            "t18b",
            &[RoutingRule {
                id: "r1".into(),
                outbound_tag: Some("direct".into()),
                domain: Some(vec!["full:localhost".into()]),
                enabled: true,
                rule_type: Some(domain::RuleType::Routing),
                ..Default::default()
            }],
        ))
        .expect("save routing");
    engine
        .set_default_routing(&saved.id)
        .expect("set default routing");
    let (_, body) = plan_body(&engine, "n1");
    assert!(
        body.contains("localhost"),
        "routing domain rule missing: {body}"
    );
    // DNS row for this core, disabled: no dns segment expectations beyond a
    // successful build; enabling must add the dns block.
    // Same flow as the DNS window: edit the stored row in place (one row
    // per core), rather than adding a second row for the core.
    let mut row = engine
        .get_dns_for_core(CoreType::Xray)
        .expect("get dns")
        .expect("builtin dns row");
    row.enabled = true;
    row.normal_dns = Some(r#"{"servers":["https://9.9.9.11/dns-query"]}"#.into());
    engine.save_dns(row).expect("save dns");
    let (_, dns_body) = plan_body(&engine, "n1");
    assert!(
        dns_body.contains("9.9.9.11"),
        "enabled DNS must reach the plan: {dns_body}"
    );
}

#[test]
fn settings_loglevel_port_and_auth_reach_plan() {
    let engine = engine();
    save(&engine, vless_leaf("n1"));
    let loaded = engine.load_settings().expect("load");
    let mut settings = loaded.settings;
    settings.core_basic_item.loglevel = Some("debug".into());
    settings.core_basic_item.log_enabled = true;
    if let Some(first) = settings.inbound.first_mut() {
        first.local_port = free_port(11895) as i32;
        first.allow_lan_conn = true;
        first.new_port4_lan = true;
        first.user = "t18b-user".into();
        first.pass = "t18b-pass".into();
    }
    engine
        .save_settings(settings, loaded.revision)
        .expect("save settings");
    let (_, body) = plan_body(&engine, "n1");
    assert!(body.contains("\"loglevel\":\"debug\""), "loglevel missing");
    assert!(body.contains("t18b-user"), "inbound auth user missing");
}
