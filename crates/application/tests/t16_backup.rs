//! T16 local backup / restore / upstream-import integration tests.
//!
//! Everything runs in temporary directories; no live installation is touched.

use std::io::Write;
use std::path::{Path, PathBuf};

use application::BackupService;
use persistence::Store;

fn seed_data_dir(dir: &Path, remarks: &str) {
    let db = dir.join("guiNDB.db");
    let store = Store::create(&db).expect("create db");
    store
        .connection()
        .execute(
            "INSERT INTO SubItem (Id, Remarks) VALUES ('s1', ?1)",
            [remarks],
        )
        .expect("insert row");
    std::fs::write(dir.join("guiNConfig.json"), r#"{"IndexId":"n1"}"#).expect("config");
    std::fs::write(dir.join("custom-notes.txt"), b"keep me").expect("resource");
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/synthetic/upstream-v2")
}

#[test]
fn local_backup_roundtrip_and_restore_is_clean() {
    let src = tempfile::tempdir().expect("src");
    seed_data_dir(src.path(), "source");
    let service = BackupService::new(src.path());
    let parent = tempfile::tempdir().expect("bundle parent");
    let bundle_root = parent.path().join("bundle");

    let backup = service.create_local(&bundle_root, 42).expect("backup");
    assert_eq!(backup.manifest.format_version, 1);
    assert!(!backup.manifest.db_sha256.is_empty());
    assert_eq!(backup.manifest.referenced_resources.len(), 1);
    assert!(service.verify(&bundle_root).expect("verify").ok);

    let target = tempfile::tempdir().expect("target");
    let target_service = BackupService::new(target.path());
    let report = target_service
        .restore(&bundle_root, &target.path().join("work"))
        .expect("restore");
    assert!(report.restored);
    let store = Store::open(target.path().join("guiNDB.db")).expect("open restored");
    assert_eq!(store.count_rows("SubItem").expect("count"), 1);
    assert!(target.path().join("guiNConfig.json").is_file());
}

#[test]
fn tampered_bundle_is_rejected_without_damaging_existing() {
    let src = tempfile::tempdir().expect("src");
    seed_data_dir(src.path(), "source");
    let service = BackupService::new(src.path());
    let parent = tempfile::tempdir().expect("bundle parent");
    let bundle_root = parent.path().join("bundle");
    service.create_local(&bundle_root, 1).expect("backup");
    std::fs::write(bundle_root.join("guiNDB.db"), b"corrupted").expect("tamper");

    assert!(!service.verify(&bundle_root).expect("verify").ok);

    let target = tempfile::tempdir().expect("target");
    seed_data_dir(target.path(), "existing");
    let target_service = BackupService::new(target.path());
    let result = target_service.restore(&bundle_root, &target.path().join("work"));
    assert!(result.is_err());
    let store = Store::open(target.path().join("guiNDB.db")).expect("open existing");
    assert_eq!(store.count_rows("SubItem").expect("count"), 1);
}

#[test]
fn recognizes_upstream_gui_configs_zip() {
    let dir = tempfile::tempdir().expect("dir");
    let zip_path = dir.path().join("upstream.zip");
    {
        let file = std::fs::File::create(&zip_path).expect("zip file");
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer
            .start_file("guiConfigs/guiNConfig.json", options)
            .expect("start");
        writer.write_all(b"{}").expect("write");
        writer.finish().expect("finish");
    }
    let service = BackupService::new(dir.path());
    let recognition = service.recognize(&zip_path).expect("recognize");
    assert!(recognition.is_upstream);
    assert!(recognition.has_config);
    assert_eq!(recognition.layout, "guiConfigs/");
}

#[test]
fn imports_upstream_fixture_via_candidate_flow() {
    // A sanitized upstream directory copied to guiConfigs-like members.
    let work = tempfile::tempdir().expect("work");
    let source = work.path().join("guiConfigs");
    std::fs::create_dir_all(&source).expect("source dir");
    let fixture = fixture_dir();
    std::fs::copy(fixture.join("guiNDB.db"), source.join("guiNDB.db")).expect("db");
    std::fs::copy(
        fixture.join("guiNConfig.json"),
        source.join("guiNConfig.json"),
    )
    .expect("config");

    let data = work.path().join("data");
    std::fs::create_dir_all(&data).expect("data");
    let service = BackupService::new(&data);
    let report = service
        .import_upstream(&source, &work.path().join("import-work"), 7)
        .expect("import");
    assert!(
        matches!(report.status, persistence::ImportStatus::Imported),
        "status: {:?}",
        report.status
    );
    assert!(data.join("guiNDB.db").is_file());
}

#[test]
fn nested_resources_roundtrip_and_restore_complete() {
    let src = tempfile::tempdir().expect("src");
    seed_data_dir(src.path(), "source");
    // Nested configuration subdirectories must be collected and copied back.
    std::fs::create_dir_all(src.path().join("custom/sub")).expect("nested dir");
    std::fs::write(src.path().join("custom/sub/node.json"), br#"{"k":1}"#).expect("nested");
    std::fs::create_dir_all(src.path().join("pac")).expect("pac dir");
    std::fs::write(
        src.path().join("pac/local.pac"),
        b"function FindProxyForURL(){}",
    )
    .expect("pac");

    let service = BackupService::new(src.path());
    let parent = tempfile::tempdir().expect("bundle parent");
    let bundle_root = parent.path().join("bundle");
    let backup = service.create_local(&bundle_root, 42).expect("backup");
    let names: Vec<&str> = backup
        .manifest
        .referenced_resources
        .iter()
        .map(|r| r.relative_path.as_str())
        .collect();
    assert!(names.contains(&"custom/sub/node.json"), "names: {names:?}");
    assert!(names.contains(&"pac/local.pac"), "names: {names:?}");
    assert!(names.contains(&"custom-notes.txt"), "names: {names:?}");
    assert!(service.verify(&bundle_root).expect("verify").ok);

    let target = tempfile::tempdir().expect("target");
    let target_service = BackupService::new(target.path());
    let report = target_service
        .restore(&bundle_root, &target.path().join("work"))
        .expect("restore");
    assert!(report.restored);
    assert_eq!(
        std::fs::read_to_string(target.path().join("custom/sub/node.json")).expect("nested back"),
        r#"{"k":1}"#
    );
    assert!(target.path().join("pac/local.pac").is_file());
    assert_eq!(
        std::fs::read_to_string(target.path().join("custom-notes.txt")).expect("top back"),
        "keep me"
    );
}

#[test]
fn tampered_resource_rejects_restore_without_damage() {
    let src = tempfile::tempdir().expect("src");
    seed_data_dir(src.path(), "source");
    std::fs::write(src.path().join("brand-new.dat"), b"original").expect("resource");
    let service = BackupService::new(src.path());
    let parent = tempfile::tempdir().expect("bundle parent");
    let bundle_root = parent.path().join("bundle");
    service.create_local(&bundle_root, 1).expect("backup");
    // Corrupt a referenced resource: verification must catch it.
    std::fs::write(bundle_root.join("brand-new.dat"), b"tampered").expect("tamper");

    let target = tempfile::tempdir().expect("target");
    seed_data_dir(target.path(), "existing");
    let target_service = BackupService::new(target.path());
    let result = target_service.restore(&bundle_root, &target.path().join("work"));
    assert!(result.is_err());
    let store = Store::open(target.path().join("guiNDB.db")).expect("open existing");
    assert_eq!(store.count_rows("SubItem").expect("count"), 1);
    assert_eq!(
        std::fs::read_to_string(target.path().join("custom-notes.txt")).expect("kept"),
        "keep me"
    );
}

#[test]
fn resource_copy_failure_rolls_back_database_and_config() {
    let src = tempfile::tempdir().expect("src");
    seed_data_dir(src.path(), "source");
    std::fs::create_dir_all(src.path().join("custom/sub")).expect("nested dir");
    std::fs::write(src.path().join("custom/sub/node.json"), b"{}").expect("nested");
    let service = BackupService::new(src.path());
    let parent = tempfile::tempdir().expect("bundle parent");
    let bundle_root = parent.path().join("bundle");
    service.create_local(&bundle_root, 1).expect("backup");

    let target = tempfile::tempdir().expect("target");
    seed_data_dir(target.path(), "existing");
    // Block the nested resource destination with a regular file so its copy
    // fails after the database/config have already been swapped.
    std::fs::write(target.path().join("custom"), b"blocker").expect("blocker");

    let target_service = BackupService::new(target.path());
    let result = target_service.restore(&bundle_root, &target.path().join("work"));
    assert!(result.is_err());
    // Database rolled back to the pre-restore row.
    let store = Store::open(target.path().join("guiNDB.db")).expect("open existing");
    assert_eq!(store.count_rows("SubItem").expect("count"), 1);
    let remark: String = store
        .connection()
        .query_row("SELECT Remarks FROM SubItem LIMIT 1", [], |r| r.get(0))
        .expect("remark");
    assert_eq!(remark, "existing");
    // Config rolled back to the pre-restore document.
    let config = std::fs::read_to_string(target.path().join("guiNConfig.json")).expect("config");
    assert_eq!(config, r#"{"IndexId":"n1"}"#);
}

#[test]
fn live_engine_quiesce_restore_reopen_roundtrip() {
    // Source bundle with a distinguishing config marker.
    let src = tempfile::tempdir().expect("src");
    seed_data_dir(src.path(), "source");
    std::fs::write(
        src.path().join("guiNConfig.json"),
        r#"{"IndexId":"n1","BrandNewRootItem":{"x":1}}"#,
    )
    .expect("src config");
    let service = BackupService::new(src.path());
    let parent = tempfile::tempdir().expect("bundle parent");
    let bundle_root = parent.path().join("bundle");
    service.create_local(&bundle_root, 1).expect("backup");

    // Target data dir with the database already open through an engine.
    let data = tempfile::tempdir().expect("data");
    seed_data_dir(data.path(), "existing");
    let engine = application::AppEngine::open(data.path()).expect("open engine");
    assert_eq!(engine.profile_count(), 0);

    let work = data.path().join("work");
    let target_service = BackupService::new(data.path());
    engine.quiesce().expect("quiesce drops sqlite handles");
    let report = target_service
        .restore(&bundle_root, &work)
        .expect("restore");
    assert!(report.restored);
    engine.reopen().expect("reopen reloads restored state");

    // The restored database is intact (source row, not the pre-restore row).
    let store = Store::open(data.path().join("guiNDB.db")).expect("open restored");
    let remark: String = store
        .connection()
        .query_row("SELECT Remarks FROM SubItem LIMIT 1", [], |r| r.get(0))
        .expect("remark");
    assert_eq!(remark, "source");
    // The restored config (with its unknown key) is the live config.
    let config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(data.path().join("guiNConfig.json")).expect("config"),
    )
    .expect("json");
    assert!(config.get("BrandNewRootItem").is_some());
}

#[test]
fn upstream_import_activates_config_and_active_id() {
    use persistence::hash::derived_id;

    let work = tempfile::tempdir().expect("work");
    let source = work.path().join("guiConfigs");
    std::fs::create_dir_all(&source).expect("source dir");
    let fixture = fixture_dir();
    std::fs::copy(fixture.join("guiNDB.db"), source.join("guiNDB.db")).expect("db");
    std::fs::copy(
        fixture.join("guiNConfig.json"),
        source.join("guiNConfig.json"),
    )
    .expect("config");

    let data = work.path().join("data");
    std::fs::create_dir_all(&data).expect("data");
    let service = BackupService::new(&data);
    let report = service
        .import_upstream(&source, &work.path().join("import-work"), 7)
        .expect("import");
    assert!(matches!(report.status, persistence::ImportStatus::Imported));

    let expected_active = derived_id("profile", &format!("{}:node-hk", report.source_fingerprint));
    let config_text = std::fs::read_to_string(data.join("guiNConfig.json")).expect("active config");
    let config: serde_json::Value = serde_json::from_str(&config_text).expect("json");
    assert_eq!(
        config.get("active_index_id").and_then(|v| v.as_str()),
        Some(expected_active.as_str())
    );
    assert_eq!(
        config.get("IndexId").and_then(|v| v.as_str()),
        Some(expected_active.as_str())
    );
    // Unknown root keys and the upstream settings tree survive verbatim.
    assert!(config.get("BrandNewRootItem").is_some());
    assert_eq!(
        config
            .get("UIItem")
            .and_then(|ui| ui.get("CurrentTheme"))
            .and_then(|v| v.as_str()),
        Some("Dark")
    );

    // A fresh engine open sees the imported configuration as active.
    let engine = application::AppEngine::open(&data).expect("open engine");
    assert_eq!(
        engine.active_profile().as_deref(),
        Some(expected_active.as_str())
    );
}
