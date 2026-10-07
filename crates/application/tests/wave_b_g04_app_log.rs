//! Wave B (FLD-CFG-063 / G-04): app diagnostic logger consumer.
//!
//! The canonical `GuiItem.EnableLog` (upstream `GUIItem.EnableLog`, consumed
//! by `AppManager` via `Logging.LoggingEnabled`) starts/stops the file logger
//! in `crates/application` with date rotation, size rotation and redaction.
//! All fixtures are synthetic; all writes stay inside `tempfile` dirs.

use std::sync::Arc;

use application::app_log::{
    enabled_from_settings, redact_line, ymd_from_unix, AppLogService, GUI_LOG_DIR_NAME, REDACTED,
};
use application::recoverable_commit::{
    commit_id_for, committed_receipt, document_token, parse_receipt, rejected_receipt,
    settings_hash_of,
};
use application::runtime_client::NullRuntimeClient;
use application::AppEngine;
use domain::codes;

const SECRET_A: &str = "SYNTH-SECRET-AAA-111";
const SECRET_B: &str = "SYNTH-SECRET-BBB-222";
const SECRET_C: &str = "SYNTH-SECRET-CCC-333";

fn engine_in_temp() -> (tempfile::TempDir, AppEngine) {
    let dir = tempfile::tempdir().unwrap();
    let engine =
        AppEngine::open_with_runtime(dir.path(), Arc::new(NullRuntimeClient::new())).unwrap();
    (dir, engine)
}

/// Whole-`GuiItem` patch with `EnableLog` set, built from the live document
/// (the group patch replaces the group, so partial patches would reset kin).
fn gui_patch_with_enable_log(engine: &AppEngine, enabled: bool) -> (serde_json::Value, u64) {
    let loaded = engine.load_settings().unwrap();
    let mut patch = serde_json::to_value(&loaded.settings.gui_item).unwrap();
    patch["EnableLog"] = serde_json::Value::Bool(enabled);
    let revision = loaded.group_revisions.get("GuiItem").copied().unwrap_or(0);
    (patch, revision)
}

#[test]
fn disabled_logger_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let service = AppLogService::new();
    let root = dir.path().join(GUI_LOG_DIR_NAME);
    let wrote = service
        .append(&root, "2026-10-07", false, "startup line")
        .unwrap();
    assert!(!wrote);
    assert!(!root.exists(), "disabled logger must not touch the fs");
}

#[test]
fn enabled_logger_starts_and_stops_with_settings() {
    let (_dir, engine) = engine_in_temp();
    assert!(
        engine.app_log_enabled(),
        "upstream default is EnableLog=true"
    );

    assert!(engine.write_app_log("synth hello").unwrap());
    let log_file = engine_log_file(&_dir);
    assert!(log_file.exists());

    let (patch, rev) = gui_patch_with_enable_log(&engine, false);
    engine.save_settings_group("GuiItem", patch, rev).unwrap();
    assert!(!engine.app_log_enabled());

    let before = std::fs::read_to_string(&log_file).unwrap();
    assert!(!engine.write_app_log("must not be written").unwrap());
    assert_eq!(std::fs::read_to_string(&log_file).unwrap(), before);

    let (patch, rev) = gui_patch_with_enable_log(&engine, true);
    engine.save_settings_group("GuiItem", patch, rev).unwrap();
    assert!(engine.app_log_enabled());
    assert!(engine.write_app_log("synth resumed").unwrap());
    assert!(std::fs::read_to_string(&log_file)
        .unwrap()
        .contains("synth resumed"));
}

fn engine_log_file(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let today = application::app_log::today_ymd_now();
    dir.path()
        .join(GUI_LOG_DIR_NAME)
        .join(format!("{today}.txt"))
}

#[test]
fn save_failure_keeps_previous_logger_state() {
    let (_dir, engine) = engine_in_temp();
    assert!(engine.app_log_enabled());

    // Stale group revision: rejected before touching storage or the logger.
    let (patch, _) = gui_patch_with_enable_log(&engine, false);
    let err = engine
        .save_settings_group("GuiItem", patch, 999)
        .unwrap_err();
    assert_eq!(err.code, codes::REVISION_STALE);
    assert!(engine.app_log_enabled());
    assert!(engine.write_app_log("still on").unwrap());

    // Validation failure on the whole-tree path: same guarantee.
    let loaded = engine.load_settings().unwrap();
    let mut bad = loaded.settings.clone();
    bad.inbound[0].local_port = 0;
    let err = engine.save_settings(bad, loaded.revision).unwrap_err();
    assert_eq!(err.code, codes::FIELD_RANGE);
    assert!(engine.app_log_enabled());

    // Only a successful save flips the switch.
    let (patch, rev) = gui_patch_with_enable_log(&engine, false);
    engine.save_settings_group("GuiItem", patch, rev).unwrap();
    assert!(!engine.app_log_enabled());
    assert!(!enabled_from_settings(
        &engine.load_settings().unwrap().settings
    ));
}

#[test]
fn rotation_archives_and_caps_backups() {
    let dir = tempfile::tempdir().unwrap();
    let service = AppLogService::with_limits(64, 2);
    let root = dir.path().join(GUI_LOG_DIR_NAME);
    for i in 0..20 {
        assert!(service
            .append(
                &root,
                "2026-10-07",
                true,
                &format!("synth line {i:02} padding-x")
            )
            .unwrap());
    }
    let current = root.join("2026-10-07.txt");
    assert!(current.exists());
    assert!(root.join("2026-10-07.txt.1").exists());
    assert!(root.join("2026-10-07.txt.2").exists());
    assert!(
        !root.join("2026-10-07.txt.3").exists(),
        "archives beyond keep_rotated must be cleaned up"
    );
}

#[test]
fn date_change_starts_new_file() {
    let dir = tempfile::tempdir().unwrap();
    let service = AppLogService::new();
    let root = dir.path().join(GUI_LOG_DIR_NAME);
    service
        .append(&root, "2026-10-06", true, "synth day one")
        .unwrap();
    service
        .append(&root, "2026-10-07", true, "synth day two")
        .unwrap();
    assert!(root.join("2026-10-06.txt").exists());
    assert!(root.join("2026-10-07.txt").exists());
    assert!(!std::fs::read_to_string(root.join("2026-10-07.txt"))
        .unwrap()
        .contains("day one"));
}

#[test]
fn redaction_matrix() {
    // URI userinfo.
    let line = format!("fetch https://user:{SECRET_A}@example.com/sub done");
    let redacted = redact_line(&line);
    assert!(!redacted.contains(SECRET_A));
    assert!(redacted.contains(REDACTED));

    // Node-link tokens of every covered scheme.
    for scheme in [
        "vmess://",
        "vless://",
        "ss://",
        "trojan://",
        "tuic://",
        "hysteria2://",
    ] {
        let line = format!("node {scheme}{SECRET_B}");
        let redacted = redact_line(&line);
        assert!(!redacted.contains(SECRET_B), "{scheme}");
        assert!(
            redacted.contains(&format!("{scheme}{REDACTED}")),
            "{scheme}"
        );
    }

    // key=value / JSON / colon forms.
    for line in [
        format!("login password={SECRET_C} ok"),
        format!("{{\"token\": \"{SECRET_C}\"}}"),
        format!("Password: {SECRET_C}"),
    ] {
        let redacted = redact_line(&line);
        assert!(!redacted.contains(SECRET_C), "{line}");
        assert!(redacted.contains(REDACTED), "{line}");
    }

    // Benign content is preserved.
    let line = "startup ok port=11808 remarks=synth-node";
    assert_eq!(redact_line(line), line);
    assert_eq!(ymd_from_unix(0), "1970-01-01");
}

#[test]
fn enabled_write_persists_redacted_not_raw() {
    let dir = tempfile::tempdir().unwrap();
    let service = AppLogService::new();
    let root = dir.path().join(GUI_LOG_DIR_NAME);
    let line = format!(
        "sync password={SECRET_A} node vmess://{SECRET_B} via https://u:{SECRET_C}@example.com/x"
    );
    assert!(service.append(&root, "2026-10-07", true, &line).unwrap());
    let body = std::fs::read_to_string(root.join("2026-10-07.txt")).unwrap();
    assert!(!body.contains(SECRET_A));
    assert!(!body.contains(SECRET_B));
    assert!(!body.contains(SECRET_C));
    assert!(body.contains(REDACTED));
}

#[test]
fn journal_usable_while_logging_off() {
    let (_dir, engine) = engine_in_temp();
    let (patch, rev) = gui_patch_with_enable_log(&engine, false);
    engine.save_settings_group("GuiItem", patch, rev).unwrap();
    assert!(!engine.app_log_enabled());
    assert!(!engine.write_app_log("journal check line").unwrap());

    // The recovery journal never consults the logger gate: receipts still
    // build and parse while logging is off.
    let settings = engine.load_settings().unwrap().settings;
    let hash = settings_hash_of(&settings);
    let commit_id = commit_id_for("synth-mutation-g04");
    let token = document_token(&commit_id, 7, &hash);
    let committed = committed_receipt("synth-mutation-g04", &commit_id, 1, 7, &token, &hash);
    let json = serde_json::to_string(&committed).unwrap();
    let parsed = parse_receipt(&json).expect("receipt must parse while logging is off");
    assert_eq!(parsed.new_revision, Some(7));

    let rejected = rejected_receipt(
        "synth-mutation-g04",
        1,
        "validation",
        application::recoverable_commit::contract_error(
            codes::FIELD_RANGE,
            "error.local_port_range",
            None,
            false,
        ),
    );
    let json = serde_json::to_string(&rejected).unwrap();
    assert!(parse_receipt(&json).is_some());
}
