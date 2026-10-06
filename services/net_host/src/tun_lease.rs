//! TUN lease journal and orchestration (T14 runtime integration).
//!
//! The core-process journal (`journal.rs`) records *process* ownership; this
//! module records the *TUN* side: which helper session owns the routes and the
//! adapter address for one net-host session. Only resources named here are
//! ever cleaned, and cleanup is idempotent, so a crash followed by a restart
//! can safely re-run it.
//!
//! Everything runs through [`HelperLink`], so tests use the in-memory
//! [`FakeHelperLink`](crate::helper_client::FakeHelperLink) and never touch a
//! real pipe, helper process, route table or adapter.

use std::path::{Path, PathBuf};

use domain::{codes, DomainError};
use runtime::tun::TunSpec;
use serde::{Deserialize, Serialize};

#[cfg(test)]
use crate::helper_client::E_TUN_HELPER_UNAVAILABLE;
use crate::helper_client::{HelperLink, TunLease};
use crate::journal::{now_ms, session_dir};

/// File name of the TUN lease journal inside a session directory.
pub const TUN_JOURNAL_FILE: &str = "tun_lease.json";

/// File name of the durable pending-cleanup record (SP-08). Written when a
/// cleanup attempt fails without confirmation and removed once a later retry
/// (or boot reconciliation) confirms the release. It carries the last error
/// and the attempt count so the failure stays visible and retryable across
/// restarts instead of being forgotten.
pub const TUN_PENDING_FILE: &str = "tun_pending.json";

/// Journaled TUN lease: the net-host-side record of what the helper owns for
/// one session. The embedded [`TunLease`] carries the full descriptor, so
/// recovery can re-derive the exact cleanup without the original plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TunJournalEntry {
    pub session_id: String,
    pub lease: TunLease,
    pub updated_at_ms: i64,
}

/// Outcome of one restart reconciliation pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TunRecoveryReport {
    pub scanned: u32,
    pub cleaned: u32,
    pub pending: u32,
}

/// Durable record of an unconfirmed TUN cleanup (SP-08 / CP-04). The lease
/// journal stays the ownership source of truth; this record adds the last
/// error and the attempt count so a failed cleanup is visible, survives a
/// restart, and can be retried. It never carries next-hop addresses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingCleanup {
    pub session_id: String,
    pub adapter_name: String,
    pub interface_index: u32,
    pub route_digest: String,
    pub dry_run: bool,
    pub error_code: String,
    pub message_key: String,
    pub attempts: u32,
    pub updated_at_ms: i64,
}

impl PendingCleanup {
    pub fn from_failure(
        session_id: &str,
        lease: &TunLease,
        error: &DomainError,
        attempts: u32,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            adapter_name: lease.adapter_name.clone(),
            interface_index: lease.interface_index,
            route_digest: lease.route_digest.clone(),
            dry_run: lease.dry_run,
            error_code: error.code.clone(),
            message_key: error.message_key.clone(),
            attempts,
            updated_at_ms: now_ms(),
        }
    }

    /// Redacted one-line summary for logs and retry surfaces.
    pub fn summary(&self) -> String {
        format!(
            "session={} adapter={} if={} digest={} attempts={} last_error={}",
            self.session_id,
            self.adapter_name,
            self.interface_index,
            &self.route_digest[..self.route_digest.len().min(12)],
            self.attempts,
            self.error_code,
        )
    }
}

/// Whether a cleanup error means "already gone" and may converge like
/// success. Today that is only `E_NOT_FOUND` (the helper's `UnknownHandle`
/// maps there): removing routes or resetting an adapter that no longer exists
/// is idempotent. Every other error — backend failure, unreachable helper,
/// permission refusal — keeps the journal for retry; a permission error is
/// never swallowed into success (CP-04).
///
/// A02 follow-up: the helper will report an explicit per-resource
/// `AlreadyGone` in `ReleaseOwnedResources`; until then only `E_NOT_FOUND`
/// converges here and nothing else is treated as absent.
pub fn is_already_absent(error: &DomainError) -> bool {
    error.code == codes::NOT_FOUND
}

pub fn tun_journal_path(run_root: &Path, session_id: &str) -> PathBuf {
    session_dir(run_root, session_id).join(TUN_JOURNAL_FILE)
}

pub fn pending_cleanup_path(run_root: &Path, session_id: &str) -> PathBuf {
    session_dir(run_root, session_id).join(TUN_PENDING_FILE)
}

/// Atomically write the pending-cleanup record (temp file + rename).
pub fn write_pending_cleanup(run_root: &Path, pending: &PendingCleanup) -> std::io::Result<()> {
    let dir = session_dir(run_root, &pending.session_id);
    std::fs::create_dir_all(&dir)?;
    let path = pending_cleanup_path(run_root, &pending.session_id);
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(pending)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Read the pending-cleanup record, if present and parseable.
pub fn read_pending_cleanup(run_root: &Path, session_id: &str) -> Option<PendingCleanup> {
    let bytes = std::fs::read(pending_cleanup_path(run_root, session_id)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Drop the pending-cleanup record. Called only after the release is confirmed
/// (or proven already-absent); a failed retry re-writes it with new attempts.
pub fn remove_pending_cleanup(run_root: &Path, session_id: &str) {
    let _ = std::fs::remove_file(pending_cleanup_path(run_root, session_id));
}

/// Every durable pending cleanup under the run root, sorted by session id.
/// This is the host-side retry surface the UI/API layer will expose once the
/// SP-00 integrator wires it through IPC/FRB (interface need N-H1).
pub fn list_pending_cleanup(run_root: &Path) -> Vec<PendingCleanup> {
    let mut out = Vec::new();
    let Ok(dirs) = std::fs::read_dir(run_root) else {
        return out;
    };
    for dir in dirs.flatten() {
        let session_id = dir.file_name().to_string_lossy().into_owned();
        if let Some(pending) = read_pending_cleanup(run_root, &session_id) {
            out.push(pending);
        }
    }
    out.sort_by(|a, b| a.session_id.cmp(&b.session_id));
    out
}

/// Record one failed attempt durably: bump the attempt count off any previous
/// pending record and persist the latest error. The lease journal is always
/// kept alongside, so the retry can re-derive the exact cleanup.
pub(crate) fn note_cleanup_failure(
    run_root: &Path,
    session_id: &str,
    lease: &TunLease,
    error: &DomainError,
) {
    let attempts = read_pending_cleanup(run_root, session_id)
        .map(|pending| pending.attempts)
        .unwrap_or(0)
        .saturating_add(1);
    let pending = PendingCleanup::from_failure(session_id, lease, error, attempts);
    eprintln!(
        "[net_host] tun pending cleanup retained: {}",
        pending.summary()
    );
    if let Err(io) = write_pending_cleanup(run_root, &pending) {
        eprintln!("[net_host] tun pending record write failed for {session_id}: {io}");
    }
}

/// Atomically write the lease journal (temp file + rename).
pub fn write_tun_journal(
    run_root: &Path,
    session_id: &str,
    lease: &TunLease,
) -> std::io::Result<()> {
    let dir = session_dir(run_root, session_id);
    std::fs::create_dir_all(&dir)?;
    let entry = TunJournalEntry {
        session_id: session_id.to_string(),
        lease: lease.clone(),
        updated_at_ms: now_ms(),
    };
    let path = tun_journal_path(run_root, session_id);
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&entry)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Read the lease journal, if present and parseable.
pub fn read_tun_journal(run_root: &Path, session_id: &str) -> Option<TunJournalEntry> {
    let bytes = std::fs::read(tun_journal_path(run_root, session_id)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Drop the lease journal. Called only after the helper resources are
/// released (or proven unreachable-but-helper-owned, see [`cleanup_tun_lease`]).
pub fn remove_tun_journal(run_root: &Path, session_id: &str) {
    let _ = std::fs::remove_file(tun_journal_path(run_root, session_id));
}

/// Whether the journaled record still describes `lease`.
///
/// Compares the helper session, adapter facts and the order-insensitive route
/// digest; anything else means the record is stale and must not be trusted.
pub fn verify_lease(journal: &TunJournalEntry, lease: &TunLease) -> bool {
    journal.lease.helper_session_id == lease.helper_session_id
        && journal.lease.adapter_name == lease.adapter_name
        && journal.lease.interface_index == lease.interface_index
        && journal.lease.route_count == lease.route_count
        && journal.lease.route_digest == lease.route_digest
        && journal.lease.dry_run == lease.dry_run
}

/// Ownership key of one applied TUN lease (SP-10, CP-12 归属).
///
/// Host-side mirror of `application::tun_ownership_key` (same
/// `adapter|index|digest` format and normalization: adapter display
/// casing/spacing is not identity, the index and the route digest are).
/// The literal format is pinned by SP-10 tests because net-host must not
/// depend on the application crate; unifying the two is SP-00 integrator
/// work. A reopened manager recovers by this journaled ownership, never by
/// the current desired settings. Staged for the SP-00 wiring.
#[allow(dead_code)]
pub fn lease_ownership_key(lease: &TunLease) -> String {
    format!(
        "{}|{}|{}",
        lease.adapter_name.trim().to_ascii_lowercase(),
        lease.interface_index,
        lease.route_digest.trim(),
    )
}

/// Recompute the digest from the journaled descriptor and compare it with the
/// stored one. Catches a truncated or hand-edited journal before cleanup.
pub fn verify_journal_integrity(journal: &TunJournalEntry) -> bool {
    let entries = journal.lease.spec.to_route_entries().unwrap_or_default();
    journal.lease.route_count as usize == entries.len()
        && journal.lease.route_digest == runtime::tun::route_digest(&entries)
}

/// Apply a TUN descriptor through the helper and journal the resulting lease.
///
/// Order: validate -> availability probe -> helper apply -> journal. A helper
/// failure is returned unchanged (notably `E_TUN_HELPER_UNAVAILABLE`);
/// net-host never falls back to a direct TUN path. When journaling itself
/// fails, whatever was just applied is released best-effort so no
/// helper-owned resource outlives its record.
pub fn apply_tun_lease(
    run_root: &Path,
    session_id: &str,
    spec: &TunSpec,
    link: &mut dyn HelperLink,
) -> Result<TunLease, DomainError> {
    spec.validate()?;
    link.available()?;
    let lease = match link.apply(spec) {
        Ok(lease) => lease,
        Err(error) => {
            remove_tun_journal(run_root, session_id);
            return Err(error);
        }
    };
    if let Err(e) = write_tun_journal(run_root, session_id, &lease) {
        let _ = link.cleanup(&lease);
        remove_tun_journal(run_root, session_id);
        return Err(
            DomainError::new(codes::INTERNAL, "error.tun_journal_failed")
                .with_detail(format!("write tun lease journal failed: {e}")),
        );
    }
    Ok(lease)
}

/// Release a TUN lease idempotently (SP-08 / CP-04).
///
/// - No journal: already cleaned — `Ok` after dropping any stale pending
///   marker, without touching the helper.
/// - Helper confirms the release, or reports already-absent (`E_NOT_FOUND`,
///   idempotent): the journal and any pending record are dropped, `Ok`.
/// - Any other helper failure: the journal is **kept**, a durable pending
///   record is written/refreshed, and the error is returned. The caller must
///   not report success and must keep the lease retryable. The helper also
///   releases owned resources on disconnect, so a retry converges once the
///   helper confirms or the resources are proven gone.
///
/// On a journal/lease mismatch the *recorded* (owned) resources are cleaned —
/// never the caller's differing descriptor — so a retry always acts on the
/// true ownership and a stale caller cannot redirect the cleanup elsewhere.
pub fn cleanup_tun_lease(
    run_root: &Path,
    session_id: &str,
    lease: &TunLease,
    link: &mut dyn HelperLink,
) -> Result<(), DomainError> {
    let owned = match read_tun_journal(run_root, session_id) {
        None => {
            remove_pending_cleanup(run_root, session_id);
            return Ok(());
        }
        Some(journal) => {
            if !verify_lease(&journal, lease) {
                eprintln!(
                    "[net_host] tun journal mismatch for {session_id}: recorded lease differs; cleaning recorded resources"
                );
            }
            if !verify_journal_integrity(&journal) {
                eprintln!(
                    "[net_host] tun journal for {session_id} failed integrity check; cleaning recorded resources anyway"
                );
            }
            journal.lease
        }
    };
    match link.cleanup(&owned) {
        Ok(()) => {
            remove_tun_journal(run_root, session_id);
            remove_pending_cleanup(run_root, session_id);
            Ok(())
        }
        Err(error) if is_already_absent(&error) => {
            eprintln!("[net_host] tun cleanup for {session_id} already absent; converging journal");
            remove_tun_journal(run_root, session_id);
            remove_pending_cleanup(run_root, session_id);
            Ok(())
        }
        Err(error) => {
            eprintln!(
                "[net_host] tun cleanup failed for {session_id}, journal retained for retry: {} {}",
                error.code, error.message_key
            );
            note_cleanup_failure(run_root, session_id, &owned, &error);
            Err(error)
        }
    }
}

/// Retry a previously failed cleanup (SP-08). Reads the retained journal —
/// the owned record, not the caller's memory — and re-issues the cleanup.
/// `Ok` means the helper confirmed the release (journal and pending record
/// dropped); `Err` refreshes the pending record with new attempts. Unknown
/// session (no journal) is `Ok` after clearing any stale pending marker.
///
/// Called from the async host retry entry; staged until the SP-00 integrator
/// exposes it over IPC/FRB (need N-H1).
#[allow(dead_code)]
pub fn retry_tun_cleanup(
    run_root: &Path,
    session_id: &str,
    link: &mut dyn HelperLink,
) -> Result<(), DomainError> {
    let Some(journal) = read_tun_journal(run_root, session_id) else {
        remove_pending_cleanup(run_root, session_id);
        return Ok(());
    };
    if !verify_journal_integrity(&journal) {
        eprintln!(
            "[net_host] tun journal for {session_id} failed integrity check; retrying recorded cleanup anyway"
        );
    }
    cleanup_tun_lease(run_root, session_id, &journal.lease, link)
}

/// Reconcile stale TUN leases after a restart.
///
/// net-host is the single owner of its run root, so any lease journal found at
/// boot is stale: the previous process is gone. Each one gets a single cleanup
/// attempt through `link`:
/// - confirmed release or already-absent: journal (and pending record)
///   dropped, counted as cleaned;
/// - any other failure (helper down, backend error, permission refusal): the
///   journal is **kept**, the pending record is refreshed, and the lease is
///   counted as pending so a later boot or an explicit retry — with a live
///   helper — can finish the cleanup. Nothing is silently dropped (CP-04).
pub fn reconcile_stale_tun(run_root: &Path, link: &mut dyn HelperLink) -> TunRecoveryReport {
    let mut report = TunRecoveryReport::default();
    let Ok(dirs) = std::fs::read_dir(run_root) else {
        return report;
    };
    let mut session_ids: Vec<String> = Vec::new();
    for dir in dirs.flatten() {
        let path = dir.path().join(TUN_JOURNAL_FILE);
        if path.is_file() {
            session_ids.push(dir.file_name().to_string_lossy().into_owned());
        }
    }
    session_ids.sort();
    for session_id in session_ids {
        let Some(journal) = read_tun_journal(run_root, &session_id) else {
            continue;
        };
        report.scanned += 1;
        if !verify_journal_integrity(&journal) {
            eprintln!(
                "[net_host] tun journal for {session_id} failed integrity check; cleaning recorded resources anyway"
            );
        }
        match link.cleanup(&journal.lease) {
            Ok(()) => {
                remove_tun_journal(run_root, &session_id);
                remove_pending_cleanup(run_root, &session_id);
                report.cleaned += 1;
            }
            Err(error) if is_already_absent(&error) => {
                eprintln!(
                    "[net_host] tun recovery for {session_id}: already absent; converging journal"
                );
                remove_tun_journal(run_root, &session_id);
                remove_pending_cleanup(run_root, &session_id);
                report.cleaned += 1;
            }
            Err(error) => {
                eprintln!(
                    "[net_host] tun recovery deferred for {session_id}: {} {}; journal kept for retry",
                    error.code, error.message_key
                );
                note_cleanup_failure(run_root, &session_id, &journal.lease, &error);
                report.pending += 1;
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helper_client::{
        DryRunHelperLink, FakeHelperFault, FakeHelperLink, UnavailableHelperLink,
    };
    use runtime::tun::{TunAddress, TunRoute, TUN_CONFIG_KIND};

    fn spec() -> TunSpec {
        TunSpec {
            kind: TUN_CONFIG_KIND.into(),
            adapter_name: "v2rayn-tun".into(),
            interface_index: 9,
            addresses: vec![TunAddress {
                address: "198.18.0.1".into(),
                prefix_len: 16,
            }],
            mtu: Some(1400),
            routes: vec![TunRoute {
                destination: "0.0.0.0/0".into(),
                next_hop: "198.18.0.1".into(),
                interface_index: 9,
                metric: 1,
            }],
            route_exclude: vec![],
        }
    }

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "v2rayn-t14-tun-{}-{}-{}",
            name,
            std::process::id(),
            now_ms()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn apply_writes_journal_and_returns_lease() {
        let root = temp_root("apply");
        let mut link = FakeHelperLink::new();
        let lease = apply_tun_lease(&root, "s1", &spec(), &mut link).unwrap();
        assert_eq!(lease.route_count, 1);
        assert!(!lease.dry_run);
        let journal = read_tun_journal(&root, "s1").expect("journal must exist");
        assert_eq!(journal.session_id, "s1");
        assert!(verify_lease(&journal, &lease));
        assert!(verify_journal_integrity(&journal));
        assert!(tun_journal_path(&root, "s1").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_denied_leaves_no_journal_with_stable_code() {
        let root = temp_root("deny");
        let mut link = FakeHelperLink::with_fault(FakeHelperFault::Deny);
        let error = apply_tun_lease(&root, "s1", &spec(), &mut link).unwrap_err();
        assert_eq!(error.code, E_TUN_HELPER_UNAVAILABLE);
        assert_eq!(error.message_key, "error.tun_helper_denied");
        assert!(read_tun_journal(&root, "s1").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_timeout_is_retryable_unavailable() {
        let root = temp_root("timeout");
        let mut link = FakeHelperLink::with_fault(FakeHelperFault::Timeout);
        let error = apply_tun_lease(&root, "s1", &spec(), &mut link).unwrap_err();
        assert_eq!(error.code, E_TUN_HELPER_UNAVAILABLE);
        assert!(error.retryable);
        assert!(read_tun_journal(&root, "s1").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_disconnect_is_unavailable() {
        let root = temp_root("disconnect");
        let mut link = FakeHelperLink::with_fault(FakeHelperFault::Disconnect);
        let error = apply_tun_lease(&root, "s1", &spec(), &mut link).unwrap_err();
        assert_eq!(error.code, E_TUN_HELPER_UNAVAILABLE);
        assert!(read_tun_journal(&root, "s1").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_rejects_invalid_spec_before_touching_helper() {
        let root = temp_root("invalid");
        let mut link = FakeHelperLink::new();
        let mut bad = spec();
        bad.kind = "other".into();
        let error = apply_tun_lease(&root, "s1", &bad, &mut link).unwrap_err();
        assert_eq!(error.code, codes::INVALID_PLAN);
        assert!(
            link.audit().is_empty(),
            "invalid spec must not reach the helper"
        );
        assert!(read_tun_journal(&root, "s1").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cleanup_releases_helper_state_and_drops_journal() {
        let root = temp_root("cleanup");
        let mut link = FakeHelperLink::new();
        let lease = apply_tun_lease(&root, "s1", &spec(), &mut link).unwrap();
        cleanup_tun_lease(&root, "s1", &lease, &mut link).unwrap();
        assert!(!link.has_tun());
        assert_eq!(link.route_count(), 0);
        assert!(read_tun_journal(&root, "s1").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cleanup_is_idempotent() {
        let root = temp_root("idempotent");
        let mut link = FakeHelperLink::new();
        let lease = apply_tun_lease(&root, "s1", &spec(), &mut link).unwrap();
        cleanup_tun_lease(&root, "s1", &lease, &mut link).unwrap();
        // Second cleanup: journal gone, helper already clean; still Ok.
        cleanup_tun_lease(&root, "s1", &lease, &mut link).unwrap();
        assert_eq!(link.cleanup_count(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cleanup_without_journal_is_ok() {
        let root = temp_root("nojournal");
        let mut link = FakeHelperLink::new();
        let lease = TunLease::new("ghost", spec(), false);
        cleanup_tun_lease(&root, "ghost", &lease, &mut link).unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cleanup_with_mismatched_lease_still_cleans() {
        let root = temp_root("mismatch");
        let mut link = FakeHelperLink::new();
        let _recorded = apply_tun_lease(&root, "s1", &spec(), &mut link).unwrap();
        let mut other = spec();
        other.interface_index = 11;
        other.routes[0].interface_index = 11;
        let foreign = TunLease::new("other-session", other, false);
        assert!(
            !verify_lease(&read_tun_journal(&root, "s1").unwrap(), &foreign),
            "leases must differ for this test"
        );
        cleanup_tun_lease(&root, "s1", &foreign, &mut link).unwrap();
        assert!(read_tun_journal(&root, "s1").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reconcile_cleans_stale_after_restart() {
        let root = temp_root("restart");
        let mut first = FakeHelperLink::new();
        apply_tun_lease(&root, "s1", &spec(), &mut first).unwrap();
        drop(first);
        // Fresh process, fresh link: the journaled lease is still cleaned.
        let mut second = FakeHelperLink::new();
        let report = reconcile_stale_tun(&root, &mut second);
        assert_eq!(
            report,
            TunRecoveryReport {
                scanned: 1,
                cleaned: 1,
                pending: 0
            }
        );
        assert!(read_tun_journal(&root, "s1").is_none());
        // Re-running is a no-op.
        let report = reconcile_stale_tun(&root, &mut second);
        assert_eq!(report, TunRecoveryReport::default());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reconcile_unavailable_keeps_pending_and_journal() {
        let root = temp_root("pending");
        let mut link = FakeHelperLink::new();
        apply_tun_lease(&root, "s1", &spec(), &mut link).unwrap();
        drop(link);
        let mut down = UnavailableHelperLink::new();
        let report = reconcile_stale_tun(&root, &mut down);
        assert_eq!(report.scanned, 1);
        assert_eq!(report.pending, 1);
        assert_eq!(report.cleaned, 0);
        assert!(
            read_tun_journal(&root, "s1").is_some(),
            "journal must survive for a later boot with a live helper"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reconcile_empty_run_root_is_zero() {
        let root = temp_root("empty");
        let mut link = FakeHelperLink::new();
        assert_eq!(
            reconcile_stale_tun(&root, &mut link),
            TunRecoveryReport::default()
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn dry_run_lease_journals_dry_run_flag() {
        let root = temp_root("dryrun");
        let mut link = DryRunHelperLink::new();
        let lease = apply_tun_lease(&root, "s1", &spec(), &mut link).unwrap();
        assert!(lease.dry_run);
        let journal = read_tun_journal(&root, "s1").unwrap();
        assert!(journal.lease.dry_run);
        cleanup_tun_lease(&root, "s1", &lease, &mut link).unwrap();
        assert!(read_tun_journal(&root, "s1").is_none());
        assert!(link.audit().iter().any(|op| op.starts_with("dry_run:")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn journal_entry_roundtrips_through_json() {
        let lease = TunLease::new("sess", spec(), false);
        let entry = TunJournalEntry {
            session_id: "s9".into(),
            lease,
            updated_at_ms: 1_700_000_000_000,
        };
        let back: TunJournalEntry =
            serde_json::from_slice(&serde_json::to_vec(&entry).unwrap()).unwrap();
        assert_eq!(entry, back);
    }

    // -- SP-08 red contracts (CP-04): cleanup failure must not report success.
    //
    // These use only the current public `HelperLink` surface, so they compile
    // before the fix and fail against the swallowing behavior. The fix keeps
    // them green without touching their expectations.

    /// A link whose apply succeeds but whose cleanup always fails with a
    /// backend error (synthetic fault injection, no OS/helper/pipe).
    struct FailingCleanupLink;

    impl crate::helper_client::HelperLink for FailingCleanupLink {
        fn session_id(&self) -> String {
            "helper-session-9".into()
        }

        fn dry_run(&self) -> bool {
            false
        }

        fn available(&mut self) -> Result<(), DomainError> {
            Ok(())
        }

        fn apply(&mut self, spec: &TunSpec) -> Result<TunLease, DomainError> {
            Ok(TunLease::new("helper-session-9", spec.clone(), false))
        }

        fn cleanup(&mut self, _lease: &TunLease) -> Result<(), DomainError> {
            Err(crate::helper_client::tun_apply_failed(
                "synthetic backend cleanup failure",
            ))
        }
    }

    /// A link whose cleanup reports the helper unreachable (retryable).
    struct UnreachableCleanupLink;

    impl crate::helper_client::HelperLink for UnreachableCleanupLink {
        fn session_id(&self) -> String {
            String::new()
        }

        fn dry_run(&self) -> bool {
            false
        }

        fn available(&mut self) -> Result<(), DomainError> {
            Ok(())
        }

        fn apply(&mut self, spec: &TunSpec) -> Result<TunLease, DomainError> {
            Ok(TunLease::new(String::new(), spec.clone(), false))
        }

        fn cleanup(&mut self, _lease: &TunLease) -> Result<(), DomainError> {
            Err(crate::helper_client::tun_helper_unavailable(
                "synthetic helper unreachable during cleanup",
            ))
        }
    }

    #[test]
    fn sp08_cleanup_failure_is_not_success_and_keeps_journal() {
        let root = temp_root("sp08-red-cleanup");
        let mut apply_link = FakeHelperLink::new();
        let lease = apply_tun_lease(&root, "s1", &spec(), &mut apply_link).unwrap();
        let mut failing = FailingCleanupLink;
        let error = cleanup_tun_lease(&root, "s1", &lease, &mut failing)
            .expect_err("an unconfirmed cleanup cannot report success");
        assert_eq!(error.code, codes::UNAVAILABLE);
        assert!(
            read_tun_journal(&root, "s1").is_some(),
            "the journal must survive a failed cleanup so a later retry can recover"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sp08_reconcile_backend_failure_keeps_pending_journal() {
        let root = temp_root("sp08-red-reconcile");
        let mut apply_link = FakeHelperLink::new();
        apply_tun_lease(&root, "s1", &spec(), &mut apply_link).unwrap();
        drop(apply_link);
        let mut failing = FailingCleanupLink;
        let report = reconcile_stale_tun(&root, &mut failing);
        assert_eq!(report.scanned, 1);
        assert_eq!(
            report.pending, 1,
            "an unconfirmed recovery must stay pending, never counted as cleaned"
        );
        assert_eq!(report.cleaned, 0);
        assert!(
            read_tun_journal(&root, "s1").is_some(),
            "the journal must survive for a later boot with a live helper"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sp08_reconcile_unreachable_keeps_pending_journal() {
        let root = temp_root("sp08-red-unreachable");
        let mut apply_link = FakeHelperLink::new();
        apply_tun_lease(&root, "s1", &spec(), &mut apply_link).unwrap();
        drop(apply_link);
        let mut down = UnreachableCleanupLink;
        let report = reconcile_stale_tun(&root, &mut down);
        assert_eq!((report.scanned, report.cleaned, report.pending), (1, 0, 1));
        assert!(read_tun_journal(&root, "s1").is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A link whose cleanup reports the resources already gone (`E_NOT_FOUND`,
    /// the current mapping of the helper's `UnknownHandle`).
    struct AlreadyAbsentLink;

    impl crate::helper_client::HelperLink for AlreadyAbsentLink {
        fn session_id(&self) -> String {
            "helper-session-9".into()
        }

        fn dry_run(&self) -> bool {
            false
        }

        fn available(&mut self) -> Result<(), DomainError> {
            Ok(())
        }

        fn apply(&mut self, spec: &TunSpec) -> Result<TunLease, DomainError> {
            Ok(TunLease::new("helper-session-9", spec.clone(), false))
        }

        fn cleanup(&mut self, _lease: &TunLease) -> Result<(), DomainError> {
            Err(DomainError::not_found("tun_lease", "s1"))
        }
    }

    #[test]
    fn sp08_already_absent_converges_like_success() {
        let root = temp_root("sp08-absent");
        let mut apply_link = FakeHelperLink::new();
        let lease = apply_tun_lease(&root, "s1", &spec(), &mut apply_link).unwrap();
        let mut gone = AlreadyAbsentLink;
        cleanup_tun_lease(&root, "s1", &lease, &mut gone)
            .expect("already-absent is idempotent success");
        assert!(read_tun_journal(&root, "s1").is_none());
        assert!(read_pending_cleanup(&root, "s1").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sp08_only_not_found_counts_as_already_absent() {
        assert!(is_already_absent(&DomainError::not_found(
            "tun_lease",
            "s1"
        )));
        assert!(!is_already_absent(&crate::helper_client::tun_apply_failed(
            "x"
        )));
        assert!(!is_already_absent(
            &crate::helper_client::tun_helper_unavailable("x")
        ));
        assert!(!is_already_absent(
            &crate::helper_client::tun_helper_denied("x")
        ));
    }

    #[test]
    fn sp08_failed_cleanup_leaves_a_listed_pending_record() {
        let root = temp_root("sp08-pending");
        let mut apply_link = FakeHelperLink::new();
        let lease = apply_tun_lease(&root, "s1", &spec(), &mut apply_link).unwrap();
        let mut failing = FailingCleanupLink;
        let error = cleanup_tun_lease(&root, "s1", &lease, &mut failing).unwrap_err();
        assert_eq!(error.code, codes::UNAVAILABLE);
        let pendings = list_pending_cleanup(&root);
        assert_eq!(pendings.len(), 1);
        let pending = &pendings[0];
        assert_eq!(pending.session_id, "s1");
        assert_eq!(pending.error_code, codes::UNAVAILABLE);
        assert_eq!(pending.attempts, 1);
        assert_eq!(pending.adapter_name, "v2rayn-tun");
        let summary = pending.summary();
        assert!(summary.contains("attempts=1"));
        assert!(
            !summary.contains("198.18"),
            "pending summaries must stay redacted"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sp08_repeated_failures_bump_attempts() {
        let root = temp_root("sp08-attempts");
        let mut apply_link = FakeHelperLink::new();
        let lease = apply_tun_lease(&root, "s1", &spec(), &mut apply_link).unwrap();
        let mut failing = FailingCleanupLink;
        for _ in 0..2 {
            let _ = cleanup_tun_lease(&root, "s1", &lease, &mut failing).unwrap_err();
        }
        assert_eq!(
            read_pending_cleanup(&root, "s1").expect("pending").attempts,
            2
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sp08_retry_after_failure_converges() {
        let root = temp_root("sp08-retry");
        let mut apply_link = FakeHelperLink::new();
        let lease = apply_tun_lease(&root, "s1", &spec(), &mut apply_link).unwrap();
        let mut failing = FailingCleanupLink;
        cleanup_tun_lease(&root, "s1", &lease, &mut failing).unwrap_err();
        assert!(read_tun_journal(&root, "s1").is_some());
        // The helper is back: the retry acts on the retained owned record and
        // confirms the release.
        let mut recovered = FakeHelperLink::new();
        retry_tun_cleanup(&root, "s1", &mut recovered).expect("retry converges");
        assert!(read_tun_journal(&root, "s1").is_none());
        assert!(read_pending_cleanup(&root, "s1").is_none());
        assert!(list_pending_cleanup(&root).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sp08_retry_still_failing_keeps_journal_and_refreshes_pending() {
        let root = temp_root("sp08-retry-fail");
        let mut apply_link = FakeHelperLink::new();
        apply_tun_lease(&root, "s1", &spec(), &mut apply_link).unwrap();
        let mut failing = FailingCleanupLink;
        retry_tun_cleanup(&root, "s1", &mut failing).unwrap_err();
        retry_tun_cleanup(&root, "s1", &mut failing).unwrap_err();
        assert!(read_tun_journal(&root, "s1").is_some());
        assert_eq!(
            read_pending_cleanup(&root, "s1").expect("pending").attempts,
            2
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sp08_retry_without_journal_clears_stale_pending() {
        let root = temp_root("sp08-retry-nojournal");
        let lease = TunLease::new("ghost", spec(), false);
        let pending = PendingCleanup::from_failure(
            "ghost",
            &lease,
            &crate::helper_client::tun_apply_failed("stale"),
            3,
        );
        write_pending_cleanup(&root, &pending).unwrap();
        let mut link = FakeHelperLink::new();
        retry_tun_cleanup(&root, "ghost", &mut link).expect("no journal is done");
        assert!(list_pending_cleanup(&root).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sp08_pending_record_roundtrips_through_json() {
        let lease = TunLease::new("sess", spec(), false);
        let pending = PendingCleanup::from_failure(
            "s9",
            &lease,
            &crate::helper_client::tun_helper_unavailable("down"),
            4,
        );
        let back: PendingCleanup =
            serde_json::from_slice(&serde_json::to_vec(&pending).unwrap()).unwrap();
        assert_eq!(pending, back);
    }

    // -- SP-10 ownership key (CP-12 归属) -------------------------------------
    //
    // Red contract: the host-side ownership key of one applied lease uses the
    // same normalization as `application::tun_ownership_key` (adapter
    // casing/spacing-insensitive, index and route digest significant), so a
    // reopened manager recovers by journaled ownership, never by current
    // desired settings. The literal format is pinned here because net-host
    // must not depend on the application crate.

    #[test]
    fn sp10_lease_ownership_key_normalizes_adapter_identity() {
        let lease = TunLease::new("helper-session-9", spec(), false);
        let key = lease_ownership_key(&lease);
        assert_eq!(
            key,
            format!("v2rayn-tun|9|{}", lease.route_digest),
            "key pins the shared adapter|index|digest format"
        );
        let mut renamed = spec();
        renamed.adapter_name = "  V2RAYN-TUN ".into();
        let other = TunLease::new("helper-session-9", renamed, false);
        assert_eq!(
            lease_ownership_key(&other),
            key,
            "display casing/spacing is not ownership"
        );
    }

    #[test]
    fn sp10_lease_ownership_key_separates_index_and_digest() {
        let lease = TunLease::new("helper-session-9", spec(), false);
        let base = lease_ownership_key(&lease);
        let mut moved = spec();
        moved.interface_index = 11;
        for route in &mut moved.routes {
            route.interface_index = 11;
        }
        let other = TunLease::new("helper-session-9", moved, false);
        assert_ne!(
            lease_ownership_key(&other),
            base,
            "an index change is a different ownership"
        );
    }
}
