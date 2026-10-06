//! Recoverable two-phase commit journal (SP-02).
//!
//! Coordinates one SQLite transaction with one config-file publish so a crash
//! or a failure at any stage reopens into a consistent snapshot:
//!
//! ```text
//! begin (stage + journal + staged file) -> db commit -> mark_db_committed
//!   -> publish staged file -> finish (done receipt, journal removed)
//! ```
//!
//! A crash before the DB commit rolls back (staged content is discarded); a
//! crash after the DB commit rolls forward (the staged file is published).
//! While a journal is unresolved the caller must block new writes and keep
//! serving the old file content, never a half-old/half-new snapshot.
//!
//! The journal record carries only identities, hashes and stages — never
//! document content or secrets. The staged publish payload necessarily holds
//! the document bytes; it lives under `staging/` and is removed by `finish`
//! or `abandon`. Nothing here logs document content.
//!
//! Fault injection ([`CommitFault`]) is test-only vocabulary, mirroring the
//! `ImportFault` precedent: production callers always pass [`CommitFault::None`].

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{PersistenceError, Result};

/// Directory (under the application data dir) holding journals, staged
/// payloads, done receipts and the recovery marker.
pub const JOURNAL_DIR: &str = "commit_journal";
const STAGING_DIR: &str = "staging";
const DONE_DIR: &str = "done";
const MARKER_FILE: &str = "recovery_required";
const JOURNAL_SUFFIX: &str = ".json";
const TMP_SUFFIX: &str = ".tmp";

/// Fault injection for commit tests. Production passes [`CommitFault::None`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommitFault {
    #[default]
    None,
    /// Fail while writing the journal/staged payload.
    FailJournalWrite,
    /// Fail the SQLite transaction (consumed by the commit coordinator).
    FailDbCommit,
    /// Fail the staged-file publish after the DB commit.
    FailFilePublish,
    /// Stop right after staging, as if the process crashed before the DB
    /// commit (consumed by the commit coordinator).
    CrashAfterStage,
    /// Stop right after the DB commit, as if the process crashed before the
    /// file publish (consumed by the commit coordinator).
    CrashAfterDbCommit,
}

/// Durable stage of one mutation. `FilesPublished` is terminal and leaves no
/// journal behind, so it is not stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitStage {
    Staged,
    DbCommitted,
}

/// Tracking record for one mutation. Metadata only: identities, hashes and
/// the stage. The document bytes stay in the staged payload file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalRecord {
    pub mutation_id: String,
    pub commit_id: String,
    pub commit_epoch: u64,
    pub dataset_epoch: u64,
    pub expected_revision: u64,
    /// Hash of the canonical normalised settings: the idempotency key. A
    /// replay with the same mutation id and the same settings hash returns
    /// the stored receipt; different content is a conflict.
    pub content_hash: String,
    /// Hash of the exact staged payload bytes: the publish integrity key.
    /// Recovery refuses to publish a staged file whose bytes do not match.
    pub doc_hash: String,
    pub stage: CommitStage,
    /// Staged payload path, relative to the data dir.
    pub staged_file: String,
    /// Publish target path, relative to the data dir.
    pub target_file: String,
    pub created_at_secs: u64,
}

/// Stored completion of one mutation: the serialised receipt plus the content
/// hash it committed, so a replay with different content is a conflict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoneRecord {
    pub receipt_json: String,
    pub content_hash: String,
}

pub fn journal_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(JOURNAL_DIR)
}

fn journal_path(data_dir: &Path, mutation_id: &str) -> PathBuf {
    journal_dir(data_dir).join(format!("{mutation_id}{JOURNAL_SUFFIX}"))
}

fn done_path(data_dir: &Path, mutation_id: &str) -> PathBuf {
    journal_dir(data_dir)
        .join(DONE_DIR)
        .join(format!("{mutation_id}{JOURNAL_SUFFIX}"))
}

fn marker_path(data_dir: &Path) -> PathBuf {
    journal_dir(data_dir).join(MARKER_FILE)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Build a [`JournalRecord`] skeleton for a new mutation. Pure constructor so
/// coordinators share the same identity rules.
#[allow(clippy::too_many_arguments)]
pub fn new_record(
    mutation_id: &str,
    commit_id: &str,
    commit_epoch: u64,
    dataset_epoch: u64,
    expected_revision: u64,
    content_hash: &str,
    doc_hash: &str,
    staged_file: &str,
    target_file: &str,
) -> JournalRecord {
    JournalRecord {
        mutation_id: mutation_id.to_string(),
        commit_id: commit_id.to_string(),
        commit_epoch,
        dataset_epoch,
        expected_revision,
        content_hash: content_hash.to_string(),
        doc_hash: doc_hash.to_string(),
        stage: CommitStage::Staged,
        staged_file: staged_file.to_string(),
        target_file: target_file.to_string(),
        created_at_secs: now_secs(),
    }
}

/// Write the staged payload + journal record (tmp + rename, best-effort
/// fsync). The target file is untouched.
pub fn begin(
    data_dir: &Path,
    record: &JournalRecord,
    staged_text: &str,
    fault: CommitFault,
) -> Result<PathBuf> {
    if fault == CommitFault::FailJournalWrite {
        return Err(PersistenceError::internal(
            "E_FAULT_INJECTED: journal write failed",
        ));
    }
    let staged = data_dir.join(&record.staged_file);
    if let Some(parent) = staged.parent() {
        std::fs::create_dir_all(parent)?;
    }
    atomic_write(&staged, staged_text.as_bytes())?;
    let path = journal_path(data_dir, &record.mutation_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(record)?;
    atomic_write(&path, text.as_bytes())?;
    Ok(staged)
}

/// Advance a journal to [`CommitStage::DbCommitted`].
pub fn mark_db_committed(data_dir: &Path, mutation_id: &str) -> Result<JournalRecord> {
    let mut record = load(data_dir, mutation_id)?.ok_or_else(|| {
        PersistenceError::internal(format!("missing journal for mutation {mutation_id}"))
    })?;
    record.stage = CommitStage::DbCommitted;
    let text = serde_json::to_string_pretty(&record)?;
    atomic_write(&journal_path(data_dir, mutation_id), text.as_bytes())?;
    Ok(record)
}

/// Publish the staged payload to its target (fsync staged, atomic rename,
/// best-effort sync). Either the old target or the full new document is on
/// disk afterwards, never a prefix.
pub fn publish_staged(data_dir: &Path, mutation_id: &str, fault: CommitFault) -> Result<()> {
    if fault == CommitFault::FailFilePublish {
        return Err(PersistenceError::internal(
            "E_FAULT_INJECTED: staged file publish failed",
        ));
    }
    let record = load(data_dir, mutation_id)?.ok_or_else(|| {
        PersistenceError::internal(format!("missing journal for mutation {mutation_id}"))
    })?;
    let staged = data_dir.join(&record.staged_file);
    let target = data_dir.join(&record.target_file);
    sync_file(&staged)?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(&staged, &target)?;
    sync_file(&target)?;
    Ok(())
}

/// Record completion: store the done receipt, then remove the journal and any
/// staged payload. Idempotent for an already-finished mutation.
pub fn finish(data_dir: &Path, mutation_id: &str, done: &DoneRecord) -> Result<()> {
    let path = done_path(data_dir, mutation_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(done)?;
    atomic_write(&path, text.as_bytes())?;
    let journal = journal_path(data_dir, mutation_id);
    if journal.exists() {
        std::fs::remove_file(&journal)?;
    }
    let staging = journal_dir(data_dir).join(STAGING_DIR).join(mutation_id);
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    Ok(())
}

/// Roll back a mutation that never reached the DB commit: discard the staged
/// payload and its journal. The live target was never touched.
pub fn abandon(data_dir: &Path, mutation_id: &str) -> Result<()> {
    let journal = journal_path(data_dir, mutation_id);
    if journal.exists() {
        std::fs::remove_file(&journal)?;
    }
    let staging = journal_dir(data_dir).join(STAGING_DIR).join(mutation_id);
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    Ok(())
}

/// All unresolved journals, oldest first.
pub fn pending(data_dir: &Path) -> Result<Vec<JournalRecord>> {
    let dir = journal_dir(data_dir);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.ends_with(JOURNAL_SUFFIX)
            || name.ends_with(&format!("{TMP_SUFFIX}{JOURNAL_SUFFIX}"))
        {
            continue;
        }
        if name == MARKER_FILE {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        out.push(serde_json::from_str::<JournalRecord>(&text)?);
    }
    out.sort_by_key(|r| (r.created_at_secs, r.mutation_id.clone()));
    Ok(out)
}

pub fn load(data_dir: &Path, mutation_id: &str) -> Result<Option<JournalRecord>> {
    let path = journal_path(data_dir, mutation_id);
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_str(&std::fs::read_to_string(
        &path,
    )?)?))
}

pub fn load_receipt(data_dir: &Path, mutation_id: &str) -> Result<Option<DoneRecord>> {
    let path = done_path(data_dir, mutation_id);
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_str(&std::fs::read_to_string(
        &path,
    )?)?))
}

/// Whether new writes must stay blocked until recovery confirms.
pub fn recovery_required(data_dir: &Path) -> bool {
    marker_path(data_dir).exists()
}

pub fn set_recovery_required(data_dir: &Path) -> Result<()> {
    let dir = journal_dir(data_dir);
    std::fs::create_dir_all(&dir)?;
    atomic_write(&marker_path(data_dir), b"recovery_required")?;
    Ok(())
}

pub fn clear_recovery_required(data_dir: &Path) -> Result<()> {
    let marker = marker_path(data_dir);
    if marker.exists() {
        std::fs::remove_file(&marker)?;
    }
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(TMP_SUFFIX);
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    sync_file(&tmp)?;
    std::fs::rename(&tmp, path)?;
    if let Some(parent) = path.parent() {
        sync_dir(parent);
    }
    Ok(())
}

fn sync_file(path: &Path) -> Result<()> {
    // A read-only handle cannot flush on Windows (os error 5); open for
    // writing without truncating. No bytes are written through this handle.
    let file = std::fs::OpenOptions::new().write(true).open(path)?;
    file.sync_all()?;
    Ok(())
}

fn sync_dir(dir: &Path) {
    if let Ok(file) = std::fs::File::open(dir) {
        let _ = file.sync_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(mutation: &str) -> JournalRecord {
        new_record(
            mutation,
            "commit-1",
            1,
            0,
            7,
            "hash-abc",
            "doc-hash-abc",
            &format!("{JOURNAL_DIR}/{STAGING_DIR}/{mutation}/guiNConfig.json"),
            "guiNConfig.json",
        )
    }

    #[test]
    fn begin_publish_finish_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let rec = record("m-1");
        begin(dir.path(), &rec, r#"{"a":1}"#, CommitFault::None).unwrap();
        assert_eq!(pending(dir.path()).unwrap().len(), 1);
        mark_db_committed(dir.path(), "m-1").unwrap();
        publish_staged(dir.path(), "m-1", CommitFault::None).unwrap();
        let text = std::fs::read_to_string(dir.path().join("guiNConfig.json")).unwrap();
        assert_eq!(text, r#"{"a":1}"#);
        finish(
            dir.path(),
            "m-1",
            &DoneRecord {
                receipt_json: "{}".into(),
                content_hash: "hash-abc".into(),
            },
        )
        .unwrap();
        assert!(pending(dir.path()).unwrap().is_empty());
        assert_eq!(
            load_receipt(dir.path(), "m-1")
                .unwrap()
                .unwrap()
                .content_hash,
            "hash-abc"
        );
    }

    #[test]
    fn journal_write_fault_leaves_no_trace() {
        let dir = tempfile::tempdir().unwrap();
        let rec = record("m-fault");
        let err = begin(dir.path(), &rec, "{}", CommitFault::FailJournalWrite).unwrap_err();
        assert_eq!(err.code(), crate::error::codes::INTERNAL);
        assert!(pending(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn publish_fault_leaves_target_untouched() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("guiNConfig.json"), r#"{"old":true}"#).unwrap();
        let rec = record("m-pub");
        begin(dir.path(), &rec, r#"{"new":true}"#, CommitFault::None).unwrap();
        mark_db_committed(dir.path(), "m-pub").unwrap();
        assert!(publish_staged(dir.path(), "m-pub", CommitFault::FailFilePublish).is_err());
        let text = std::fs::read_to_string(dir.path().join("guiNConfig.json")).unwrap();
        assert_eq!(text, r#"{"old":true}"#);
        assert_eq!(pending(dir.path()).unwrap().len(), 1);
    }

    #[test]
    fn abandon_discards_staged_payload() {
        let dir = tempfile::tempdir().unwrap();
        let rec = record("m-ab");
        begin(dir.path(), &rec, "{}", CommitFault::None).unwrap();
        abandon(dir.path(), "m-ab").unwrap();
        assert!(pending(dir.path()).unwrap().is_empty());
        assert!(!dir.path().join("guiNConfig.json").exists());
    }

    #[test]
    fn recovery_marker_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!recovery_required(dir.path()));
        set_recovery_required(dir.path()).unwrap();
        assert!(recovery_required(dir.path()));
        clear_recovery_required(dir.path()).unwrap();
        assert!(!recovery_required(dir.path()));
    }

    #[test]
    fn journal_record_is_metadata_only() {
        let rec = record("m-meta");
        let text = serde_json::to_string(&rec).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        let keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        for key in keys {
            assert!(
                [
                    "mutation_id",
                    "commit_id",
                    "commit_epoch",
                    "dataset_epoch",
                    "expected_revision",
                    "content_hash",
                    "doc_hash",
                    "stage",
                    "staged_file",
                    "target_file",
                    "created_at_secs"
                ]
                .contains(&key),
                "unexpected journal key {key}"
            );
        }
    }
}
