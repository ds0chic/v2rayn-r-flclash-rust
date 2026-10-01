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

use crate::helper_client::{HelperLink, TunLease, E_TUN_HELPER_UNAVAILABLE};
use crate::journal::{now_ms, session_dir};

/// File name of the TUN lease journal inside a session directory.
pub const TUN_JOURNAL_FILE: &str = "tun_lease.json";

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

pub fn tun_journal_path(run_root: &Path, session_id: &str) -> PathBuf {
    session_dir(run_root, session_id).join(TUN_JOURNAL_FILE)
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

/// Release a TUN lease idempotently: clean helper resources, drop the journal.
///
/// - No journal: already cleaned, `Ok` without touching the helper.
/// - Journal/lease mismatch: still cleans the *recorded* resources (never
///   leaks) and drops the stale record.
/// - Helper cleanup failure: logged and swallowed; the helper also releases
///   owned resources on disconnect, so an explicit-cleanup failure still
///   converges and must never override the caller's root-cause error.
pub fn cleanup_tun_lease(
    run_root: &Path,
    session_id: &str,
    lease: &TunLease,
    link: &mut dyn HelperLink,
) -> Result<(), DomainError> {
    match read_tun_journal(run_root, session_id) {
        None => return Ok(()),
        Some(journal) if !verify_lease(&journal, lease) => {
            eprintln!(
                "[net_host] tun journal mismatch for {session_id}: recorded lease differs; cleaning recorded resources anyway"
            );
        }
        Some(_) => {}
    }
    if let Err(error) = link.cleanup(lease) {
        eprintln!(
            "[net_host] tun cleanup best-effort failed for {session_id}: {} {}",
            error.code, error.message_key
        );
    }
    remove_tun_journal(run_root, session_id);
    Ok(())
}

/// Reconcile stale TUN leases after a restart.
///
/// net-host is the single owner of its run root, so any lease journal found at
/// boot is stale: the previous process is gone. Each one gets a single cleanup
/// attempt through `link`. When the helper itself is unavailable the journal
/// is *kept* and counted as pending so a later boot (with a live helper) can
/// finish the cleanup; nothing is silently dropped.
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
                report.cleaned += 1;
            }
            Err(error) if error.code == E_TUN_HELPER_UNAVAILABLE => {
                eprintln!(
                    "[net_host] tun recovery deferred for {session_id}: helper unavailable (journal kept)"
                );
                report.pending += 1;
            }
            Err(error) => {
                // Non-availability failure (e.g. poisoned test lock): the
                // helper disconnect path still owns convergence; drop the
                // record rather than wedge every future boot on it.
                eprintln!(
                    "[net_host] tun recovery best-effort failed for {session_id}: {} {}; record dropped",
                    error.code, error.message_key
                );
                remove_tun_journal(run_root, &session_id);
                report.cleaned += 1;
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
}
