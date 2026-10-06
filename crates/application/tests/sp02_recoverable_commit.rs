//! SP-02 red contract: recoverable cross-store settings commit.
//!
//! Correct expectations (stable-port plan §3.3 / §5.1, CP-06/07/10):
//!
//! - a save carries `dataset_epoch` + `mutation_id`; a stale epoch or a stale
//!   `expected_revision` is **Rejected with no `new_revision`**;
//! - a committed save returns `new_revision` + `saved_document_token` and
//!   survives an independent reopen;
//! - DB committed but file publish failed -> `CommitUnknown`, new writes are
//!   blocked with `RecoveryRequired` (never "nothing changed"), and recovery
//!   rolls forward to exactly one commit (mutation idempotent);
//! - crash before the DB commit -> reopen rolls back, the old snapshot is
//!   intact and new writes work again;
//! - recovery never serves a half-old/half-new writable snapshot: while a
//!   commit is unresolved the old file content stays readable and writes stay
//!   blocked;
//! - the small journal/receipt records carry only ids/hashes/stages, never
//!   secret material (the staged publish payload necessarily holds the
//!   document; the tracking records must not).
//!
//! All fixtures are synthetic (ports >= 11808, documentation values only).
//! Fault injection uses the commit test hook; the normal path uses real
//! SQLite + real files + independent reopen. No sockets, cores, helpers,
//! OS proxy/route/TUN/DNS writes or user secrets are involved.

use std::sync::Arc;

use application::recoverable_commit::CommitTestFault;
use application::runtime_client::NullRuntimeClient;
use application::AppEngine;
use domain::AppSettings;
use ipc_contract::stable::SettingsSaveState;

const SECRET_PROBE: &str = "synthetic-secret-probe-7f3a9c";

fn open_engine(dir: &std::path::Path) -> AppEngine {
    AppEngine::open_with_runtime(dir, Arc::new(NullRuntimeClient::new()))
        .unwrap_or_else(|error| panic!("open must succeed: {error}"))
}

fn settings_with_limit(base: &AppSettings, limit: i32) -> AppSettings {
    let mut next = base.clone();
    next.gui_item.tray_menu_servers_limit = limit;
    next
}

fn settings_with_secret(base: &AppSettings) -> AppSettings {
    let mut next = base.clone();
    next.gui_item.tray_menu_servers_limit = 27;
    next.extra.insert(
        "SyntheticProbe".to_string(),
        serde_json::json!({"note": SECRET_PROBE}),
    );
    next
}

fn current_limit(engine: &AppEngine) -> i32 {
    engine
        .load_settings()
        .expect("load must succeed")
        .settings
        .gui_item
        .tray_menu_servers_limit
}

#[test]
fn rejected_stale_revision_has_no_new_revision() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open_engine(dir.path());
    let loaded = engine.load_settings().unwrap();
    let receipt = engine.save_settings_commit(
        engine.dataset_epoch(),
        loaded.revision + 99,
        "m-stale-revision",
        settings_with_limit(&loaded.settings, 21),
    );
    assert_eq!(receipt.save, SettingsSaveState::Rejected, "{receipt:?}");
    assert_eq!(receipt.new_revision, None, "rejected saves pin no revision");
    assert_eq!(
        receipt.dataset_epoch,
        engine.dataset_epoch(),
        "receipt echoes the request epoch"
    );
    assert_eq!(
        current_limit(&engine),
        loaded.settings.gui_item.tray_menu_servers_limit
    );
}

#[test]
fn rejected_stale_epoch_has_no_new_revision() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open_engine(dir.path());
    let loaded = engine.load_settings().unwrap();
    let receipt = engine.save_settings_commit(
        engine.dataset_epoch() + 5,
        loaded.revision,
        "m-stale-epoch",
        settings_with_limit(&loaded.settings, 21),
    );
    assert_eq!(receipt.save, SettingsSaveState::Rejected, "{receipt:?}");
    assert_eq!(
        receipt.new_revision, None,
        "epoch mismatch pins no revision"
    );
}

#[test]
fn committed_save_returns_revision_and_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open_engine(dir.path());
    let loaded = engine.load_settings().unwrap();
    let receipt = engine.save_settings_commit(
        engine.dataset_epoch(),
        loaded.revision,
        "m-happy-path",
        settings_with_limit(&loaded.settings, 23),
    );
    assert_eq!(receipt.save, SettingsSaveState::Committed, "{receipt:?}");
    let new_revision = receipt.new_revision.expect("commit pins a revision");
    assert!(receipt.saved_document_token.is_some(), "{receipt:?}");
    assert!(!receipt.mutation_id.is_empty() && !receipt.commit_id.is_empty());

    // Same mutation replays the stored receipt without a second commit.
    let replay = engine.query_settings_mutation(engine.dataset_epoch(), "m-happy-path");
    assert_eq!(replay.save, SettingsSaveState::Committed, "{replay:?}");
    assert_eq!(replay.new_revision, Some(new_revision));
    assert_eq!(replay.commit_id, receipt.commit_id);

    drop(engine);
    let reopened = open_engine(dir.path());
    assert_eq!(current_limit(&reopened), 23);
    let again = reopened.query_settings_mutation(reopened.dataset_epoch(), "m-happy-path");
    assert_eq!(again.save, SettingsSaveState::Committed, "{again:?}");
    assert_eq!(again.new_revision, Some(new_revision));
}

#[test]
fn unknown_mutation_query_is_not_started_without_revision() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open_engine(dir.path());
    let receipt = engine.query_settings_mutation(engine.dataset_epoch(), "m-never-seen");
    assert_eq!(receipt.save, SettingsSaveState::NotStarted, "{receipt:?}");
    assert_eq!(receipt.new_revision, None);
    assert!(
        !receipt.phase_errors.is_empty(),
        "unknown query explains itself"
    );
}

#[test]
fn file_publish_failure_blocks_writes_until_roll_forward() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open_engine(dir.path());
    let loaded = engine.load_settings().unwrap();
    let before = loaded.settings.gui_item.tray_menu_servers_limit;

    engine.set_commit_test_fault(CommitTestFault::FailFilePublish);
    let failed = engine.save_settings_commit(
        engine.dataset_epoch(),
        loaded.revision,
        "m-publish-fails",
        settings_with_limit(&loaded.settings, 29),
    );
    engine.set_commit_test_fault(CommitTestFault::None);
    assert_eq!(failed.save, SettingsSaveState::CommitUnknown, "{failed:?}");
    assert_eq!(
        failed.new_revision, None,
        "unknown outcome pins no revision"
    );

    // The DB half is committed but the file half is not: the readable
    // snapshot stays the old one and new writes are blocked, never reported
    // as "nothing changed".
    assert_eq!(current_limit(&engine), before, "no half-new snapshot");
    assert!(engine.pending_commit_recovery(), "recovery must be pending");
    let blocked = engine.save_settings_commit(
        engine.dataset_epoch(),
        loaded.revision,
        "m-while-blocked",
        settings_with_limit(&loaded.settings, 30),
    );
    assert_eq!(
        blocked.save,
        SettingsSaveState::RecoveryRequired,
        "{blocked:?}"
    );
    assert_eq!(blocked.new_revision, None);

    // Crash-style reopen keeps the block until recovery confirms.
    drop(engine);
    let reopened = open_engine(dir.path());
    assert!(
        reopened.pending_commit_recovery(),
        "reopen still needs recovery"
    );
    assert_eq!(current_limit(&reopened), before);
    let still_blocked = reopened.save_settings_commit(
        reopened.dataset_epoch(),
        reopened.load_settings().unwrap().revision,
        "m-after-reopen",
        settings_with_limit(&reopened.load_settings().unwrap().settings, 31),
    );
    assert_eq!(still_blocked.save, SettingsSaveState::RecoveryRequired);

    // Recovery rolls forward to exactly one commit.
    let recovered = reopened.recover_pending_commits();
    assert!(!recovered.is_empty(), "recovery reports what it resolved");
    let done = reopened.query_settings_mutation(reopened.dataset_epoch(), "m-publish-fails");
    assert_eq!(done.save, SettingsSaveState::Committed, "{done:?}");
    assert_eq!(done.commit_id, failed.commit_id, "same commit, not a copy");
    let pinned = done.new_revision.expect("roll-forward pins a revision");
    assert_eq!(current_limit(&reopened), 29);
    assert!(!reopened.pending_commit_recovery());

    // New writes work again after recovery.
    let loaded2 = reopened.load_settings().unwrap();
    assert_eq!(loaded2.revision, pinned);
    let next = reopened.save_settings_commit(
        reopened.dataset_epoch(),
        loaded2.revision,
        "m-after-recovery",
        settings_with_limit(&loaded2.settings, 33),
    );
    assert_eq!(next.save, SettingsSaveState::Committed, "{next:?}");
}

#[test]
fn crash_before_db_commit_rolls_back_on_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open_engine(dir.path());
    let loaded = engine.load_settings().unwrap();
    let before = loaded.settings.gui_item.tray_menu_servers_limit;
    let revision_before = loaded.revision;

    engine.set_commit_test_fault(CommitTestFault::CrashAfterStage);
    let unknown = engine.save_settings_commit(
        engine.dataset_epoch(),
        revision_before,
        "m-crash-early",
        settings_with_limit(&loaded.settings, 37),
    );
    assert_eq!(
        unknown.save,
        SettingsSaveState::CommitUnknown,
        "{unknown:?}"
    );

    // Simulate the crash: drop without finishing, then reopen independently.
    drop(engine);
    let reopened = open_engine(dir.path());
    let recovered = reopened.recover_pending_commits();
    assert!(!recovered.is_empty(), "recovery reports the rollback");
    let outcome = reopened.query_settings_mutation(reopened.dataset_epoch(), "m-crash-early");
    assert_eq!(outcome.save, SettingsSaveState::Rejected, "{outcome:?}");
    assert_eq!(outcome.new_revision, None, "rolled back: nothing committed");
    assert_eq!(current_limit(&reopened), before, "old snapshot intact");
    assert_eq!(reopened.load_settings().unwrap().revision, revision_before);

    // The store accepts new writes again.
    let loaded2 = reopened.load_settings().unwrap();
    let next = reopened.save_settings_commit(
        reopened.dataset_epoch(),
        loaded2.revision,
        "m-after-rollback",
        settings_with_limit(&loaded2.settings, 39),
    );
    assert_eq!(next.save, SettingsSaveState::Committed, "{next:?}");
}

#[test]
fn mutation_replay_is_idempotent_and_conflict_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open_engine(dir.path());
    let loaded = engine.load_settings().unwrap();
    let first = engine.save_settings_commit(
        engine.dataset_epoch(),
        loaded.revision,
        "m-idempotent",
        settings_with_limit(&loaded.settings, 41),
    );
    assert_eq!(first.save, SettingsSaveState::Committed, "{first:?}");

    // Same mutation + same original parameters replays the stored receipt
    // exactly (the client retrying after a timeout), even though the live
    // revision has moved on.
    let replay = engine.save_settings_commit(
        engine.dataset_epoch(),
        loaded.revision,
        "m-idempotent",
        settings_with_limit(&loaded.settings, 41),
    );
    assert_eq!(replay.save, SettingsSaveState::Committed, "{replay:?}");
    assert_eq!(replay.commit_id, first.commit_id);
    assert_eq!(replay.new_revision, first.new_revision);
    assert_eq!(
        engine.load_settings().unwrap().revision,
        first.new_revision.unwrap(),
        "replay must not bump the revision again"
    );

    // Same mutation + different content is a conflict, not a silent overwrite.
    let loaded3 = engine.load_settings().unwrap();
    let conflict = engine.save_settings_commit(
        engine.dataset_epoch(),
        loaded3.revision,
        "m-idempotent",
        settings_with_limit(&loaded.settings, 43),
    );
    assert_eq!(conflict.save, SettingsSaveState::Rejected, "{conflict:?}");
    assert_eq!(conflict.new_revision, None);
    assert_eq!(current_limit(&engine), 41);
}

#[test]
fn journal_tracking_records_carry_no_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open_engine(dir.path());
    let loaded = engine.load_settings().unwrap();
    let receipt = engine.save_settings_commit(
        engine.dataset_epoch(),
        loaded.revision,
        "m-with-secret",
        settings_with_secret(&loaded.settings),
    );
    assert_eq!(receipt.save, SettingsSaveState::Committed, "{receipt:?}");
    drop(engine);

    // Tracking records (pending journals, done receipts, markers) hold only
    // ids/hashes/stages. The staged publish payload holds the document by
    // design and is excluded here; it is cleaned after a full commit.
    let journal = dir.path().join("commit_journal");
    let mut tracked = Vec::new();
    if journal.exists() {
        let mut stack = vec![journal.clone()];
        while let Some(next) = stack.pop() {
            for entry in std::fs::read_dir(&next).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    // Staging payloads are content, not tracking records.
                    if path.file_name().map(|n| n == "staging").unwrap_or(false) {
                        continue;
                    }
                    stack.push(path);
                } else {
                    tracked.push(path);
                }
            }
        }
    }
    assert!(
        !tracked.is_empty(),
        "done receipts must be kept for queries"
    );
    for path in &tracked {
        let text = std::fs::read_to_string(path).unwrap();
        assert!(
            !text.contains(SECRET_PROBE),
            "tracking record must not carry secrets: {}",
            path.display()
        );
    }
    let receipt_json = serde_json::to_string(&receipt).unwrap();
    assert!(
        !receipt_json.contains(SECRET_PROBE),
        "receipt is metadata only"
    );
}
