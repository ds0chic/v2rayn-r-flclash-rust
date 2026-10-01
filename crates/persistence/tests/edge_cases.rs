//! Edge cases: empty config, all-field config, null/empty/zero boundaries and
//! target-preservation on failure.

mod common;

use std::path::Path;

use persistence::{
    import_from_path, ImportFault, ImportOptions, ImportStatus, Store, UPSTREAM_TABLES,
};
use rusqlite::Connection;

fn options(fault: ImportFault) -> ImportOptions {
    ImportOptions { now: 1, fault }
}

fn create_db(path: &Path, insert: impl FnOnce(&Connection)) {
    let conn = Connection::open(path).unwrap();
    for table in UPSTREAM_TABLES {
        conn.execute_batch(&table.create_sql()).unwrap();
    }
    insert(&conn);
    conn.close().unwrap();
}

#[test]
fn empty_source_imports_zero_rows() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("guiNConfig.json"), "{}").unwrap();
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let target = dir.path().join("app.db");

    let report = import_from_path(dir.path(), &target, &work, &options(ImportFault::None)).unwrap();
    assert_eq!(report.status, ImportStatus::Imported);
    assert!(report.counts.iter().all(|c| c.source_rows == 0));
    assert!(target.exists());
}

#[test]
fn null_absent_and_empty_are_preserved_distinctly() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("guiNConfig.json"),
        r#"{"A":null,"B":"","UIItem":{"CurrentTheme":null,"CurrentLanguage":""},"Future":1}"#,
    )
    .unwrap();
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let target = dir.path().join("app.db");
    let report = import_from_path(dir.path(), &target, &work, &options(ImportFault::None)).unwrap();
    assert_eq!(report.status, ImportStatus::Imported);

    let store = Store::open_readonly(&target).unwrap();
    let stored: serde_json::Value =
        serde_json::from_str(&store.get_meta("upstream_config").unwrap().unwrap()).unwrap();
    assert!(stored["A"].is_null());
    assert_eq!(stored["B"], "");
    assert!(stored.get("C").is_none());
    assert!(stored["UIItem"]["CurrentTheme"].is_null());
    assert_eq!(stored["UIItem"]["CurrentLanguage"], "");
    assert_eq!(stored["Future"], 1);
}

#[test]
fn zero_port_is_a_warning_not_a_commit_blocker() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("guiNConfig.json"), "{}").unwrap();
    create_db(&dir.path().join("guiNDB.db"), |conn| {
        conn.execute(
            "INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Remarks, Address, Port) \
             VALUES ('bad-port', 5, 4, 'zero', '192.0.2.1', 0)",
            [],
        )
        .unwrap();
    });
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let target = dir.path().join("app.db");
    let report = import_from_path(dir.path(), &target, &work, &options(ImportFault::None)).unwrap();
    assert_eq!(report.status, ImportStatus::Imported);
    assert!(report.warnings.iter().any(|w| w.code == "W_FIELD"));
}

#[test]
fn all_fields_config_is_stored() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("guiNConfig.json"),
        r#"{
            "IndexId":"n1","SubIndexId":"s1",
            "CoreBasicItem":{"LogEnabled":true,"Loglevel":"debug"},
            "TunModeItem":{"EnableTun":false,"AutoRoute":true,"Mtu":1500},
            "KcpItem":{"Mtu":1350,"Tti":20},
            "GrpcItem":{"IdleTimeout":60},
            "RoutingBasicItem":{"DomainStrategy":"IPIfNonMatch"},
            "GuiItem":{"EnableLog":true,"TrayMenuServersLimit":25},
            "MsgUIItem":{"AutoRefresh":true},
            "UIItem":{"CurrentFontSize":13,"MainColumnItem":[{"Name":"Remarks","Width":200,"Index":0}]},
            "ConstItem":{"SubConvertUrl":""},
            "SpeedTestItem":{"SpeedTestTimeout":10},
            "Mux4RayItem":{"Concurrency":8},
            "Mux4SboxItem":{"Protocol":"h2mux","MaxConnections":4},
            "HysteriaItem":{"UpMbps":10,"DownMbps":20,"HopInterval":30},
            "ClashUIItem":{"EnableIPv6":true},
            "SystemProxyItem":{"SysProxyType":1},
            "WebDavItem":{"Url":null},
            "CheckUpdateItem":{"CheckPreReleaseUpdate":false},
            "Fragment4RayItem":{"Packets":"tlshello","Lengths":["100-200"]},
            "Inbound":[],
            "GlobalHotkeys":[],
            "CoreTypeItem":[],
            "SimpleDNSItem":{"FakeIP":true},
            "HappyEyeballs4RayItem":{"TryDelayMs":250}
        }"#,
    )
    .unwrap();
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let target = dir.path().join("app.db");
    let report = import_from_path(dir.path(), &target, &work, &options(ImportFault::None)).unwrap();
    assert_eq!(report.status, ImportStatus::Imported);

    let doc = {
        let store = Store::open_readonly(&target).unwrap();
        let raw = store.get_meta("upstream_config").unwrap().unwrap();
        persistence::ConfigDocument::parse(&raw).unwrap()
    };
    assert_eq!(doc.font_size(), Some(13));
    assert_eq!(doc.main_columns().unwrap()[0].width, 200);
    assert_eq!(doc.index_id(), "n1");
}

#[test]
fn failed_candidate_never_touches_an_existing_target() {
    let first = tempfile::tempdir().unwrap();
    std::fs::write(first.path().join("guiNConfig.json"), "{}").unwrap();
    create_db(&first.path().join("guiNDB.db"), |conn| {
        conn.execute(
            "INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Remarks) \
             VALUES ('keep', 5, 4, 'keep-me')",
            [],
        )
        .unwrap();
    });

    let target_dir = tempfile::tempdir().unwrap();
    let work = target_dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let target = target_dir.path().join("app.db");

    let ok = import_from_path(first.path(), &target, &work, &options(ImportFault::None)).unwrap();
    assert_eq!(ok.status, ImportStatus::Imported);
    let before = Store::open_readonly(&target)
        .unwrap()
        .count_rows("ProfileItem")
        .unwrap();

    let second = tempfile::tempdir().unwrap();
    std::fs::write(second.path().join("guiNConfig.json"), "{}").unwrap();
    create_db(&second.path().join("guiNDB.db"), |conn| {
        conn.execute(
            "INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Remarks) \
             VALUES ('other', 5, 4, 'other')",
            [],
        )
        .unwrap();
    });
    let failed = import_from_path(
        second.path(),
        &target,
        &work,
        &options(ImportFault::ValidationFailure),
    )
    .unwrap();
    assert_eq!(failed.status, ImportStatus::Failed);

    let after = Store::open_readonly(&target)
        .unwrap()
        .count_rows("ProfileItem")
        .unwrap();
    assert_eq!(before, after, "failed candidate must not change the target");
    assert!(!target_dir.path().join("app.db.bak").exists());
}
