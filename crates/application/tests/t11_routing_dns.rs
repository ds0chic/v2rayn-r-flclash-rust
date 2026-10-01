//! T11 routing + DNS engine tests: CRUD save/reopen, rule order, dangling
//! warnings, import/export round-trip, loopback URL import, regional presets
//! and DNS CRUD/validation.
//!
//! Ports are only used by the loopback URL-import fixture (>= 11808, never
//! 10808); no kernel is started and no system state is touched.

use std::sync::Arc;

use application::{AppEngine, DnsRepository, RegionalPreset, RoutingRepository};
use domain::{CoreType, DnsProfile, RoutingProfile, RoutingRule, RuleMode};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn rule(id: &str, outbound: &str, domain: Option<&str>) -> RoutingRule {
    RoutingRule {
        id: id.to_string(),
        outbound_tag: Some(outbound.to_string()),
        domain: domain.map(|d| vec![d.to_string()]),
        enabled: true,
        remarks: Some(id.to_string()),
        rule_type: Some(domain::RuleType::Routing),
        ..Default::default()
    }
}

fn profile(remarks: &str, rules: &[RoutingRule]) -> RoutingProfile {
    let mut profile = RoutingProfile {
        remarks: remarks.to_string(),
        ..Default::default()
    };
    profile.set_rules(rules).expect("set rules");
    profile
}

#[test]
fn routing_crud_survives_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let engine =
        AppEngine::open_with_runtime(dir.path(), Arc::new(application::NullRuntimeClient::new()))
            .expect("open");
    // Builtins are seeded on first open.
    assert!(engine.list_routings().expect("list").len() >= 3);

    let saved = engine
        .save_routing(profile(
            "my-scheme",
            &[rule("r1", "proxy", Some("geosite:google"))],
        ))
        .expect("save");
    assert!(saved.rule_num == 1);

    // Reopen and confirm persistence.
    drop(engine);
    let reopened =
        AppEngine::open_with_runtime(dir.path(), Arc::new(application::NullRuntimeClient::new()))
            .expect("reopen");
    let loaded = reopened
        .get_routing(&saved.id)
        .expect("get")
        .expect("present");
    assert_eq!(loaded.remarks, "my-scheme");
    assert_eq!(loaded.rule_num, 1);
    let rules = loaded.rules().expect("rules");
    assert_eq!(rules[0].outbound_tag.as_deref(), Some("proxy"));

    // Remarks are required.
    let bad = profile("", &[]);
    assert!(reopened.save_routing(bad).is_err());
}

#[test]
fn routing_rule_order_and_move() {
    let engine = AppEngine::in_memory();
    let saved = engine
        .save_routing(profile(
            "order",
            &[
                rule("a", "proxy", Some("geosite:a")),
                rule("b", "direct", Some("geosite:b")),
                rule("c", "block", Some("geosite:c")),
            ],
        ))
        .expect("save");
    // Move first rule to bottom: order becomes b, c, a.
    let moved = engine
        .move_routing_rule(&saved.id, 0, domain::routing::MoveDirection::Bottom)
        .expect("move");
    let rules = moved.rules().expect("rules");
    let tags: Vec<_> = rules
        .iter()
        .map(|r| r.outbound_tag.clone().unwrap())
        .collect();
    assert_eq!(tags, vec!["direct", "block", "proxy"]);

    // Out-of-range index is an error.
    assert!(engine
        .move_routing_rule(&saved.id, 9, domain::routing::MoveDirection::Up)
        .is_err());

    // Boundary no-op succeeds.
    let top = engine
        .move_routing_rule(&saved.id, 0, domain::routing::MoveDirection::Top)
        .expect("noop");
    assert_eq!(top.rules().expect("rules").len(), 3);
}

#[test]
fn routing_dangling_reference_warns() {
    let engine = AppEngine::in_memory();
    // No profiles seeded: a remarks reference dangles.
    let saved = engine
        .save_routing(profile("warn", &[rule("r1", "my-node", Some("geosite:x"))]))
        .expect("save");
    let warnings = engine.routing_warnings(&saved.id).expect("warnings");
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].code, "routing_dangling_reference");

    // Built-in tags never warn.
    let clean = engine
        .save_routing(profile(
            "clean",
            &[
                rule("r1", "proxy", Some("geosite:x")),
                rule("r2", "direct", Some("geosite:y")),
                rule("r3", "block", Some("geosite:z")),
            ],
        ))
        .expect("save");
    assert!(engine.routing_warnings(&clean.id).expect("w").is_empty());
}

#[test]
fn routing_default_switch_and_delete_promotes() {
    let engine = AppEngine::in_memory();
    let schemes = engine.list_routings().expect("list");
    assert!(schemes.iter().any(|s| s.is_active));
    let first_active = schemes.iter().find(|s| s.is_active).unwrap().id.clone();

    let extra = engine.save_routing(profile("extra", &[])).expect("save");
    // Already-active is a conflict.
    assert!(engine.set_default_routing(&first_active).is_err());
    engine.set_default_routing(&extra.id).expect("switch");
    let active = engine.default_routing().expect("default").expect("some");
    assert_eq!(active.id, extra.id);

    // Deleting the active profile promotes another row.
    engine.delete_routing(&extra.id).expect("delete");
    let promoted = engine.default_routing().expect("default").expect("some");
    assert_ne!(promoted.id, extra.id);
    assert!(promoted.is_active);
}

#[test]
fn routing_import_export_round_trip() {
    let engine = AppEngine::in_memory();
    let saved = engine.save_routing(profile("rt", &[])).expect("save");
    let text = r#"[
        {"remarks": "one", "outboundTag": "proxy", "domain": ["geosite:google"]},
        {"remarks": "two", "outboundTag": "direct", "port": "80", "network": "tcp"}
    ]"#;
    let imported = engine
        .import_routing_rules(&saved.id, text, true)
        .expect("import");
    assert_eq!(imported.rule_num, 2);

    // Export selected: only the first rule.
    let rules = imported.rules().expect("rules");
    let exported = engine
        .export_routing_rules(&saved.id, Some(&[rules[0].id.clone()]))
        .expect("export");
    let back: Vec<serde_json::Value> = serde_json::from_str(&exported).expect("json");
    assert_eq!(back.len(), 1);

    // Full export re-imports cleanly (ids are cleared on export).
    let full = engine
        .export_routing_rules(&saved.id, None)
        .expect("full export");
    let replaced = engine
        .import_routing_rules(&saved.id, &full, true)
        .expect("reimport");
    assert_eq!(replaced.rule_num, 2);

    // Invalid JSON is rejected and the stored rules are untouched.
    assert!(engine
        .import_routing_rules(&saved.id, "not json", true)
        .is_err());
    assert_eq!(
        engine
            .get_routing(&saved.id)
            .expect("get")
            .expect("some")
            .rule_num,
        2
    );
}

async fn bind_loopback() -> TcpListener {
    for port in 11808..11950u16 {
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)).await {
            return listener;
        }
    }
    panic!("no free loopback port in 11808..11950");
}

#[tokio::test]
async fn routing_url_import_from_loopback() {
    let body = br#"[{"remarks": "via-url", "outboundTag": "proxy", "domain": ["geosite:github"]}]"#;
    let listener = bind_loopback().await;
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = vec![0u8; 4096];
            let _ = socket.read(&mut buf).await;
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = socket.write_all(header.as_bytes()).await;
            let _ = socket.write_all(body).await;
        }
    });

    let url = format!("http://127.0.0.1:{port}/rules.json");
    let text = application::routing::fetch_rules_text(&url, false, None)
        .await
        .expect("fetch");
    let engine = AppEngine::in_memory();
    let saved = engine.save_routing(profile("url", &[])).expect("save");
    let imported = engine
        .import_routing_rules(&saved.id, &text, true)
        .expect("import");
    assert_eq!(imported.rule_num, 1);
    assert_ne!(port, 10808);
    server.abort();
}

#[test]
fn regional_presets_default_and_offline() {
    let engine = AppEngine::in_memory();
    // Default resets and reports no pending URLs.
    let (pending, _) = engine
        .apply_regional_preset(RegionalPreset::Default)
        .expect("default");
    assert!(pending.is_empty());

    // Russia / Iran offline: URLs are set, embedded defaults keep generation
    // working, remote templates are reported pending (never faked).
    for preset in [RegionalPreset::RussiaOffline, RegionalPreset::IranOffline] {
        let (pending, _) = engine.apply_regional_preset(preset).expect("preset");
        assert_eq!(pending.len(), 3);
        assert!(pending.iter().all(|u| u.starts_with("https://")));
        let xray = engine
            .get_dns_for_core(CoreType::Xray)
            .expect("dns")
            .expect("xray row");
        assert!(xray.enabled);
        assert!(xray.normal_dns.as_ref().is_some_and(|s| !s.is_empty()));
    }
}

#[test]
fn dns_crud_and_validation() {
    let dir = tempfile::tempdir().expect("tempdir");
    let engine =
        AppEngine::open_with_runtime(dir.path(), Arc::new(application::NullRuntimeClient::new()))
            .expect("open");
    // Builtins: one row per core, both disabled.
    let items = engine.list_dns().expect("list");
    assert_eq!(items.len(), 2);

    // Xray accepts plain server lists and JSON with `servers`.
    let mut xray = engine
        .get_dns_for_core(CoreType::Xray)
        .expect("get")
        .expect("row");
    xray.normal_dns = Some("8.8.8.8,1.1.1.1".to_string());
    xray.enabled = true;
    engine.save_dns(xray).expect("save plain");

    // Xray rejects JSON-looking text without `servers`.
    let mut bad = engine
        .get_dns_for_core(CoreType::Xray)
        .expect("get")
        .expect("row");
    bad.normal_dns = Some(r#"{"hosts": {}}"#.to_string());
    assert!(engine.save_dns(bad).is_err());

    // sing-box requires Dns4Sbox shape with typed servers.
    let mut sbox = engine
        .get_dns_for_core(CoreType::SingBox)
        .expect("get")
        .expect("row");
    sbox.normal_dns = Some(r#"{"servers": []}"#.to_string());
    assert!(engine.save_dns(sbox).is_err());
    let mut sbox = engine
        .get_dns_for_core(CoreType::SingBox)
        .expect("get")
        .expect("row");
    sbox.normal_dns =
        Some(r#"{"servers": [{"tag": "remote", "type": "tcp", "server": "8.8.8.8"}]}"#.to_string());
    engine.save_dns(sbox).expect("save sbox");

    // Import-default fills both texts from the embedded templates.
    let filled = engine
        .import_default_dns(CoreType::Xray)
        .expect("import default");
    assert!(filled
        .normal_dns
        .as_ref()
        .is_some_and(|s| s.contains("servers")));
    assert!(filled.tun_dns.as_ref().is_some_and(|s| !s.is_empty()));

    // Reopen: DNS rows persist.
    drop(engine);
    let reopened =
        AppEngine::open_with_runtime(dir.path(), Arc::new(application::NullRuntimeClient::new()))
            .expect("reopen");
    assert_eq!(reopened.list_dns().expect("list").len(), 2);
    let xray = reopened
        .get_dns_for_core(CoreType::Xray)
        .expect("get")
        .expect("row");
    assert!(xray.enabled);
}

#[test]
fn dns_hosts_merge_semantics() {
    use std::collections::BTreeMap;
    let mut simple = domain::SimpleDnsItem::builtin();
    simple.hosts = Some("custom.example.com 93.184.216.34".to_string());
    simple.use_system_hosts = Some(true);
    let mut system = BTreeMap::new();
    system.insert("sys.example.com".to_string(), "192.0.2.9".to_string());
    // `dns.google` exists in both common and system maps: common wins
    // (TryAdd never overwrites), custom always overwrites.
    system.insert("dns.google".to_string(), "9.9.9.9".to_string());
    let merged = domain::dns::merge_hosts(&simple, &system);
    assert!(merged.contains_key("custom.example.com"));
    assert!(merged.contains_key("sys.example.com"));
    assert_eq!(
        merged.get("dns.google").expect("common"),
        &vec![
            "8.8.8.8".to_string(),
            "8.8.4.4".to_string(),
            "2001:4860:4860::8888".to_string(),
            "2001:4860:4860::8844".to_string(),
        ]
    );
}

#[test]
fn rule_mode_switch_persists() {
    let dir = tempfile::tempdir().expect("tempdir");
    let engine =
        AppEngine::open_with_runtime(dir.path(), Arc::new(application::NullRuntimeClient::new()))
            .expect("open");
    assert_eq!(engine.rule_mode(), RuleMode::Rule);
    engine.set_rule_mode(RuleMode::Global).expect("set");
    drop(engine);
    let reopened =
        AppEngine::open_with_runtime(dir.path(), Arc::new(application::NullRuntimeClient::new()))
            .expect("reopen");
    assert_eq!(reopened.rule_mode(), RuleMode::Global);
    reopened.set_rule_mode(RuleMode::Direct).expect("set");
    assert_eq!(reopened.rule_mode(), RuleMode::Direct);
}

#[test]
fn builtin_routing_templates_parse() {
    // The three embedded upstream templates must parse to non-empty rules.
    let builtins = domain::routing::builtin_profiles();
    assert_eq!(builtins.len(), 3);
    for (profile, rules) in &builtins {
        assert!(!rules.is_empty(), "{}", profile.remarks);
        for rule in rules {
            domain::routing::validate_rule(rule).expect("valid rule");
        }
    }
    // Unused import keepalive: DnsRepository/RoutingRepository are exercised
    // through the engine above; reference the traits explicitly.
    fn _assert_traits<T: RoutingRepository, U: DnsRepository>() {}
    let _ = _assert_traits::<
        application::InMemoryRoutingRepository,
        application::InMemoryDnsRepository,
    >;
    let _ = DnsProfile::default();
}
