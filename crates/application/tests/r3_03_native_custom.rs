//! R3-03: a `Custom` node on a non-Xray/sing-box core carries a native config
//! file (mihomo YAML via the SP-24 G-06 merge, naive args file, mieru JSON via
//! `MIERU_CONFIG_JSON_FILE`, ...). Non-mihomo native content stays verbatim
//! instead of being re-serialised as JSON or rejected through the Xray
//! `inbounds` parser. Only Xray-family / sing-box Custom configs are
//! endpoint-parsed.
//!
//! Pure plan assertions: no core or OS is touched.

use std::sync::Arc;

use application::{AppEngine, NullRuntimeClient};
use domain::{ConfigSource, ConfigType, CoreType, DesiredRevision, Profile};

fn engine() -> AppEngine {
    AppEngine::with_runtime(Arc::new(NullRuntimeClient::new()))
}

fn save_native(engine: &AppEngine, core: CoreType, raw: &str) -> Profile {
    let mut profile = Profile {
        index_id: "native".into(),
        config_type: ConfigType::Custom,
        core_type: Some(core),
        remarks: "native".into(),
        address: "inline".into(),
        ..Default::default()
    };
    profile.proto_extra.extra.insert(
        application::codegen::CUSTOM_CONFIG_KEY.to_string(),
        serde_json::json!(raw),
    );
    let revision = engine.desired_revision();
    engine
        .save_profile(profile, DesiredRevision::new(revision))
        .expect("save native custom profile")
        .0
}

fn plan_body(engine: &AppEngine, core: CoreType, raw: &str) -> (CoreType, String) {
    save_native(engine, core, raw);
    let revision = engine.desired_revision();
    let plan = engine
        .build_runtime_plan("native", revision)
        .unwrap_or_else(|error| panic!("native {core:?} plan must build: {error:?}"));
    let body = match plan.target.config {
        ConfigSource::Inline { body } => body,
        _ => panic!("inline config"),
    };
    (plan.target.core_type, body)
}

#[test]
fn mihomo_yaml_custom_merges_runtime_rewrites() {
    // SP-24 G-06 supersedes verbatim passthrough for mihomo: the native YAML
    // passes through `mihomo_body_for_plan` (runtime rewrites + TUN/mixin
    // merge) before it is persisted, while unknown keys are retained.
    let engine = engine();
    let revision = engine.load_settings().unwrap().revision;
    let mut settings = engine.load_settings().unwrap().settings;
    if let Some(inbound) = settings.inbound.first_mut() {
        inbound.local_port = 11880;
    }
    engine.save_settings(settings, revision).unwrap();
    let raw = "mixed-port: 11880\nmode: rule\nproxies: []\n";
    let (core, body) = plan_body(&engine, CoreType::Mihomo, raw);
    assert_eq!(core, CoreType::Mihomo);
    assert!(
        body.contains("mixed-port: 11880"),
        "local port rewrite: {body}"
    );
    assert!(body.contains("mode: rule"), "existing keys kept: {body}");
    assert!(body.contains("proxies"), "unknown keys retained: {body}");
    assert!(!body.contains("10808"), "never the live proxy port: {body}");
}

#[test]
fn naive_custom_is_preserved_verbatim() {
    let raw = r#"{"listen":"socks://127.0.0.1:11881","proxy":"https://example.com"}"#;
    let (core, body) = plan_body(&engine(), CoreType::NaiveProxy, raw);
    assert_eq!(core, CoreType::NaiveProxy);
    assert_eq!(body, raw);
}

#[test]
fn mieru_custom_is_preserved_verbatim() {
    let raw = r#"{"portBindings":[{"port":11882,"protocol":"TCP"}]}"#;
    let (core, body) = plan_body(&engine(), CoreType::Mieru, raw);
    assert_eq!(core, CoreType::Mieru);
    assert_eq!(body, raw);
}

#[test]
fn xray_custom_without_proxy_inbound_is_still_rejected() {
    // The structured Xray path stays strict: a JSON Custom config with no
    // socks/http inbound remains a hard plan error (not silently accepted).
    let engine = engine();
    save_native(&engine, CoreType::Xray, r#"{"inbounds":[],"outbounds":[]}"#);
    let revision = engine.desired_revision();
    let error = engine
        .build_runtime_plan("native", revision)
        .expect_err("xray custom without inbound must fail");
    assert_eq!(error.message_key, "error.custom_endpoint_parse_failed");
}
