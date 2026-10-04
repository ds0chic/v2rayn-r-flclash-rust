//! R3-PROF-11: a Reality node saved (or imported) with an empty fingerprint
//! freezes the then-current `CoreBasicItem.DefFingerprint` into the profile,
//! mirroring the frozen `ConfigHandler.AddServerCommon:1214-1216`. Changing the
//! configured default afterwards must not alter already-saved nodes, across a
//! reopen.
//!
//! Synthetic data only (RFC 5737 address, fake UUID); temp SQLite, no network.

use std::sync::Arc;

use application::runtime_client::NullRuntimeClient;
use application::AppEngine;
use domain::{ConfigType, DesiredRevision, Profile};

fn open(dir: &std::path::Path) -> AppEngine {
    AppEngine::open_with_runtime(dir, Arc::new(NullRuntimeClient::new())).expect("open engine")
}

fn desired(engine: &AppEngine) -> DesiredRevision {
    engine.snapshot().unwrap().revisions.desired
}

fn set_default_fingerprint(engine: &AppEngine, value: Option<&str>) {
    let loaded = engine.load_settings().unwrap();
    let mut settings = loaded.settings;
    settings.core_basic_item.def_fingerprint = value.map(str::to_string);
    engine
        .save_settings(settings, loaded.revision)
        .expect("save settings");
}

fn reality_draft(id: &str) -> Profile {
    let mut profile = Profile {
        index_id: id.into(),
        config_type: ConfigType::Vless,
        remarks: "r3-prof-11".into(),
        address: "192.0.2.10".into(),
        port: 443,
        password: "11111111-2222-3333-4444-555555555555".into(),
        ..Default::default()
    };
    profile.security.stream_security = Some("reality".into());
    profile
}

#[test]
fn save_freezes_the_default_and_ignores_later_change() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open(dir.path());
    set_default_fingerprint(&engine, Some("chrome"));

    let (saved, _) = engine
        .save_profile(reality_draft("r3p11-save"), desired(&engine))
        .unwrap();
    assert_eq!(saved.security.fingerprint.as_deref(), Some("chrome"));

    // Changing the configured default must not touch the already-saved node.
    set_default_fingerprint(&engine, Some("firefox"));
    let loaded = engine.profile_by_id("r3p11-save").unwrap().unwrap();
    assert_eq!(
        loaded.security.fingerprint.as_deref(),
        Some("chrome"),
        "an already-saved Reality node keeps its frozen fingerprint"
    );
}

#[test]
fn frozen_fingerprint_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    {
        let engine = open(dir.path());
        set_default_fingerprint(&engine, Some("chrome"));
        let (saved, _) = engine
            .save_profile(reality_draft("r3p11-reopen"), desired(&engine))
            .unwrap();
        assert_eq!(saved.security.fingerprint.as_deref(), Some("chrome"));
    }
    {
        // Reopen, then change the default: the persisted node is unchanged.
        let engine = open(dir.path());
        set_default_fingerprint(&engine, Some("safari"));
        let loaded = engine.profile_by_id("r3p11-reopen").unwrap().unwrap();
        assert_eq!(loaded.security.fingerprint.as_deref(), Some("chrome"));
    }
    // A third open still reads the frozen value.
    let engine = open(dir.path());
    let loaded = engine.profile_by_id("r3p11-reopen").unwrap().unwrap();
    assert_eq!(loaded.security.fingerprint.as_deref(), Some("chrome"));
}

#[test]
fn import_freezes_the_default_too() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open(dir.path());
    set_default_fingerprint(&engine, Some("chrome"));

    let (saved, _) = engine
        .save_imported_profile(reality_draft("r3p11-import"), desired(&engine))
        .unwrap();
    assert_eq!(saved.security.fingerprint.as_deref(), Some("chrome"));
}

#[test]
fn explicit_fingerprint_and_non_reality_are_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open(dir.path());
    set_default_fingerprint(&engine, Some("chrome"));

    let mut explicit = reality_draft("r3p11-explicit");
    explicit.security.fingerprint = Some("firefox".into());
    let (saved, _) = engine.save_profile(explicit, desired(&engine)).unwrap();
    assert_eq!(saved.security.fingerprint.as_deref(), Some("firefox"));

    let mut tls = reality_draft("r3p11-tls");
    tls.security.stream_security = Some("tls".into());
    tls.security.fingerprint = None;
    let (saved_tls, _) = engine.save_profile(tls, desired(&engine)).unwrap();
    assert!(
        saved_tls.security.fingerprint.is_none(),
        "the default fingerprint is only frozen for Reality nodes"
    );
}
