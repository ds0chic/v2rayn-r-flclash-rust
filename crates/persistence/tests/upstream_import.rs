//! End-to-end import tests against the synthetic upstream fixtures.

mod common;

use persistence::{
    import_from_path, ImportFault, ImportOptions, ImportStatus, ProtocolExtraBlob,
    TransportExtraBlob, UPSTREAM_VERSION,
};
use rusqlite::Connection;

fn options() -> ImportOptions {
    ImportOptions {
        now: 1_900_000_000,
        fault: ImportFault::None,
    }
}

#[test]
fn v2_fixture_migrates_groups_and_normal_nodes() {
    let (_keep, work, target) = common::temp_workspace();
    let report = import_from_path(&common::upstream_v2(), &target, &work, &options()).unwrap();

    assert_eq!(report.status, ImportStatus::Imported);
    assert!(report.committed);
    let migration_ids: Vec<&str> = report
        .migrations
        .iter()
        .map(|m| m.migration_id.as_str())
        .collect();
    assert!(migration_ids.contains(&"MIG-ENT-002"));
    assert!(migration_ids.contains(&"MIG-ENT-003"));
    assert!(migration_ids.contains(&"MIG-ENT-004"));

    // Group rows are a migration source only; ProfileItem keeps every source row.
    assert_eq!(report.source_version, 2);

    let store = persistence::Store::open_readonly(&target).unwrap();
    let profiles = store.read_rows("ProfileItem").unwrap();
    assert_eq!(profiles.len(), 7);
    assert_eq!(store.read_rows("ProfileGroupItem").unwrap().len(), 0);

    // Duplicate Remarks are kept as separate rows (never merged by name).
    let dup = profiles
        .iter()
        .filter(|r| r.string("Remarks") == "香港节点 🚀")
        .count();
    assert_eq!(dup, 2, "same-remarks nodes must not be merged");

    // Chinese/emoji names survive.
    assert!(profiles
        .iter()
        .any(|r| r.string("Remarks") == "台灣節點 🇹🇼"));

    // SS legacy Security moved into ProtoExtra; unknown blob key preserved.
    let ss = profiles
        .iter()
        .find(|r| {
            r.string("Remarks") == "香港节点 🚀" && r.string("IndexId").starts_with("profile-")
        })
        .expect("migrated SS node");
    let blob = ProtocolExtraBlob::parse(ss.opt_string("ProtoExtra").as_deref()).unwrap();
    assert_eq!(blob.ss_method.as_deref(), Some("aes-256-gcm"));
    assert!(blob.extra.contains_key("BrandNewFlag"));

    // Group details copied from the deprecated table and ids remapped.
    let group = profiles
        .iter()
        .find(|r| r.string("Remarks") == "策略组")
        .expect("group row");
    let group_blob = ProtocolExtraBlob::parse(group.opt_string("ProtoExtra").as_deref()).unwrap();
    assert_eq!(group_blob.group_type.as_deref(), Some("PolicyGroup"));
    let children = group_blob.child_items.unwrap();
    assert!(children.contains("profile-"));
    assert!(children.contains("missing-id"));

    // Routing outbound reference and the dangling one are surfaced as warnings.
    assert!(report.warnings.iter().any(|w| w.code == "W_REF_OUTBOUND"));
    assert!(report.warnings.iter().any(|w| w.code == "W_REF_CHILD"));

    // guiNConfig.json preserved verbatim, including unknown keys.
    let config_json = store.get_meta("upstream_config").unwrap().unwrap();
    let config: serde_json::Value = serde_json::from_str(&config_json).unwrap();
    assert_eq!(config["UIItem"]["CurrentTheme"], "Dark");
    assert_eq!(config["BrandNewRootItem"]["weight"], 7);
    assert_eq!(config["SystemProxyItem"]["SystemProxyExceptions"], "");

    // Raw source rows are retained for auditing.
    assert!(store.count_raw_records(&report.batch_id).unwrap() >= 7);
}

#[test]
fn v3_fixture_migrates_transport() {
    let (_keep, work, target) = common::temp_workspace();
    let report = import_from_path(&common::upstream_v3(), &target, &work, &options()).unwrap();
    assert_eq!(report.status, ImportStatus::Imported);
    assert!(report
        .migrations
        .iter()
        .any(|m| m.migration_id == "MIG-ENT-004"));

    let store = persistence::Store::open_readonly(&target).unwrap();
    let profiles = store.read_rows("ProfileItem").unwrap();
    let by_remarks = |needle: &str| {
        profiles
            .iter()
            .find(|r| r.string("Remarks") == needle)
            .unwrap_or_else(|| panic!("missing {needle}"))
    };

    let ws = by_remarks("WS 节点");
    let ws_t = TransportExtraBlob::parse(ws.opt_string("TransportExtra").as_deref()).unwrap();
    assert_eq!(ws_t.host.as_deref(), Some("ws.example.invalid"));
    assert_eq!(ws_t.path.as_deref(), Some("/ws"));

    let grpc = by_remarks("gRPC 节点");
    let grpc_t = TransportExtraBlob::parse(grpc.opt_string("TransportExtra").as_deref()).unwrap();
    assert_eq!(
        grpc_t.grpc_authority.as_deref(),
        Some("grpc.example.invalid")
    );
    assert_eq!(grpc_t.grpc_service_name.as_deref(), Some("svc"));
    assert_eq!(grpc_t.grpc_mode.as_deref(), Some("multi"));

    let kcp = by_remarks("KCP 节点");
    let kcp_t = TransportExtraBlob::parse(kcp.opt_string("TransportExtra").as_deref()).unwrap();
    assert_eq!(kcp_t.kcp_header_type.as_deref(), Some("srtp"));
    assert_eq!(kcp_t.kcp_seed.as_deref(), Some("seed-token"));

    let tcp = by_remarks("TCP 别名节点");
    assert_eq!(tcp.string("Network"), "raw");
    let tcp_t = TransportExtraBlob::parse(tcp.opt_string("TransportExtra").as_deref()).unwrap();
    assert_eq!(tcp_t.host.as_deref(), Some("tcp.example.invalid"));

    assert!(profiles.iter().all(|r| r.i64("ConfigVersion") == 4));
}

#[test]
fn same_fixture_import_is_idempotent() {
    let (_keep, work, target) = common::temp_workspace();
    let first = import_from_path(&common::upstream_v2(), &target, &work, &options()).unwrap();
    assert_eq!(first.status, ImportStatus::Imported);
    let count_after_first = persistence::Store::open_readonly(&target)
        .unwrap()
        .count_rows("ProfileItem")
        .unwrap();

    let second = import_from_path(&common::upstream_v2(), &target, &work, &options()).unwrap();
    assert_eq!(second.status, ImportStatus::AlreadyImported);
    assert!(!second.committed);

    let count_after_second = persistence::Store::open_readonly(&target)
        .unwrap()
        .count_rows("ProfileItem")
        .unwrap();
    assert_eq!(count_after_first, count_after_second);
}

#[test]
fn same_content_from_another_directory_is_idempotent() {
    // The source fingerprint must be content-addressed, not path-addressed:
    // moving/reinstalling the same guiNDB.db into a new directory must not
    // duplicate every node on re-import.
    fn copy_source(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for name in ["guiNConfig.json", "guiNDB.db"] {
            std::fs::copy(from.join(name), to.join(name)).unwrap();
        }
    }

    let (_keep, work, target) = common::temp_workspace();
    let first = import_from_path(&common::upstream_v2(), &target, &work, &options()).unwrap();
    assert_eq!(first.status, ImportStatus::Imported);

    let moved_root = tempfile::tempdir().unwrap();
    let moved = moved_root.path().join("relocated");
    copy_source(&common::upstream_v2(), &moved);

    let second = import_from_path(&moved, &target, &work, &options()).unwrap();
    assert_eq!(
        second.status,
        ImportStatus::AlreadyImported,
        "same bytes from another directory must be a no-op"
    );
    assert!(!second.committed);

    let store = persistence::Store::open_readonly(&target).unwrap();
    assert_eq!(store.count_rows("ProfileItem").unwrap(), 7);
}

#[test]
fn corrupt_database_errors_without_panicking() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("guiNConfig.json"), r#"{"IndexId":"x"}"#).unwrap();
    std::fs::write(dir.path().join("guiNDB.db"), b"definitely not a database").unwrap();
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let target = dir.path().join("app.db");

    let result = import_from_path(dir.path(), &target, &work, &options());
    assert!(result.is_err(), "corrupt source must surface an error");
    assert!(!target.exists(), "no target may be created on failure");
}

#[test]
fn missing_source_is_rejected_politely() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let target = dir.path().join("app.db");
    let report = import_from_path(dir.path(), &target, &work, &options()).unwrap();
    assert_eq!(report.status, ImportStatus::Rejected);
    assert!(!target.exists());
    assert!(!report.errors.is_empty());
}

#[test]
fn source_commit_matches_upstream_baseline() {
    assert_eq!(UPSTREAM_VERSION, "7.25.4");
    assert_eq!(
        persistence::SOURCE_COMMIT,
        "7d6a967c18c697f28dc6917122ed3a4993fcf336"
    );
    // Ensure the fixture databases are openable and match the baseline shape.
    let conn = Connection::open(common::upstream_v2().join("guiNDB.db")).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM ProfileItem", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 7);
}

#[test]
fn archive_source_imports_via_gui_configs_layout() {
    use std::io::Write;

    // Build a directory source, then zip it under the upstream guiConfigs/ layout.
    let src = tempfile::tempdir().unwrap();
    std::fs::write(
        src.path().join("guiNConfig.json"),
        r#"{"IndexId":"zip-node","UIItem":{"CurrentTheme":"Dark"}}"#,
    )
    .unwrap();
    {
        let conn = Connection::open(src.path().join("guiNDB.db")).unwrap();
        for table in persistence::UPSTREAM_TABLES {
            conn.execute_batch(&table.create_sql()).unwrap();
        }
        conn.execute(
            "INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Remarks) \
             VALUES ('zip-1', 5, 4, 'Zip 节点')",
            [],
        )
        .unwrap();
    }

    let zip_path = src.path().join("backup.zip");
    {
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer
            .start_file("guiConfigs/guiNConfig.json", options)
            .unwrap();
        writer
            .write_all(&std::fs::read(src.path().join("guiNConfig.json")).unwrap())
            .unwrap();
        writer.start_file("guiConfigs/guiNDB.db", options).unwrap();
        writer
            .write_all(&std::fs::read(src.path().join("guiNDB.db")).unwrap())
            .unwrap();
        writer.finish().unwrap();
    }

    let work = src.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let target = src.path().join("app.db");
    let report = import_from_path(&zip_path, &target, &work, &options()).unwrap();
    assert_eq!(report.status, ImportStatus::Imported);
    assert_eq!(report.source_kind, "archive");
    let store = persistence::Store::open_readonly(&target).unwrap();
    assert_eq!(store.count_rows("ProfileItem").unwrap(), 1);
    let config = store.get_meta("upstream_config").unwrap().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&config).unwrap();
    assert_eq!(parsed["UIItem"]["CurrentTheme"], "Dark");
}
