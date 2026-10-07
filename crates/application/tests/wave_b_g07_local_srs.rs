//! Wave B (local-only): FLD-CFG-086 / G-07 producer -> E2E gap closure.
//!
//! An engine opened at a temp data dir with `bin/srss/geosite-*.srs` files +
//! a routing rule referencing one of them must (a) pick up the local snapshot
//! in `runtime_codegen_options`, (b) emit a sing-box config whose rule_set for
//! that tag is `local` (path under `bin/srss/`), with the xray input carrying
//! the same snapshot + `ruleset_url`, and (c) stay stable across reopen.
//!
//! Synthetic fixtures only. No kernel is started; every emitted port is
//! pre-probed free and `>= 11808` (never 10808). No network access.

use std::sync::Arc;

use application::{AppEngine, NullRuntimeClient};
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

fn open_engine(dir: &std::path::Path) -> AppEngine {
    AppEngine::open_with_runtime(dir, Arc::new(NullRuntimeClient::new())).expect("open engine")
}

fn vless_leaf(id: &str) -> Profile {
    let mut profile = Profile {
        index_id: id.into(),
        config_type: ConfigType::Vless,
        core_type: Some(CoreType::SingBox),
        remarks: id.into(),
        address: "192.0.2.10".into(),
        port: 443,
        password: "11111111-2222-3333-4444-555555555555".into(),
        ..Default::default()
    };
    profile.proto_extra.vless_encryption = Some("none".into());
    profile
}

fn geosite_rule(id: &str, outbound: &str, domain: &str) -> RoutingRule {
    RoutingRule {
        id: id.into(),
        outbound_tag: Some(outbound.into()),
        domain: Some(vec![domain.into()]),
        enabled: true,
        remarks: Some(id.into()),
        rule_type: Some(domain::RuleType::Routing),
        ..Default::default()
    }
}

/// Point the stored inbound base port at a pre-probed free port so the
/// generator guard (never 10808) passes and `runtime_codegen_options` derives
/// loopback-safe ports.
fn set_base_port(engine: &AppEngine, port: u16) {
    let loaded = engine.load_settings().expect("load settings");
    let mut settings = loaded.settings;
    if let Some(first) = settings.inbound.first_mut() {
        first.local_port = i32::from(port);
    }
    engine
        .save_settings(settings, loaded.revision)
        .expect("save settings");
}

fn seed(engine: &AppEngine, base: u16) {
    let revision = engine.desired_revision();
    engine
        .save_profile(vless_leaf("n1"), DesiredRevision::new(revision))
        .expect("save node");
    let mut scheme = RoutingProfile {
        remarks: "wave-b-g07".into(),
        ..Default::default()
    };
    scheme
        .set_rules(&[geosite_rule("r1", "proxy", "geosite:google")])
        .expect("set rules");
    let saved = engine.save_routing(scheme).expect("save routing");
    engine
        .set_default_routing(&saved.id)
        .expect("set default routing");
    set_base_port(engine, base);
}

fn assert_singbox_local_emission(main: &serde_json::Value) {
    let rule_sets = main
        .pointer("/route/rule_set")
        .and_then(serde_json::Value::as_array)
        .expect("route.rule_set array");
    let entry = rule_sets
        .iter()
        .find(|entry| {
            entry.get("tag").and_then(serde_json::Value::as_str) == Some("geosite-google")
        })
        .expect("geosite-google rule_set entry");
    assert_eq!(
        entry.get("type").and_then(serde_json::Value::as_str),
        Some("local"),
        "local snapshot must select a local rule_set: {entry}"
    );
    let path = entry
        .get("path")
        .and_then(serde_json::Value::as_str)
        .expect("local rule_set path");
    assert!(
        path.contains("srss") && path.contains("geosite-google.srs"),
        "local path must reference bin/srss/geosite-google.srs: {path}"
    );
    assert!(
        !rule_sets.iter().any(|entry| {
            entry.get("tag").and_then(serde_json::Value::as_str) == Some("geosite-google")
                && entry.get("type").and_then(serde_json::Value::as_str) == Some("remote")
        }),
        "snapshot tag must not also emit a remote rule_set"
    );
}

#[test]
fn wave_b_g07_local_srs_snapshot_reaches_emission_and_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Synthetic local SRS files; the `.txt` decoy must be ignored.
    let srss = dir.path().join("bin").join("srss");
    std::fs::create_dir_all(&srss).expect("srss dir");
    std::fs::write(srss.join("geosite-google.srs"), b"wave-b-synthetic-srs").expect("srs file");
    std::fs::write(srss.join("notes.txt"), b"not-an-srs").expect("decoy file");

    let engine = open_engine(dir.path());
    seed(&engine, free_port(11808));

    // (a) The producer picks up the local snapshot.
    let opts = engine.runtime_codegen_options();
    assert!(
        opts.local_srs_files.contains("geosite-google"),
        "snapshot missing geosite-google: {:?}",
        opts.local_srs_files
    );
    assert_eq!(
        opts.local_srs_files.len(),
        1,
        "only *.srs stems belong in the snapshot: {:?}",
        opts.local_srs_files
    );

    // (b) sing-box: the codegen input carries the snapshot and emits local.
    let input = engine
        .build_codegen_input("n1", CoreType::SingBox, &opts)
        .expect("sing-box input");
    assert!(
        input.settings.local_srs_files.contains("geosite-google"),
        "input must carry the snapshot"
    );
    assert!(
        input
            .settings
            .ruleset_url
            .as_deref()
            .is_some_and(|url| !url.is_empty()),
        "srs source template must reach generation"
    );
    let generated = application::codegen::generate(CoreType::SingBox, &input).expect("generate");
    assert_singbox_local_emission(&generated.main);

    // (b) xray where applicable: same input snapshot + ruleset_url, domain
    // passthrough intact (xray has no local rule_set concept).
    let xray_input = engine
        .build_codegen_input("n1", CoreType::Xray, &opts)
        .expect("xray input");
    assert!(
        xray_input
            .settings
            .local_srs_files
            .contains("geosite-google"),
        "xray input must carry the same snapshot"
    );
    let xray_generated =
        application::codegen::generate(CoreType::Xray, &xray_input).expect("xray generate");
    let xray_rules = xray_generated
        .main
        .pointer("/routing/rules")
        .and_then(serde_json::Value::as_array)
        .expect("routing.rules array");
    assert!(
        xray_rules.iter().any(|rule| {
            rule.get("domain")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|domains| {
                    domains
                        .iter()
                        .any(|domain| domain.as_str() == Some("geosite:google"))
                })
        }),
        "xray must keep the geosite domain rule: {}",
        xray_generated
            .main
            .pointer("/routing/rules")
            .unwrap_or(&serde_json::Value::Null)
    );

    // (c) Stable across reopen: same snapshot, same local emission.
    drop(engine);
    drop(opts);
    drop(input);
    let reopened = open_engine(dir.path());
    let reopened_opts = reopened.runtime_codegen_options();
    assert!(
        reopened_opts.local_srs_files.contains("geosite-google"),
        "snapshot must survive reopen: {:?}",
        reopened_opts.local_srs_files
    );
    let reopened_input = reopened
        .build_codegen_input("n1", CoreType::SingBox, &reopened_opts)
        .expect("reopen input");
    let reopened_generated = application::codegen::generate(CoreType::SingBox, &reopened_input)
        .expect("reopen generate");
    assert_singbox_local_emission(&reopened_generated.main);
}
