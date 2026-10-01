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
