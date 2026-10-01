//! Persistent recovery journal (plan §13).
//!
//! The journal is a hint, never system truth: after a crash the actual process
//! set is re-read and each recorded PID is only acted on when its creation
//! time still matches. It records plan *summaries* (ids, revision, config
//! hash, port, PID) and never the config body or any credential.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ipc_contract::RecoveryStage;
use serde::{Deserialize, Serialize};

use runtime::{matches_identity, terminate_identity, ProcessIdentity};

/// One journaled session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub session_id: String,
    pub plan_id: String,
    pub desired_revision: u64,
    pub config_sha256: String,
    pub stage: RecoveryStage,
    pub pid: Option<u32>,
    pub created_at_ms: Option<i64>,
    pub port: u16,
    pub updated_at_ms: i64,
}

impl JournalEntry {
    pub fn identity(&self) -> Option<ProcessIdentity> {
        match (self.pid, self.created_at_ms) {
            (Some(pid), Some(created_at_ms)) => Some(ProcessIdentity::new(pid, created_at_ms)),
            _ => None,
        }
    }
}

/// Summary returned to the snapshot as `StartupRecovery`.
#[derive(Debug, Clone, Default)]
pub struct RecoveryReport {
    pub needed: bool,
    pub last_stage: Option<RecoveryStage>,
    pub restored: u32,
    pub pending: u32,
}

/// Directory holding one session's staged config, log and journal.
pub fn session_dir(run_root: &Path, session_id: &str) -> PathBuf {
    run_root.join(session_id)
}

fn journal_path(run_root: &Path, session_id: &str) -> PathBuf {
    session_dir(run_root, session_id).join("journal.json")
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Atomically write a journal entry (temp file + rename).
pub fn write_entry(run_root: &Path, entry: &JournalEntry) -> std::io::Result<()> {
    let dir = session_dir(run_root, &entry.session_id);
    std::fs::create_dir_all(&dir)?;
    let path = journal_path(run_root, &entry.session_id);
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_vec_pretty(entry)?;
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Read all journals under a run root.
pub fn read_all(run_root: &Path) -> Vec<JournalEntry> {
    let mut entries = Vec::new();
    let Ok(dirs) = std::fs::read_dir(run_root) else {
        return entries;
    };
    for dir in dirs.flatten() {
        let path = dir.path().join("journal.json");
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(entry) = serde_json::from_slice::<JournalEntry>(&bytes) {
                entries.push(entry);
            }
        }
    }
    entries
}

/// Reconcile stale sessions on startup.
///
/// A session is stale when its journal never reached `Finalized`. Ownership is
/// re-verified by `(pid, creation_time)` before any termination; PID reuse can
/// therefore never cause an unrelated process to be killed.
pub fn recover_stale(run_root: &Path) -> RecoveryReport {
    let mut report = RecoveryReport::default();
    for mut entry in read_all(run_root) {
        if entry.stage == RecoveryStage::Finalized {
            continue;
        }
        report.needed = true;
        report.last_stage = Some(entry.stage);
        let cleaned = match entry.identity() {
            // Still our process: terminate it by verified identity. If the
            // terminate loses a race with a process that is already exiting,
            // re-check: a gone/recycled PID is treated as cleaned so a stale
            // journal is always finalized rather than left pending forever.
            Some(identity) if matches_identity(&identity) => {
                let killed = terminate_identity(&identity);
                let gone = !matches_identity(&identity);
                killed || gone
            }
            // No identity recorded, or the PID is gone/recycled: nothing of
            // ours can still be running.
            _ => true,
        };
        eprintln!(
            "[net_host] recovery session={} stage={:?} pid={:?} cleaned={}",
            entry.session_id, entry.stage, entry.pid, cleaned
        );
        if cleaned {
            report.restored += 1;
            entry.stage = RecoveryStage::Finalized;
            entry.updated_at_ms = now_ms();
            let _ = write_entry(run_root, &entry);
        } else {
            report.pending += 1;
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, stage: RecoveryStage) -> JournalEntry {
        JournalEntry {
            session_id: id.into(),
            plan_id: "p".into(),
            desired_revision: 1,
            config_sha256: "ab".into(),
            stage,
            pid: None,
            created_at_ms: None,
            port: 11808,
            updated_at_ms: 0,
        }
    }

    #[test]
    fn finalized_sessions_are_not_recovered() {
        let root = std::env::temp_dir().join(format!("v2rayn-t03-journal-{}", now_ms()));
        std::fs::create_dir_all(&root).unwrap();
        write_entry(&root, &entry("done", RecoveryStage::Finalized)).unwrap();
        let report = recover_stale(&root);
        assert!(!report.needed);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn stale_session_without_pid_is_finalized() {
        let root = std::env::temp_dir().join(format!("v2rayn-t03-journal-{}", now_ms() + 1));
        std::fs::create_dir_all(&root).unwrap();
        write_entry(&root, &entry("stale", RecoveryStage::Applying)).unwrap();
        let report = recover_stale(&root);
        assert!(report.needed);
        assert_eq!(report.restored, 1);
        assert_eq!(read_all(&root)[0].stage, RecoveryStage::Finalized);
        let _ = std::fs::remove_dir_all(&root);
    }
}
