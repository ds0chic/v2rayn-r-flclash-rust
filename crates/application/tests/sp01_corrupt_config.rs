//! SP-01 red contract: corrupt `guiNConfig.json` must fail closed.
//!
//! Correct expectations (stable-port target, stricter than the frozen
//! upstream `ConfigHandler.LoadConfig`, which whole-tree defaults on any
//! `Deserialize` failure):
//!
//! - missing file  -> first-run init with defaults (success);
//! - present-but-empty / whitespace-only -> structured corrupt error;
//! - truncated JSON -> structured corrupt error;
//! - known-field type error (`GuiItem.TrayMenuServersLimit`) -> structured
//!   corrupt error; the other legal fields and unknown keys are NOT silently
//!   replaced by defaults and the source file is NOT overwritten;
//! - explicit `null` group == missing group (upstream `??=` semantics);
//! - missing groups get defaults; unknown root/nested keys survive a
//!   save + independent reopen round trip.
//!
//! All fixtures are synthetic (ports >= 11808, documentation addresses only).
//! No sockets, cores, helpers or OS writes are involved.

use std::sync::Arc;

use application::runtime_client::NullRuntimeClient;
use application::AppEngine;

const TRAY_LIMIT_BAD: &str = r#"{
    "UiItem": {"CurrentLanguage": "en"},
    "GuiItem": {"KeepOlderDedupl": true, "TrayMenuServersLimit": "oops-not-a-number"},
    "Inbound": [{"LocalPort": 11808, "Protocol": 0, "UdpEnabled": true}],
    "FutureRoot": {"x": [1, 2, 3]}
}"#;

fn open_engine(dir: &std::path::Path) -> AppEngine {
    AppEngine::open_with_runtime(dir, Arc::new(NullRuntimeClient::new()))
        .unwrap_or_else(|error| panic!("open must succeed: {error}"))
}

fn open_err(dir: &std::path::Path, case: &str) -> domain::DomainError {
    match AppEngine::open_with_runtime(dir, Arc::new(NullRuntimeClient::new())) {
        Ok(_) => panic!("{case} must fail"),
        Err(err) => err,
    }
}

fn write_config(dir: &std::path::Path, text: &str) {
    std::fs::write(dir.join("guiNConfig.json"), text).unwrap();
}

fn read_raw(dir: &std::path::Path) -> String {
    std::fs::read_to_string(dir.join("guiNConfig.json")).unwrap()
}

#[test]
fn missing_file_initialises_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open_engine(dir.path());
    let loaded = engine.load_settings().expect("load after init");
    assert_eq!(loaded.settings.gui_item.tray_menu_servers_limit, 20);
    assert_eq!(
        loaded.settings.ui_item.current_language.as_deref(),
        Some("zh-Hans")
    );
}

#[test]
fn present_empty_file_is_corrupt_not_default() {
    let dir = tempfile::tempdir().unwrap();
    write_config(dir.path(), "");
    let before = read_raw(dir.path());
    let err = open_err(dir.path(), "empty file");
    assert_eq!(err.code, domain::codes::FIELD_FORMAT, "{err}");
    assert_eq!(err.message_key, "error.config_corrupt", "{err}");
    assert_eq!(read_raw(dir.path()), before, "source must not be touched");
}

#[test]
fn whitespace_only_file_is_corrupt() {
    let dir = tempfile::tempdir().unwrap();
    write_config(dir.path(), "  \n\t  ");
    let err = open_err(dir.path(), "blank file");
    assert_eq!(err.message_key, "error.config_corrupt", "{err}");
}

#[test]
fn truncated_json_is_corrupt_and_preserved() {
    let dir = tempfile::tempdir().unwrap();
    write_config(dir.path(), r#"{"GuiItem": {"TrayMenuServersLimit": "#);
    let before = read_raw(dir.path());
    let err = open_err(dir.path(), "truncated file");
    assert_eq!(err.message_key, "error.config_corrupt", "{err}");
    assert_eq!(read_raw(dir.path()), before, "source must not be touched");
}

#[test]
fn bad_tray_limit_type_fails_without_defaulting_rest() {
    let dir = tempfile::tempdir().unwrap();
    write_config(dir.path(), TRAY_LIMIT_BAD);
    let before = read_raw(dir.path());
    let err = open_err(dir.path(), "bad field type");
    assert_eq!(err.code, domain::codes::FIELD_FORMAT, "{err}");
    assert_eq!(err.message_key, "error.config_corrupt", "{err}");
    // The legal neighbours and the unknown key must still be on disk,
    // untouched: no silent whole-tree defaulting, no overwrite.
    let after = read_raw(dir.path());
    assert_eq!(after, before);
    assert!(after.contains("\"CurrentLanguage\": \"en\""));
    assert!(after.contains("\"FutureRoot\""));
    assert!(after.contains("11808"));
}

#[test]
fn explicit_null_group_behaves_like_missing() {
    let dir = tempfile::tempdir().unwrap();
    write_config(dir.path(), r#"{"GuiItem": null, "KcpItem": null}"#);
    let engine = open_engine(dir.path());
    let loaded = engine.load_settings().unwrap();
    assert_eq!(loaded.settings.gui_item.tray_menu_servers_limit, 20);
    assert_eq!(loaded.settings.kcp_item.mtu, 1350);
}

#[test]
fn empty_object_gets_full_defaults() {
    let dir = tempfile::tempdir().unwrap();
    write_config(dir.path(), "{}");
    let engine = open_engine(dir.path());
    let loaded = engine.load_settings().unwrap();
    assert_eq!(loaded.settings.inbound[0].local_port, 10808);
    assert_eq!(
        loaded.settings.ui_item.current_language.as_deref(),
        Some("zh-Hans")
    );
}

#[test]
fn unknown_keys_survive_save_and_independent_reopen() {
    let dir = tempfile::tempdir().unwrap();
    write_config(
        dir.path(),
        r#"{
            "UiItem": {"CurrentLanguage": "en"},
            "GuiItem": {"KeepOlderDedupl": true, "TrayMenuServersLimit": 25, "FutureToggle": 7},
            "Inbound": [{"LocalPort": 11808, "Protocol": 0, "UdpEnabled": true}],
            "FutureRoot": {"x": [1, 2, 3]}
        }"#,
    );
    let engine = open_engine(dir.path());
    let loaded = engine.load_settings().unwrap();
    assert_eq!(loaded.settings.gui_item.tray_menu_servers_limit, 25);
    assert!(loaded.settings.gui_item.extra.contains_key("FutureToggle"));
    assert!(loaded.settings.extra.contains_key("FutureRoot"));
    // Whole-tree save then a fully independent reopen keeps everything.
    engine
        .save_settings(loaded.settings.clone(), loaded.revision)
        .expect("save valid doc");
    drop(engine);
    let reopened = open_engine(dir.path());
    let again = reopened.load_settings().unwrap();
    assert_eq!(again.settings.gui_item.tray_menu_servers_limit, 25);
    assert_eq!(
        again.settings.ui_item.current_language.as_deref(),
        Some("en")
    );
    assert_eq!(again.settings.inbound[0].local_port, 11808);
    assert!(again.settings.gui_item.extra.contains_key("FutureToggle"));
    assert!(again.settings.extra.contains_key("FutureRoot"));
}

#[test]
fn empty_string_null_and_missing_stay_distinct() {
    let dir = tempfile::tempdir().unwrap();
    write_config(
        dir.path(),
        r#"{"SubIndexId": "", "CoreBasicItem": {"LogEnabled": true, "Loglevel": null}}"#,
    );
    let engine = open_engine(dir.path());
    let loaded = engine.load_settings().unwrap();
    assert_eq!(loaded.settings.sub_index_id.as_deref(), Some(""));
    assert_eq!(loaded.settings.core_basic_item.loglevel, None);
    let dir2 = tempfile::tempdir().unwrap();
    write_config(dir2.path(), "{}");
    let engine2 = open_engine(dir2.path());
    assert_eq!(engine2.load_settings().unwrap().settings.sub_index_id, None);
}

#[test]
fn corrupt_file_can_be_repaired_and_reopened() {
    let dir = tempfile::tempdir().unwrap();
    write_config(dir.path(), TRAY_LIMIT_BAD);
    open_err(dir.path(), "corrupt open");
    // The user (or a recovery flow) repairs the file out of band; a fresh
    // open retries the read instead of being poisoned by the first failure.
    write_config(
        dir.path(),
        r#"{"UiItem": {"CurrentLanguage": "en"}, "GuiItem": {"TrayMenuServersLimit": 25}}"#,
    );
    let engine = open_engine(dir.path());
    let loaded = engine.load_settings().unwrap();
    assert_eq!(loaded.settings.gui_item.tray_menu_servers_limit, 25);
    assert_eq!(
        loaded.settings.ui_item.current_language.as_deref(),
        Some("en")
    );
}
