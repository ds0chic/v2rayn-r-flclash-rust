//! T16 local backup / restore / upstream-import integration tests.
//!
//! Everything runs in temporary directories; no live installation is touched.

use std::io::Write;
use std::path::{Path, PathBuf};

use application::BackupService;
use persistence::Store;

fn write_synthetic_upstream_db(path: &Path) {
    let store = Store::create(path).expect("create upstream db");
    let conn = store.connection();
    for table in persistence::UPSTREAM_TABLES {
        conn.execute_batch(&table.create_sql())
            .expect("upstream table");
    }
    conn.execute(
        "INSERT INTO ProfileItem \
         (IndexId, ConfigType, ConfigVersion, Subid, Remarks, Address, Security, Id) \
         VALUES ('node-custom', 2, 4, '', 'custom', 'custom.json', '', '')",
        [],
    )
    .expect("insert custom profile");
    drop(store);
}

fn build_upstream_zip(zip_path: &Path, config: &str, db: &Path, resources: &[(&str, &[u8])]) {
    let file = std::fs::File::create(zip_path).expect("zip file");
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    writer
        .start_file("guiConfigs/guiNConfig.json", options)
        .expect("config member");
    writer.write_all(config.as_bytes()).expect("config bytes");
    let db_bytes = std::fs::read(db).expect("read source db");
    writer
        .start_file("guiConfigs/guiNDB.db", options)
        .expect("db member");
    writer.write_all(&db_bytes).expect("db bytes");
    for (relative, bytes) in resources {
        writer
            .start_file(format!("guiConfigs/{relative}"), options)
            .expect("resource member");
        writer.write_all(bytes).expect("resource bytes");
    }
    writer.finish().expect("finish zip");
}

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

/// A runtime whose snapshot reports a live session but whose stop always fails,
/// to prove the restore lifecycle aborts before exchanging any file.
struct StopFailingRuntime;

impl application::RuntimeClient for StopFailingRuntime {
    fn snapshot(&self) -> Result<application::RuntimeSnapshot, domain::DomainError> {
        Ok(application::RuntimeSnapshot {
            state: domain::RuntimeState::Running,
            ..Default::default()
        })
    }

    fn apply(
        &self,
        _plan: &domain::runtime_plan::RuntimePlan,
    ) -> Result<application::ApplyOutcome, domain::DomainError> {
        Ok(application::ApplyOutcome::Accepted {
            operation_id: "op-test".to_string(),
        })
    }

    fn stop(&self) -> Result<(), domain::DomainError> {
        Err(domain::DomainError::new(
            domain::codes::INTERNAL,
            "error.test_stop_failed",
        ))
    }

    fn cancel(
        &self,
        _job_id: &domain::JobId,
    ) -> Result<domain::CancelOutcome, domain::DomainError> {
        Ok(domain::CancelOutcome::NotCancellable)
    }
}

fn null_engine(dir: &Path) -> application::AppEngine {
    application::AppEngine::open_with_runtime(
        dir,
        std::sync::Arc::new(application::NullRuntimeClient::new()),
    )
    .expect("open null-runtime engine")
}

#[test]
fn restore_lifecycle_reloads_settings_and_active() {
    let src = tempfile::tempdir().expect("src");
    seed_data_dir(src.path(), "source");
    std::fs::write(
        src.path().join("guiNConfig.json"),
        r#"{"IndexId":"n1","active_index_id":"restored-active","desired_revision":7,"UIItem":{"CurrentTheme":"Dark"}}"#,
    )
    .expect("src config");
    let service = BackupService::new(src.path());
    let parent = tempfile::tempdir().expect("bundle parent");
    let bundle_root = parent.path().join("bundle");
    service.create_local(&bundle_root, 3).expect("backup");

    let data = tempfile::tempdir().expect("data");
    seed_data_dir(data.path(), "existing");
    let engine = null_engine(data.path());
    assert_eq!(engine.desired_revision(), 0);

    let target_service = BackupService::new(data.path());
    let report = target_service
        .restore_with_lifecycle(&engine, &bundle_root, &data.path().join("work"))
        .expect("lifecycle restore");
    assert!(report.restored);
    // After reopen the engine serves the restored settings/active, not the
    // pre-restore in-memory values.
    assert_eq!(engine.active_profile().as_deref(), Some("restored-active"));
    assert_eq!(engine.desired_revision(), 7);
}

#[test]
fn restore_lifecycle_propagates_stop_failure_before_exchange() {
    let src = tempfile::tempdir().expect("src");
    seed_data_dir(src.path(), "source");
    let service = BackupService::new(src.path());
    let parent = tempfile::tempdir().expect("bundle parent");
    let bundle_root = parent.path().join("bundle");
    service.create_local(&bundle_root, 1).expect("backup");

    let data = tempfile::tempdir().expect("data");
    seed_data_dir(data.path(), "existing");
    let engine = application::AppEngine::open_with_runtime(
        data.path(),
        std::sync::Arc::new(StopFailingRuntime),
    )
    .expect("open engine");

    let result = service.restore_with_lifecycle(&engine, &bundle_root, &data.path().join("work"));
    assert!(result.is_err(), "a stop failure must abort the restore");
    // The live database was never exchanged.
    let store = Store::open(data.path().join("guiNDB.db")).expect("open db");
    let remark: String = store
        .connection()
        .query_row("SELECT Remarks FROM SubItem LIMIT 1", [], |r| r.get(0))
        .expect("remark");
    assert_eq!(remark, "existing");
}

#[test]
fn engine_reopen_propagates_corrupt_config() {
    let data = tempfile::tempdir().expect("data");
    seed_data_dir(data.path(), "existing");
    let engine = null_engine(data.path());
    engine.quiesce().expect("quiesce drops sqlite handles");
    // Damage the config while the engine holds no handle; reopen must surface
    // the failure instead of leaving the engine on empty in-memory backends.
    std::fs::write(data.path().join("guiNConfig.json"), b"{ not json").expect("corrupt config");
    assert!(
        engine.reopen().is_err(),
        "reopen must surface a corrupt config"
    );
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

#[test]
fn upstream_import_installs_resources_and_generates_file_node() {
    let work = tempfile::tempdir().expect("work");
    let src_db = work.path().join("source-guiNDB.db");
    write_synthetic_upstream_db(&src_db);

    let custom: &[u8] = br#"{"outbounds":[{"tag":"custom-out","protocol":"freedom"}]}"#;
    let pac: &[u8] = b"function FindProxyForURL(){}";
    let script: &[u8] = b"// upstream script";
    let zip_path = work.path().join("upstream.zip");
    build_upstream_zip(
        &zip_path,
        r#"{"IndexId":"node-custom","SubIndexId":"","UIItem":{"CurrentTheme":"Dark"},"BrandNewRootItem":{"x":1}}"#,
        &src_db,
        &[
            ("config/custom.json", custom),
            ("pac.txt", pac),
            ("scripts/check.js", script),
        ],
    );

    let data = work.path().join("data");
    std::fs::create_dir_all(&data).expect("data");
    let service = BackupService::new(&data);
    let report = service
        .import_upstream(&zip_path, &work.path().join("import-work"), 7)
        .expect("import");
    assert!(
        matches!(report.status, persistence::ImportStatus::Imported),
        "status: {:?}",
        report.status
    );

    // Resources land at their live-relative paths with matching hashes.
    let live_custom = data.join("config/custom.json");
    assert!(live_custom.is_file(), "config/custom.json not installed");
    assert_eq!(
        persistence::hash::sha256_file(&live_custom).expect("hash live"),
        persistence::hash::sha256_hex(custom)
    );
    assert!(data.join("pac.txt").is_file(), "pac.txt not installed");
    assert!(
        data.join("scripts/check.js").is_file(),
        "nested script not installed"
    );
    // The database/config members are not duplicated as resources.
    assert!(!data.join("guiNConfig.json.tmp-restore").exists());

    // Reopen: the file-type active node generates from the live file content.
    let engine = application::AppEngine::open(&data).expect("open engine");
    let active = engine.active_profile().expect("active profile");
    let expected = persistence::hash::derived_id(
        "profile",
        &format!("{}:node-custom", report.source_fingerprint),
    );
    assert_eq!(active, expected);
    let input = engine
        .build_codegen_input(
            &active,
            domain::CoreType::Xray,
            &application::codegen::CodegenOptions::default(),
        )
        .expect("codegen input");
    let text = input.profile.custom_config.as_deref().unwrap_or("");
    assert!(
        text.contains("custom-out"),
        "custom config not resolved from the live file: {text}"
    );
}

#[test]
fn config_activation_failure_rolls_back_committed_import() {
    let work = tempfile::tempdir().expect("work");
    let data = work.path().join("data");
    std::fs::create_dir_all(&data).expect("data");
    {
        let store = Store::create(data.join("guiNDB.db")).expect("target db");
        store
            .connection()
            .execute(
                "INSERT INTO SubItem (Id, Remarks) VALUES ('old', 'existing')",
                [],
            )
            .expect("seed row");
    }
    // Force `activate_upstream_config` to fail: the live config path is a
    // directory, so `write_json_atomic` cannot replace it.
    std::fs::create_dir(data.join("guiNConfig.json")).expect("blocking dir");

    let src_db = work.path().join("source-guiNDB.db");
    write_synthetic_upstream_db(&src_db);
    let zip_path = work.path().join("upstream.zip");
    build_upstream_zip(
        &zip_path,
        r#"{"IndexId":"node-custom"}"#,
        &src_db,
        &[("config/custom.json", b"{\"a\":1}")],
    );

    let service = BackupService::new(&data);
    let result = service.import_upstream(&zip_path, &work.path().join("import-work"), 7);
    assert!(result.is_err(), "activation should have failed");

    // Database rolled back to the pre-import row, resources not installed.
    let store = Store::open(data.join("guiNDB.db")).expect("open rolled-back db");
    assert_eq!(store.count_rows("SubItem").expect("count"), 1);
    let remark: String = store
        .connection()
        .query_row("SELECT Remarks FROM SubItem LIMIT 1", [], |r| r.get(0))
        .expect("remark");
    assert_eq!(remark, "existing");
    assert!(!data.join("config/custom.json").exists());
}

#[test]
fn resource_install_failure_rolls_back_config_and_database() {
    let work = tempfile::tempdir().expect("work");
    let data = work.path().join("data");
    std::fs::create_dir_all(&data).expect("data");
    {
        let store = Store::create(data.join("guiNDB.db")).expect("target db");
        store
            .connection()
            .execute(
                "INSERT INTO SubItem (Id, Remarks) VALUES ('old', 'existing')",
                [],
            )
            .expect("seed row");
    }
    std::fs::write(data.join("guiNConfig.json"), r#"{"IndexId":"old-config"}"#)
        .expect("old config");
    // Block the resource destination: `config` is a regular file, so the nested
    // `config/custom.json` copy cannot create its parent directory.
    std::fs::write(data.join("config"), b"blocker").expect("blocker");

    let src_db = work.path().join("source-guiNDB.db");
    write_synthetic_upstream_db(&src_db);
    let zip_path = work.path().join("upstream.zip");
    build_upstream_zip(
        &zip_path,
        r#"{"IndexId":"node-custom"}"#,
        &src_db,
        &[("config/custom.json", b"{\"a\":1}")],
    );

    let service = BackupService::new(&data);
    let result = service.import_upstream(&zip_path, &work.path().join("import-work"), 7);
    assert!(result.is_err(), "resource install should have failed");

    let store = Store::open(data.join("guiNDB.db")).expect("open rolled-back db");
    assert_eq!(store.count_rows("SubItem").expect("count"), 1);
    let remark: String = store
        .connection()
        .query_row("SELECT Remarks FROM SubItem LIMIT 1", [], |r| r.get(0))
        .expect("remark");
    assert_eq!(remark, "existing");
    let config = std::fs::read_to_string(data.join("guiNConfig.json")).expect("config");
    assert_eq!(config, r#"{"IndexId":"old-config"}"#);
}

#[test]
fn restore_prior_config_snapshot_failure_rolls_back_database() {
    let src = tempfile::tempdir().expect("src");
    seed_data_dir(src.path(), "source");
    std::fs::write(
        src.path().join("guiNConfig.json"),
        r#"{"IndexId":"source"}"#,
    )
    .expect("src config");
    let service = BackupService::new(src.path());
    let parent = tempfile::tempdir().expect("bundle parent");
    let bundle_root = parent.path().join("bundle");
    service.create_local(&bundle_root, 1).expect("backup");

    let target = tempfile::tempdir().expect("target");
    seed_data_dir(target.path(), "existing");
    std::fs::write(
        target.path().join("guiNConfig.json"),
        r#"{"IndexId":"existing"}"#,
    )
    .expect("existing config");
    // Make the prior-config snapshot fail: the sibling path is a directory.
    std::fs::create_dir(target.path().join("guiNConfig.json.restore-prev")).expect("blocking dir");

    let target_service = BackupService::new(target.path());
    let result = target_service.restore(&bundle_root, &target.path().join("work"));
    assert!(result.is_err(), "prior snapshot should have failed");

    let store = Store::open(target.path().join("guiNDB.db")).expect("open rolled-back db");
    assert_eq!(store.count_rows("SubItem").expect("count"), 1);
    let remark: String = store
        .connection()
        .query_row("SELECT Remarks FROM SubItem LIMIT 1", [], |r| r.get(0))
        .expect("remark");
    assert_eq!(remark, "existing");
    let config = std::fs::read_to_string(target.path().join("guiNConfig.json")).expect("config");
    assert_eq!(config, r#"{"IndexId":"existing"}"#);
}
