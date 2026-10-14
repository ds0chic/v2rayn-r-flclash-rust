//! Ownership-journal file behaviour: round-trip, corruption quarantine and
//! the quarantine cap. Fake protect backends and temp directories only; the
//! live proxy is never touched and no network is used.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use platform::sysproxy::journal::{
    clear_journal, journal_path, load_journal, save_journal, JournalLoad, OwnershipJournal,
    StoredOwner, MAX_QUARANTINE_KEPT,
};
use platform::{AppliedChange, FakeProtectBackend, PlainBackend, ProxyField};

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn unique_dir(tag: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "v2rayn-r-journal-it-{tag}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn owner() -> StoredOwner {
    StoredOwner {
        pid: 4242,
        created_at_ms: 1_700_000_000_000,
    }
}

fn ledger() -> Vec<AppliedChange> {
    vec![
        AppliedChange {
            field: ProxyField::Enabled,
            before: Some("0".to_string()),
            after: Some("1".to_string()),
        },
        AppliedChange {
            field: ProxyField::Server,
            before: Some("old:1".to_string()),
            after: Some("127.0.0.1:11809".to_string()),
        },
    ]
}

#[test]
fn save_load_roundtrip_preserves_ledger_and_owner() {
    let dir = unique_dir("roundtrip");
    let path = journal_path(&dir);
    let journal = OwnershipJournal::new(owner(), ledger());
    save_journal(&path, &journal, &FakeProtectBackend::new(11)).expect("save");
    match load_journal(&path, &FakeProtectBackend::new(11)).expect("load") {
        JournalLoad::Loaded(back) => assert_eq!(back, journal),
        other => panic!("expected Loaded, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_file_loads_as_absent_and_clear_is_idempotent() {
    let dir = unique_dir("absent");
    let path = journal_path(&dir);
    assert!(matches!(
        load_journal(&path, &PlainBackend).expect("load"),
        JournalLoad::Absent
    ));
    clear_journal(&path).expect("clear absent");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn garbage_file_is_quarantined_not_loaded() {
    let dir = unique_dir("garbage");
    let path = journal_path(&dir);
    std::fs::write(&path, b"definitely not a journal").expect("write garbage");
    match load_journal(&path, &PlainBackend).expect("load") {
        JournalLoad::Quarantined(moved) => {
            assert!(moved.is_file(), "quarantine target must exist");
            assert!(
                moved
                    .file_name()
                    .expect("name")
                    .to_string_lossy()
                    .starts_with("sysproxy-ownership.journal.corrupt-"),
                "quarantine name must be filename-only, got {}",
                moved.display()
            );
        }
        other => panic!("expected Quarantined, got {other:?}"),
    }
    assert!(!path.exists(), "corrupt file must move aside");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn quarantine_keeps_only_the_latest_three() {
    let dir = unique_dir("cap");
    // Pre-seed four stale quarantines, then trigger one more.
    for n in 0..4 {
        std::fs::write(
            dir.join(format!("sysproxy-ownership.journal.corrupt-{n:020}")),
            b"stale",
        )
        .expect("seed");
    }
    let path = journal_path(&dir);
    std::fs::write(&path, b"corrupt").expect("corrupt journal");
    let loaded = load_journal(&path, &PlainBackend).expect("load");
    assert!(
        matches!(loaded, JournalLoad::Quarantined(_)),
        "got {loaded:?}"
    );
    let kept: Vec<_> = std::fs::read_dir(&dir)
        .expect("read dir")
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("sysproxy-ownership.journal.corrupt-")
        })
        .collect();
    assert_eq!(
        kept.len(),
        MAX_QUARANTINE_KEPT,
        "quarantine must cap at {MAX_QUARANTINE_KEPT}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
