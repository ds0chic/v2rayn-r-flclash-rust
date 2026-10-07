//! Wave A acceptance (SP-23, 2026-10-07) for FLD-CFG-093/094:
//! `CoreTypeItem[].ConfigType -> CoreType` binding drives `resolve_target_core`
//! (explicit node core wins; unbound config types fall back to Xray; the
//! binding survives reopen). Synthetic rows only; no kernel, no OS state.

use std::sync::Arc;

use application::{AppEngine, NullRuntimeClient};
use domain::{ConfigType, CoreType, CoreTypeBinding, DesiredRevision, Profile};

fn engine_at(dir: &std::path::Path) -> AppEngine {
    AppEngine::open_with_runtime(dir, Arc::new(NullRuntimeClient::new())).expect("open engine")
}

fn save_profile(engine: &AppEngine, profile: Profile) -> Profile {
    let revision = engine.desired_revision();
    engine
        .save_profile(profile, DesiredRevision::new(revision))
        .expect("save profile")
        .0
}

fn leaf(id: &str, config_type: ConfigType, core: Option<CoreType>) -> Profile {
    let mut profile = Profile {
        index_id: id.into(),
        config_type,
        core_type: core,
        remarks: id.into(),
        address: "192.0.2.10".into(),
        port: 443,
        password: "11111111-2222-3333-4444-555555555555".into(),
        ..Default::default()
    };
    if config_type == ConfigType::Vless {
        profile.proto_extra.vless_encryption = Some("none".into());
    }
    profile
}

fn save_bindings(engine: &AppEngine, bindings: Vec<CoreTypeBinding>) {
    let loaded = engine.load_settings().expect("load settings");
    let mut settings = loaded.settings;
    settings.core_type_item = Some(bindings);
    settings.init_core_type_items();
    engine
        .save_settings(settings, loaded.revision)
        .expect("save bindings");
}

#[test]
fn fld093_binding_decides_until_explicit_core_wins_and_survives_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let engine = engine_at(dir.path());
    save_bindings(
        &engine,
        vec![CoreTypeBinding::new(ConfigType::Vless, CoreType::SingBox)],
    );
    let bound = save_profile(&engine, leaf("bound", ConfigType::Vless, None));
    let explicit = save_profile(
        &engine,
        leaf("explicit", ConfigType::Vless, Some(CoreType::Xray)),
    );

    assert_eq!(
        engine.resolve_target_core(&bound).expect("resolve bound"),
        CoreType::SingBox,
        "the canonical binding decides when the row has no explicit core"
    );
    assert_eq!(
        engine
            .resolve_target_core(&explicit)
            .expect("resolve explicit"),
        CoreType::Xray,
        "an explicit node core wins over the binding"
    );

    drop(engine);
    let reopened = engine_at(dir.path());
    let bound = reopened
        .profile_by_id("bound")
        .expect("read profile")
        .expect("bound exists");
    assert_eq!(
        reopened
            .resolve_target_core(&bound)
            .expect("resolve after reopen"),
        CoreType::SingBox,
        "the binding survives reopen (save -> load)"
    );
}

#[test]
fn fld094_unbound_config_type_falls_back_to_xray() {
    let dir = tempfile::tempdir().expect("tempdir");
    let engine = engine_at(dir.path());
    save_bindings(
        &engine,
        vec![CoreTypeBinding::new(ConfigType::Vless, CoreType::SingBox)],
    );
    let trojan = save_profile(&engine, leaf("trojan", ConfigType::Trojan, None));
    assert_eq!(
        engine.resolve_target_core(&trojan).expect("resolve trojan"),
        CoreType::Xray,
        "a config type without a binding keeps the upstream Xray default"
    );
}
