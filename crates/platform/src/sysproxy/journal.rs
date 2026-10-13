//! Crash-recovery ownership journal for the system proxy (H19).
//!
//! Mirrors the in-memory `applied` ledger (`AppliedChange` per field) to
//! `<data_dir>/sysproxy-ownership.journal` so a process that dies with the
//! proxy pointed at a dead local port can be reconciled on next startup.
//! Payload bytes are protected with a [`ProtectBackend`] (DPAPI on Windows);
//! the envelope below is the only plaintext.
//!
//! Envelope (all integers little-endian):
//! `magic "V2RJ" (4B) + u16 version(=1) + u8 alg_len + alg label + u32
//! blob_len + blob`. A file failing any structural check, the JSON parse or
//! the unprotect step is quarantined next to itself (latest 3 kept) and
//! treated as absent. Quarantine/log lines carry file names only, never
//! proxy values.
//!
//! This module performs file IO only; it never snapshots or writes the live
//! proxy. Reconciliation reuses `restore_if_owned` semantics in the caller.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{PlatformError, Result};
use crate::protect::ProtectBackend;
use crate::sysproxy::AppliedChange;

/// Journal file name inside the application data directory.
pub const JOURNAL_FILE_NAME: &str = "sysproxy-ownership.journal";
/// Envelope version written by [`save_journal`].
pub const JOURNAL_VERSION: u16 = 1;
/// How many quarantined corrupt journals are kept beside the live file.
pub const MAX_QUARANTINE_KEPT: usize = 3;

const ENVELOPE_MAGIC: &[u8; 4] = b"V2RJ";

/// Identity of the process that wrote the journal: a bare pid can be
/// recycled, so recovery re-verifies the creation time before trusting it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredOwner {
    pub pid: u32,
    pub created_at_ms: i64,
}

/// Durable mirror of the owned-field ledger plus its writer identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnershipJournal {
    pub owner: StoredOwner,
    pub applied: Vec<AppliedChange>,
    pub saved_at_ms: i64,
}

impl OwnershipJournal {
    pub fn new(owner: StoredOwner, applied: Vec<AppliedChange>) -> Self {
        Self {
            owner,
            applied,
            saved_at_ms: now_ms(),
        }
    }
}

/// Outcome of [`load_journal`].
#[derive(Debug)]
pub enum JournalLoad {
    /// No journal file exists.
    Absent,
    /// A well-formed journal.
    Loaded(OwnershipJournal),
    /// The file was corrupt/foreign and was moved aside to the returned path.
    /// Treat as absent; the path is for filename-only diagnostics.
    Quarantined(PathBuf),
}

/// Journal path for a data directory.
pub fn journal_path(data_dir: &Path) -> PathBuf {
    data_dir.join(JOURNAL_FILE_NAME)
}

/// Atomically replace the journal at `path`. Best-effort durability
/// (tmp + fsync + rename); callers log failures without failing the apply.
pub fn save_journal(
    path: &Path,
    journal: &OwnershipJournal,
    protect: &dyn ProtectBackend,
) -> Result<()> {
    let inner = serde_json::to_vec(journal)
        .map_err(|e| PlatformError::Backend(format!("ownership journal serialise failed: {e}")))?;
    let blob = protect.protect(&inner)?;
    let label = protect.label();
    if label.len() > u8::MAX as usize {
        return Err(PlatformError::Backend("protect label too long".to_string()));
    }
    let mut file = Vec::with_capacity(ENVELOPE_MAGIC.len() + 7 + label.len() + blob.len());
    file.extend_from_slice(ENVELOPE_MAGIC);
    file.extend_from_slice(&JOURNAL_VERSION.to_le_bytes());
    file.push(label.len() as u8);
    file.extend_from_slice(label.as_bytes());
    file.extend_from_slice(&(blob.len() as u32).to_le_bytes());
    file.extend_from_slice(&blob);
    atomic_write(path, &file)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .map_err(|e| PlatformError::Io(e))?
            .permissions();
        permissions.set_mode(0o600);
        std::fs::set_permissions(path, permissions).map_err(PlatformError::Io)?;
    }
    Ok(())
}

/// Load the journal at `path`. A corrupt file is quarantined and reported as
/// [`JournalLoad::Quarantined`]; only IO failures reading a present file are
/// hard errors.
pub fn load_journal(path: &Path, protect: &dyn ProtectBackend) -> Result<JournalLoad> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(JournalLoad::Absent),
        Err(e) => return Err(PlatformError::Io(e)),
    };
    let journal = match parse_envelope(&bytes, protect) {
        Ok(journal) => journal,
        Err(_) => {
            let quarantined = quarantine(path)?;
            return Ok(JournalLoad::Quarantined(quarantined));
        }
    };
    Ok(JournalLoad::Loaded(journal))
}

/// Remove the journal file. Missing file is success (idempotent).
pub fn clear_journal(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(PlatformError::Io(e)),
    }
}

/// Split `n` bytes off the front of `cursor`.
fn take<'a>(cursor: &mut &'a [u8], n: usize) -> Result<&'a [u8]> {
    if cursor.len() < n {
        return Err(PlatformError::Backend(
            "ownership journal is corrupt".to_string(),
        ));
    }
    let (head, tail) = cursor.split_at(n);
    *cursor = tail;
    Ok(head)
}

fn parse_envelope(bytes: &[u8], protect: &dyn ProtectBackend) -> Result<OwnershipJournal> {
    let invalid = || PlatformError::Backend("ownership journal is corrupt".to_string());
    let mut cursor = bytes;
    if take(&mut cursor, 4)? != ENVELOPE_MAGIC {
        return Err(invalid());
    }
    let version = u16::from_le_bytes(take(&mut cursor, 2)?.try_into().map_err(|_| invalid())?);
    if version != JOURNAL_VERSION {
        return Err(invalid());
    }
    let alg_len = take(&mut cursor, 1)?[0] as usize;
    let alg = std::str::from_utf8(take(&mut cursor, alg_len)?).map_err(|_| invalid())?;
    if alg != protect.label() {
        return Err(invalid());
    }
    let blob_len =
        u32::from_le_bytes(take(&mut cursor, 4)?.try_into().map_err(|_| invalid())?) as usize;
    let blob = take(&mut cursor, blob_len)?;
    if !cursor.is_empty() {
        return Err(invalid());
    }
    let inner = protect.unprotect(blob)?;
    serde_json::from_slice(&inner)
        .map_err(|_| PlatformError::Backend("ownership journal payload is corrupt".to_string()))
}

/// Move a corrupt journal aside, keeping the latest [`MAX_QUARANTINE_KEPT`].
fn quarantine(path: &Path) -> Result<PathBuf> {
    let parent = path.parent().ok_or_else(|| {
        PlatformError::Backend("ownership journal has no parent directory".to_string())
    })?;
    let stem = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| JOURNAL_FILE_NAME.to_string());
    let target = parent.join(format!("{stem}.corrupt-{}", now_nanos()));
    // Best effort: a rename failure still reports the original path so the
    // caller can log a filename instead of losing the fact entirely.
    if std::fs::rename(path, &target).is_err() {
        return Ok(path.to_path_buf());
    }
    prune_quarantine(parent, &stem);
    Ok(target)
}

/// Delete older quarantined journals beyond [`MAX_QUARANTINE_KEPT`].
/// Nanosecond suffixes sort chronologically, so names order oldest first.
fn prune_quarantine(dir: &Path, stem: &str) {
    let prefix = format!("{stem}.corrupt-");
    let mut matches: Vec<PathBuf> = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(&prefix) {
            matches.push(entry.path());
        }
    }
    matches.sort();
    while matches.len() > MAX_QUARANTINE_KEPT {
        let oldest = matches.remove(0);
        let _ = std::fs::remove_file(oldest);
    }
}

/// Tmp + fsync + rename + parent sync. Mirrors the persistence commit
/// pattern; kept local so `platform` stays free of new dependencies.
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(PlatformError::Io)?;
    }
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes).map_err(PlatformError::Io)?;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&tmp)
        .map_err(PlatformError::Io)?;
    let _ = file.sync_all();
    std::fs::rename(&tmp, path).map_err(PlatformError::Io)?;
    if let Some(parent) = path.parent() {
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    Ok(())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn now_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protect::{FakeProtectBackend, PlainBackend};

    fn owner() -> StoredOwner {
        StoredOwner {
            pid: 4242,
            created_at_ms: 1_700_000_000_000,
        }
    }

    fn entry() -> AppliedChange {
        AppliedChange {
            field: crate::sysproxy::ProxyField::Server,
            before: Some("old:1".to_string()),
            after: Some("127.0.0.1:11809".to_string()),
        }
    }

    fn unique_dir(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("v2rayn-r-journal-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn envelope_rejects_garbage_prefix() {
        let protect = PlainBackend;
        assert!(parse_envelope(b"nope", &protect).is_err());
        assert!(parse_envelope(b"V2RJ\x01", &protect).is_err());
    }

    #[test]
    fn envelope_rejects_foreign_algorithm_label() {
        let dir = unique_dir("foreign-alg");
        let path = dir.join(JOURNAL_FILE_NAME);
        let journal = OwnershipJournal::new(owner(), vec![entry()]);
        save_journal(&path, &journal, &FakeProtectBackend::new(3)).expect("save");
        // A backend with another key/label must not decode it: quarantine.
        let loaded = load_journal(&path, &FakeProtectBackend::new(4)).expect("load");
        assert!(
            matches!(loaded, JournalLoad::Quarantined(_)),
            "foreign label must quarantine, got {loaded:?}"
        );
        assert!(!path.exists(), "corrupt file must move aside");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
