//! SP-03 red contract: canonical identity across backup/restore.
//!
//! Correct expectations (stable-port plan §3.1 / §5.2, CP-07 / CP-SET-04):
//!
//! - `set_active(B)` writes the **canonical** `IndexId` and the engine-owned
//!   `active_index_id` with the same value, so a later backup carries B (no
//!   identity drift between the new private field and the upstream field).
//! - backup → change selection/default → restore → independent reopen selects
//!   the backed-up default B (or the visible first row when B is gone),
//!   following upstream `ProfilesViewModel.RefreshServersBiz` (361–375):
//!   in-memory pending selection first, then the persisted `IndexId` default,
//!   then the first visible row. A temporary table selection of C never
//!   rewrites the persisted default.
//! - every restore/import replacement advances the persisted `dataset_epoch`;
//!   requests carrying the pre-restore epoch are rejected afterwards
//!   (`save_settings_commit` pins no `new_revision` for them), and a fresh
//!   process reopen observes the advanced epoch.
//! - the current group (`SubIndexId`) round-trips through backup/restore and
//!   resolves against the restored subscriptions (unknown group = All/None).
//!
//! All fixtures are synthetic (TEST-NET addresses, reserved ports, no user
//! secrets). Scratch tempdirs only; no sockets, cores, helpers, OS
//! proxy/route/TUN/DNS writes or port 10808 are involved.

use std::sync::Arc;

use application::runtime_client::NullRuntimeClient;
use application::{AppEngine, BackupService, SubItem};
use domain::{ConfigType, DesiredRevision, Profile};
use ipc_contract::stable::SettingsSaveState;

const NODE_A: &str = "sp03-node-a";
const NODE_B: &str = "sp03-node-b";
const GROUP_G: &str = "sp03-sub-g";

fn open_engine(dir: &std::path::Path) -> AppEngine {
    AppEngine::open_with_runtime(dir, Arc::new(NullRuntimeClient::new()))
        .unwrap_or_else(|error| panic!("open must succeed: {error}"))
}

fn synthetic_node(id: &str, remarks: &str) -> Profile {
    Profile {
        index_id: id.to_string(),
        config_type: ConfigType::Vless,
        remarks: remarks.to_string(),
        address: "192.0.2.10".into(),
        port: 443,
        password: "11111111-2222-3333-4444-555555555555".into(),
        subid: GROUP_G.to_string(),
        ..Default::default()
    }
}

fn seed_group(engine: &AppEngine) {
    engine
        .save_sub_item(SubItem {
            id: GROUP_G.into(),
            remarks: "group-g".into(),
            url: "https://example.com/s".into(),
            enabled: true,
            ..SubItem::default()
        })
        .expect("seed subscription group");
}

fn seed_nodes(engine: &AppEngine) {
    seed_group(engine);
    let mut revision = DesiredRevision::new(engine.desired_revision());
    for (id, remarks) in [(NODE_A, "A"), (NODE_B, "B")] {
        let (_, next) = engine
            .save_imported_profile(synthetic_node(id, remarks), revision)
            .expect("seed node");
        revision = next;
    }
}

fn set_current_group(engine: &AppEngine, group: &str) {
    let loaded = engine.load_settings().expect("load settings");
    let group_revision = loaded
        .group_revisions
        .get("SubIndexId")
        .copied()
        .unwrap_or(0);
    engine
        .save_settings_group(
            "SubIndexId",
            serde_json::Value::String(group.to_string()),
            group_revision,
        )
        .expect("set current group");
}

fn read_config(dir: &std::path::Path) -> serde_json::Value {
    let text = std::fs::read_to_string(dir.join("guiNConfig.json")).expect("read config");
    serde_json::from_str(&text).expect("parse config")
}

fn config_str(config: &serde_json::Value, key: &str) -> Option<String> {
    config.get(key).and_then(|v| v.as_str()).map(str::to_string)
}

#[test]
fn set_active_unifies_canonical_index_id() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open_engine(dir.path());
    seed_nodes(&engine);

    engine
        .set_active(Some(NODE_B.into()))
        .expect("set active B");

    let config = read_config(dir.path());
    assert_eq!(
        config_str(&config, "IndexId").as_deref(),
        Some(NODE_B),
        "canonical IndexId must follow the active node"
    );
    assert_eq!(
        config_str(&config, "active_index_id").as_deref(),
        Some(NODE_B),
        "engine mirror must equal the canonical IndexId"
    );
}

#[test]
fn backup_carries_unified_canonical_identity() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open_engine(dir.path());
    seed_nodes(&engine);
    set_current_group(&engine, GROUP_G);
    engine
        .set_active(Some(NODE_B.into()))
        .expect("set active B");

    let service = BackupService::new(dir.path());
    let parent = tempfile::tempdir().unwrap();
    let bundle_root = parent.path().join("bundle");
    service.create_local(&bundle_root, 1).expect("backup");

    let bundled: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(bundle_root.join("guiNConfig.json")).expect("read bundled config"),
    )
    .expect("parse bundled config");
    assert_eq!(
        config_str(&bundled, "IndexId").as_deref(),
        Some(NODE_B),
        "backup must carry the active node as canonical IndexId"
    );
    assert_eq!(
        config_str(&bundled, "SubIndexId").as_deref(),
        Some(GROUP_G),
        "backup must carry the current group"
    );
}

#[test]
fn backup_change_restore_reopen_selects_backed_up_default() {
    let dir = tempfile::tempdir().unwrap();
    let epoch_before;
    {
        let engine = open_engine(dir.path());
        seed_nodes(&engine);
        set_current_group(&engine, GROUP_G);
        engine.set_active(Some(NODE_B.into())).expect("default B");
        epoch_before = engine.dataset_epoch();

        let service = BackupService::new(dir.path());
        let parent = tempfile::tempdir().unwrap();
        let bundle_root = parent.path().join("bundle");
        service.create_local(&bundle_root, 1).expect("backup");

        // Post-backup change: the default moves to A while the bundle keeps B.
        engine.set_active(Some(NODE_A.into())).expect("change to A");
        assert_eq!(engine.active_profile().as_deref(), Some(NODE_A));

        // Production path: quiesce -> exchange -> reopen.
        service
            .restore_with_lifecycle(&engine, &bundle_root, &dir.path().join(".work"))
            .expect("restore");
        assert_eq!(
            engine.active_profile().as_deref(),
            Some(NODE_B),
            "restore must bring back the backed-up default"
        );
        assert!(
            engine.dataset_epoch() > epoch_before,
            "restore must advance the dataset epoch"
        );
    }
    // Independent reopen (fresh process): B or the visible first row.
    let engine = open_engine(dir.path());
    assert_eq!(
        engine.active_profile().as_deref(),
        Some(NODE_B),
        "reopen must select the restored default B"
    );
    let config = read_config(dir.path());
    assert_eq!(config_str(&config, "IndexId").as_deref(), Some(NODE_B));
    assert_eq!(
        config_str(&config, "active_index_id").as_deref(),
        Some(NODE_B),
        "reopen must keep the dual identity unified"
    );
    assert_eq!(
        config_str(&config, "SubIndexId").as_deref(),
        Some(GROUP_G),
        "reopen must keep the restored current group"
    );
    assert!(
        engine.profile_by_id(NODE_B).expect("lookup B").is_some(),
        "the selected default must exist in the restored database"
    );
}

#[test]
fn reopen_repairs_dangling_default_to_first_visible_row() {
    let dir = tempfile::tempdir().unwrap();
    {
        let engine = open_engine(dir.path());
        seed_nodes(&engine);
        engine.set_active(Some(NODE_B.into())).expect("default B");
    }
    // B disappears out of band (e.g. a restore whose database no longer
    // carries it); the config still names B.
    {
        let store = persistence::Store::open(dir.path().join("guiNDB.db")).expect("open store");
        let tx = store.begin().expect("begin");
        tx.execute(
            "DELETE FROM \"ProfileItem\" WHERE \"IndexId\" = ?1",
            rusqlite::params![NODE_B],
        )
        .expect("delete B");
        tx.commit().expect("commit");
    }
    let engine = open_engine(dir.path());
    assert_eq!(
        engine.active_profile().as_deref(),
        Some(NODE_A),
        "a dangling default must fall back to the visible first row (upstream SetDefaultServer)"
    );
    let config = read_config(dir.path());
    assert_eq!(
        config_str(&config, "IndexId").as_deref(),
        Some(NODE_A),
        "the repair must unify the canonical IndexId"
    );
    assert_eq!(
        config_str(&config, "active_index_id").as_deref(),
        Some(NODE_A),
        "the repair must unify the engine mirror"
    );
}

#[test]
fn legacy_config_with_only_index_id_resolves() {
    let dir = tempfile::tempdir().unwrap();
    {
        let engine = open_engine(dir.path());
        seed_nodes(&engine);
    }
    // Upstream-written file: canonical IndexId only, no engine mirror keys.
    std::fs::write(
        dir.path().join("guiNConfig.json"),
        format!(r#"{{"IndexId":"{NODE_A}","SubIndexId":"{GROUP_G}"}}"#),
    )
    .expect("write legacy config");

    let engine = open_engine(dir.path());
    assert_eq!(
        engine.active_profile().as_deref(),
        Some(NODE_A),
        "a legacy file carrying only IndexId must resolve the default"
    );
}

#[test]
fn restore_advances_epoch_and_rejects_stale_requests() {
    let dir = tempfile::tempdir().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let bundle_root = parent.path().join("bundle");
    let stale_epoch;
    let stale_revision;
    {
        let engine = open_engine(dir.path());
        seed_nodes(&engine);
        engine.set_active(Some(NODE_B.into())).expect("default B");
        stale_epoch = engine.dataset_epoch();
        let loaded = engine.load_settings().expect("load settings");
        stale_revision = loaded.revision;

        let service = BackupService::new(dir.path());
        service.create_local(&bundle_root, 1).expect("backup");
        service
            .restore_with_lifecycle(&engine, &bundle_root, &dir.path().join(".work"))
            .expect("restore");
        assert!(
            engine.dataset_epoch() > stale_epoch,
            "restore must advance the dataset epoch"
        );

        // A pre-restore request must not take effect afterwards.
        let loaded = engine.load_settings().expect("reload settings");
        let receipt = engine.save_settings_commit(
            stale_epoch,
            loaded.revision,
            "m-sp03-stale",
            loaded.settings.clone(),
        );
        assert_eq!(receipt.save, SettingsSaveState::Rejected, "{receipt:?}");
        assert_eq!(receipt.new_revision, None, "rejected saves pin no revision");

        // The current epoch still works.
        let receipt = engine.save_settings_commit(
            engine.dataset_epoch(),
            loaded.revision,
            "m-sp03-fresh",
            loaded.settings.clone(),
        );
        assert_eq!(receipt.save, SettingsSaveState::Committed, "{receipt:?}");
        assert!(receipt.new_revision.is_some());
    }
    // A fresh process observes the advanced epoch, so replaying the old
    // revision/epoch pair from before the restore stays rejected.
    let engine = open_engine(dir.path());
    assert!(
        engine.dataset_epoch() > stale_epoch,
        "the advanced epoch must survive an independent reopen"
    );
    let loaded = engine.load_settings().expect("load settings");
    assert_ne!(loaded.revision, stale_revision);
    let receipt = engine.save_settings_commit(
        stale_epoch,
        stale_revision,
        "m-sp03-replay",
        loaded.settings.clone(),
    );
    assert_eq!(
        receipt.save,
        SettingsSaveState::Rejected,
        "old epoch requests must stay rejected after reopen: {receipt:?}"
    );
    assert_eq!(receipt.new_revision, None);
}

#[test]
fn sequential_restores_advance_epoch_monotonically() {
    let dir = tempfile::tempdir().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let bundle_root = parent.path().join("bundle");
    let engine = open_engine(dir.path());
    seed_nodes(&engine);
    engine.set_active(Some(NODE_B.into())).expect("default B");
    let service = BackupService::new(dir.path());
    service.create_local(&bundle_root, 1).expect("backup");

    let first = engine.dataset_epoch();
    service
        .restore_with_lifecycle(&engine, &bundle_root, &dir.path().join(".work"))
        .expect("first restore");
    let second = engine.dataset_epoch();
    assert!(second > first, "first restore must advance the epoch");
    service
        .restore_with_lifecycle(&engine, &bundle_root, &dir.path().join(".work"))
        .expect("second restore");
    assert!(
        engine.dataset_epoch() > second,
        "re-restoring the same bundle must advance the epoch again"
    );
    assert_eq!(
        engine.active_profile().as_deref(),
        Some(NODE_B),
        "repeated restores keep selecting B"
    );
}

#[test]
fn restore_without_bundle_config_fails_closed_on_corrupt_live_config() {
    let dir = tempfile::tempdir().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let bundle_root = parent.path().join("bundle");
    {
        let engine = open_engine(dir.path());
        seed_nodes(&engine);
        engine.set_active(Some(NODE_B.into())).expect("default B");
        let service = BackupService::new(dir.path());
        service.create_local(&bundle_root, 1).expect("backup");
    }
    // A DB-only bundle: drop the config member and clear its manifest hash.
    std::fs::remove_file(bundle_root.join("guiNConfig.json")).expect("drop bundled config");
    let manifest_path = bundle_root.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    manifest["config_sha256"] = serde_json::Value::Null;
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("write manifest"),
    )
    .expect("rewrite manifest");
    // Damage the live config while the database stays valid.
    std::fs::write(dir.path().join("guiNConfig.json"), b"{ not json").expect("corrupt live");

    let service = BackupService::new(dir.path());
    let result = service.restore(&bundle_root, &dir.path().join(".work"));
    assert!(
        result.is_err(),
        "epoch publish must fail closed: {result:?}"
    );
    // The live database is rolled back to the pre-restore rows...
    let store = persistence::Store::open(dir.path().join("guiNDB.db")).expect("open db");
    assert_eq!(store.count_rows("ProfileItem").expect("count"), 2);
    // ...and the corrupt live config is preserved untouched for recovery,
    // never paved over with a fresh document.
    assert_eq!(
        std::fs::read(dir.path().join("guiNConfig.json")).expect("read live config"),
        b"{ not json",
    );
}

#[test]
fn upstream_zip_roundtrip_keeps_dual_identity_unified() {
    let dir = tempfile::tempdir().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let bundle_root = parent.path().join("bundle");
    {
        let engine = open_engine(dir.path());
        seed_nodes(&engine);
        set_current_group(&engine, GROUP_G);
        engine.set_active(Some(NODE_B.into())).expect("default B");
        let service = BackupService::new(dir.path());
        service.create_local(&bundle_root, 1).expect("backup");
    }
    // Upstream-interoperable layout, as served to WebDAV/original clients.
    let bytes = application::zip_upstream_layout(&bundle_root).expect("zip upstream layout");
    let zip_path = parent.path().join("backup.zip");
    std::fs::write(&zip_path, &bytes).expect("write zip");

    let target = tempfile::tempdir().unwrap();
    let engine = open_engine(target.path());
    let service = BackupService::new(target.path());
    let report = service
        .import_upstream_with_lifecycle(&engine, &zip_path, &target.path().join(".work"), 1)
        .expect("import upstream zip");
    assert_eq!(report.status, persistence::ImportStatus::Imported);

    let active = engine.active_profile().expect("active after import");
    let config = read_config(target.path());
    assert_eq!(
        config_str(&config, "IndexId").as_deref(),
        Some(active.as_str()),
        "import activation must unify canonical IndexId with the active node"
    );
    assert_eq!(
        config_str(&config, "active_index_id").as_deref(),
        Some(active.as_str())
    );
    assert!(
        engine
            .profile_by_id(&active)
            .expect("lookup active")
            .is_some(),
        "the activated node must exist in the restored database"
    );
}

#[test]
fn sub_index_id_roundtrips_backup_restore() {
    let dir = tempfile::tempdir().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let bundle_root = parent.path().join("bundle");
    {
        let engine = open_engine(dir.path());
        seed_nodes(&engine);
        set_current_group(&engine, GROUP_G);
        engine.set_active(Some(NODE_B.into())).expect("default B");
        let service = BackupService::new(dir.path());
        service.create_local(&bundle_root, 1).expect("backup");

        // Post-backup drift that the restore must undo.
        set_current_group(&engine, "");
        engine.set_active(Some(NODE_A.into())).expect("change to A");
        service
            .restore_with_lifecycle(&engine, &bundle_root, &dir.path().join(".work"))
            .expect("restore");
    }
    let engine = open_engine(dir.path());
    let config = read_config(dir.path());
    assert_eq!(
        config_str(&config, "SubIndexId").as_deref(),
        Some(GROUP_G),
        "restore must bring back the backed-up current group"
    );
    let groups: Vec<String> = engine
        .list_sub_items()
        .expect("list subs")
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert!(
        groups.contains(&GROUP_G.to_string()),
        "the restored group must exist in the restored database"
    );
}
