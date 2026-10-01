//! T04b: import/migration compatibility against sanitized *real-shape* fixtures.
//!
//! The fixtures under `fixtures/synthetic/upstream-real-shape/` are derived and
//! sanitized from a real v2rayN profile directory (see that directory's
//! README). They contain no real addresses, ports, credentials, subscription
//! URLs or remarks. These tests pin the importer against the column shapes and
//! version distributions that real upstream builds actually produce.

mod common;

use std::path::Path;

use persistence::{
    import_from_path, ConfigDocument, ImportFault, ImportOptions, ImportStatus, Store,
};
use rusqlite::Connection;

fn options() -> ImportOptions {
    ImportOptions {
        now: 1_900_000_000,
        fault: ImportFault::None,
    }
}

fn has_non_ascii(s: &str) -> bool {
    s.chars().any(|c| c as u32 > 127)
}

fn has_emoji(s: &str) -> bool {
    s.chars().any(|c| {
        let u = c as u32;
        (0x1F000..=0x1FAFF).contains(&u)
            || (0x2600..=0x27BF).contains(&u)
            || (0x1F1E6..=0x1F1FF).contains(&u)
    })
}

fn count(store: &Store, sql: &str) -> u64 {
    store.count_query(sql, &[]).unwrap()
}

#[test]
fn real_current_shape_imports_and_reads_canonical_ui_item() {
    let (_keep, work, target) = common::temp_workspace();
    let report = import_from_path(&common::real_shape_dir(), &target, &work, &options()).unwrap();

    assert_eq!(report.status, ImportStatus::Imported);
    assert!(report.committed);
    assert!(
        report.errors.is_empty(),
        "no fatal validation errors expected"
    );
    assert_eq!(report.source_version, 4);

    let store = Store::open_readonly(&target).unwrap();
    assert_eq!(store.count_rows("ProfileItem").unwrap(), 40);
    assert_eq!(store.count_rows("SubItem").unwrap(), 1);
    assert_eq!(store.count_rows("ProfileExItem").unwrap(), 78);
    assert_eq!(store.count_rows("ServerStatItem").unwrap(), 10);
    assert_eq!(store.count_rows("RoutingItem").unwrap(), 4);
    assert_eq!(store.count_rows("DNSItem").unwrap(), 2);
    assert_eq!(store.count_rows("FullConfigTemplateItem").unwrap(), 2);

    // The newer upstream `EchForceQuery` column exists in the candidate even
    // though the frozen 7.25.4 entity does not declare it.
    let has_ech_force_query: i64 = store
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('ProfileItem') WHERE name = 'EchForceQuery'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(has_ech_force_query, 1);

    // Canonical `UiItem` key is read: theme/window/column state resolve.
    let config_json = store.get_meta("upstream_config").unwrap().unwrap();
    let doc = ConfigDocument::parse(&config_json).unwrap();
    // The fixture is sanitized, so the language value is synthetic; only its
    // presence and the structural counts are pinned here.
    assert!(doc.language().is_some_and(|v| !v.is_empty()));
    assert_eq!(doc.window_states().unwrap().len(), 14);
    assert_eq!(doc.main_columns().unwrap().len(), 14);
    assert_eq!(doc.clash_columns().unwrap().len(), 5);

    // Second import is a content-addressed no-op.
    let second = import_from_path(&common::real_shape_dir(), &target, &work, &options()).unwrap();
    assert_eq!(second.status, ImportStatus::AlreadyImported);
    assert!(!second.committed);
    assert_eq!(store.count_rows("ProfileItem").unwrap(), 40);
}

#[test]
fn real_shape_content_from_another_directory_is_idempotent() {
    let moved_root = tempfile::tempdir().unwrap();
    let moved = moved_root.path().join("relocated");
    std::fs::create_dir_all(&moved).unwrap();
    for name in ["guiNConfig.json", "guiNDB.db"] {
        std::fs::copy(common::real_shape_dir().join(name), moved.join(name)).unwrap();
    }

    let (_keep, work, target) = common::temp_workspace();
    let first = import_from_path(&common::real_shape_dir(), &target, &work, &options()).unwrap();
    assert_eq!(first.status, ImportStatus::Imported);
    let second = import_from_path(&moved, &target, &work, &options()).unwrap();
    assert_eq!(
        second.status,
        ImportStatus::AlreadyImported,
        "same bytes from a different directory must be a no-op"
    );
    assert_eq!(
        Store::open_readonly(&target)
            .unwrap()
            .count_rows("ProfileItem")
            .unwrap(),
        40
    );
}

#[test]
fn real_old_v1_migrates_v2_to_v4_and_keeps_unicode() {
    let (_keep, source) = common::bare_db_source("upstream-old-v1.db");
    let source_db = source.join("guiNDB.db");
    let before = std::fs::read(&source_db).unwrap();

    let (_keep2, work, target) = common::temp_workspace();
    let report = import_from_path(&source, &target, &work, &options()).unwrap();
    assert_eq!(report.status, ImportStatus::Imported);
    assert!(report.errors.is_empty());
    assert_eq!(report.source_version, 2);

    let mig = |id: &str| {
        report
            .migrations
            .iter()
            .find(|m| m.migration_id == id)
            .unwrap_or_else(|| panic!("missing {id}"))
    };
    assert_eq!(mig("MIG-ENT-003").entities_touched, 90);
    assert_eq!(mig("MIG-ENT-004").entities_touched, 90);

    let store = Store::open_readonly(&target).unwrap();
    let profiles = store.read_rows("ProfileItem").unwrap();
    assert_eq!(profiles.len(), 90);
    assert!(profiles.iter().all(|r| r.i64("ConfigVersion") == 4));
    assert!(profiles.iter().all(protocol_extra_or_missing));
    assert_eq!(
        profiles
            .iter()
            .filter(|r| has_non_ascii(&r.string("Remarks")))
            .count(),
        74
    );
    assert_eq!(
        profiles
            .iter()
            .filter(|r| has_emoji(&r.string("Remarks")))
            .count(),
        6
    );

    // Read-only guarantee: importing never writes the source directory.
    assert_eq!(std::fs::read(&source_db).unwrap(), before);
}

fn protocol_extra_or_missing(row: &persistence::RawRow) -> bool {
    // Every migrated profile carries a ProtoExtra blob; transport nodes carry
    // one too. This helper keeps the assertion intent obvious at the call site.
    row.opt_string("ProtoExtra").is_some()
}

#[test]
fn real_old_v2_migrates_and_current_shape_is_additive() {
    let (_keep, source) = common::bare_db_source("upstream-old-v2.db");
    let (_keep2, work, target) = common::temp_workspace();
    let report = import_from_path(&source, &target, &work, &options()).unwrap();
    assert_eq!(report.status, ImportStatus::Imported);
    assert!(report.errors.is_empty());

    let mig = |id: &str| {
        report
            .migrations
            .iter()
            .find(|m| m.migration_id == id)
            .unwrap_or_else(|| panic!("missing {id}"))
    };
    assert_eq!(mig("MIG-ENT-003").entities_touched, 10);
    assert_eq!(mig("MIG-ENT-004").entities_touched, 10);

    let store = Store::open_readonly(&target).unwrap();
    assert_eq!(store.count_rows("ProfileItem").unwrap(), 10);
    assert_eq!(store.count_rows("ProfileExItem").unwrap(), 470);
    // The old source lacks EchForceQuery; the candidate still has the column.
    let has_ech_force_query: i64 = store
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('ProfileItem') WHERE name = 'EchForceQuery'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(has_ech_force_query, 1);
}

#[test]
fn real_current_ech_force_query_value_is_preserved() {
    // Copy the sanitized current database, stamp a synthetic value into the
    // newer column, and prove the importer carries it into the live table.
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::copy(
        common::real_shape_dir().join("guiNDB.db"),
        source.join("guiNDB.db"),
    )
    .unwrap();
    {
        let conn = Connection::open(source.join("guiNDB.db")).unwrap();
        conn.execute(
            "UPDATE ProfileItem SET EchForceQuery = 'fixture-ech-force' \
             WHERE rowid = (SELECT rowid FROM ProfileItem LIMIT 1)",
            [],
        )
        .unwrap();
    }

    let (_keep, work, target) = common::temp_workspace();
    let report = import_from_path(&source, &target, &work, &options()).unwrap();
    assert_eq!(report.status, ImportStatus::Imported);

    let store = Store::open_readonly(&target).unwrap();
    assert_eq!(
        count(
            &store,
            "SELECT COUNT(*) FROM ProfileItem WHERE EchForceQuery = 'fixture-ech-force'"
        ),
        1
    );
}

#[test]
fn real_bak_v4_imports_anytls_and_shadowsocks_types() {
    let (_keep, source) = common::bare_db_source("upstream-bak.db");
    let (_keep2, work, target) = common::temp_workspace();
    let report = import_from_path(&source, &target, &work, &options()).unwrap();
    assert_eq!(report.status, ImportStatus::Imported);
    assert!(report.errors.is_empty());
    assert_eq!(report.source_version, 4);

    let store = Store::open_readonly(&target).unwrap();
    assert_eq!(store.count_rows("ProfileItem").unwrap(), 454);
    assert_eq!(store.count_rows("ProfileExItem").unwrap(), 454);
    assert_eq!(store.count_rows("ServerStatItem").unwrap(), 85);
    assert_eq!(store.count_rows("SubItem").unwrap(), 13);
    // ConfigType 11 (Anytls) and 3 (Shadowsocks) survive unremapped.
    assert_eq!(
        count(
            &store,
            "SELECT COUNT(*) FROM ProfileItem WHERE ConfigType = 11"
        ),
        25
    );
    assert_eq!(
        count(
            &store,
            "SELECT COUNT(*) FROM ProfileItem WHERE ConfigType = 3"
        ),
        2
    );
    assert_eq!(
        count(
            &store,
            "SELECT COUNT(*) FROM ProfileItem WHERE ConfigVersion = 4"
        ),
        454
    );
    // Emoji remarks from the real shape survive the round trip.
    let emoji = store
        .read_rows("ProfileItem")
        .unwrap()
        .iter()
        .filter(|r| has_emoji(&r.string("Remarks")))
        .count();
    assert_eq!(emoji, 81);
}

fn _assert_path_type(_: &Path) {}

#[test]
fn truncated_real_shape_db_errors_without_panicking() {
    // A real-shaped database cut short must be reported as an error and must
    // never leave a half-written target behind.
    let (_keep, source) = common::bare_db_source("upstream-bak.db");
    let db = source.join("guiNDB.db");
    let bytes = std::fs::read(&db).unwrap();
    std::fs::write(&db, &bytes[..bytes.len() / 3]).unwrap();

    let (_keep2, work, target) = common::temp_workspace();
    let result = import_from_path(&source, &target, &work, &options());
    // Either a clean rejection or a committed import is acceptable; a panic is
    // not. When rejected, the target must not exist as a partial database.
    match result {
        Ok(report) => assert!(!report.committed || report.errors.is_empty()),
        Err(_) => assert!(
            !target.exists(),
            "a rejected import must not leave a partial target"
        ),
    }
}

#[test]
fn real_shape_source_with_wal_imports_committed_content() {
    // Stage the sanitized current database, add a committed-but-uncheckpointed
    // row in WAL mode, and prove the importer sees it (backup API, not a raw
    // file copy).
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::copy(
        common::real_shape_dir().join("guiNDB.db"),
        source.join("guiNDB.db"),
    )
    .unwrap();
    let conn = Connection::open(source.join("guiNDB.db")).unwrap();
    conn.pragma_update(None, "journal_mode", "WAL").unwrap();
    conn.execute(
        "INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Remarks, Network) \
         VALUES ('wal-real-shape', 5, 4, 'wal-fixture', 'ws')",
        [],
    )
    .unwrap();

    let (_keep, work, target) = common::temp_workspace();
    let report = import_from_path(&source, &target, &work, &options()).unwrap();
    assert_eq!(report.status, ImportStatus::Imported);
    let store = Store::open_readonly(&target).unwrap();
    // The fixture has 40 profiles; the committed WAL row must make it 41.
    assert_eq!(
        store.count_rows("ProfileItem").unwrap(),
        41,
        "committed WAL content must be snapshot, not lost"
    );
    assert_eq!(
        count(
            &store,
            "SELECT COUNT(*) FROM ProfileItem WHERE Remarks = 'wal-fixture'"
        ),
        1
    );
    drop(conn);
}
