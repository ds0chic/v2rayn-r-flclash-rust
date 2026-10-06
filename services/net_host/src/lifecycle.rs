//! Pure exit-reconciliation decisions (SP-06).
//!
//! No handles, no I/O: given what the handle-authoritative observation found,
//! decide the next state, whether a read path may reconcile at all, and how
//! the fact generation advances. `session` owns the locks and side effects.

use domain::RuntimeState;

/// Reconcile only when no command is in flight and a session exists. An
/// in-flight apply owns the session through readiness; stealing it from a
/// read path would drop the command's outcome.
pub fn should_reconcile(active_operation: Option<&str>, has_session: bool) -> bool {
    active_operation.is_none() && has_session
}

/// Next actual-generation after one exit transition. Every exit pushes the
/// fact generation even though the desired plan did not change (plan §3.1).
pub fn next_generation(current: u64) -> u64 {
    current.saturating_add(1)
}

/// Host-side keep-alive watch values (SP-09, helper protocol v2).
///
/// The privileged helper separates the per-request bound from the long idle
/// bound (24h default), so an active session is never reclaimed for mere UI
/// inactivity. The renew protocol (`RenewLease` / `GetLeaseStatus` with the
/// per-request timeout, consecutive-failure reconciliation and an
/// authenticated owner check) is on the wire since helper protocol v2, and
/// the host calls it from the reconcile pass: leases due for renewal are
/// renewed for their owning helper link, and a dead lease stages through the
/// same exit record as an observed elevated exit (see `session`).
///
/// Values follow RUNTIME_TUN_SOLUTION section 6.1: renew every 15s, lease
/// term 90s, reconcile after 3 consecutive renew failures.
pub const HELPER_RENEW_INTERVAL_MS: u64 = 15_000;
pub const HELPER_LEASE_TERM_MS: u64 = 90_000;
pub const HELPER_RENEW_FAILURES_BEFORE_RECONCILE: u32 = 3;

/// Whether a renew is due. Pure virtual-time decision (`ms` on one monotonic
/// clock); the reconcile pass issues the actual renew RPC.
pub fn renew_due(last_renew_ms: u64, now_ms: u64) -> bool {
    now_ms.saturating_sub(last_renew_ms) >= HELPER_RENEW_INTERVAL_MS
}

/// Whether the lease term lapsed without confirmation. An active session that
/// keeps renewing never hits this, however long the UI stays idle; only a
/// dead owner (no renew, no traffic confirmation) expires.
pub fn lease_expired(last_confirmed_ms: u64, now_ms: u64) -> bool {
    now_ms.saturating_sub(last_confirmed_ms) >= HELPER_LEASE_TERM_MS
}

/// Whether consecutive renew failures force reconciliation instead of another
/// blind retry.
pub fn renew_reconcile_due(consecutive_failures: u32) -> bool {
    consecutive_failures >= HELPER_RENEW_FAILURES_BEFORE_RECONCILE
}

/// Ownership rule for recovery (SP-09): a lease survives UI inactivity and
/// manager silence; only a dead owner loses it. A reopened manager must
/// recover by the journaled ownership (helper session + adapter identity +
/// route digest), never by the current desired settings or by guessing.
#[allow(dead_code)]
pub fn active_lease_survives_ui_idle() -> bool {
    true
}

/// Overall readiness of one managed session (SP-10, CP-12 / TUN-A05).
///
/// One pure decision over the main core, every sidecar and the
/// elevated-observation state: a dead main core is never Ready; a failed
/// sidecar — or an elevated sidecar still without helper exit observation
/// (A02 `PollCoreExits` pending) — degrades instead of reporting clean
/// Running. Staged for the A02 wiring; covered by SP-10 unit tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SessionReadiness {
    Ready,
    Degraded,
    NotReady,
}

/// Aggregate one readiness verdict (SP-10).
///
/// `sidecar_ready` carries one entry per tracked sidecar; `elevated_pending`
/// is true while an elevated sidecar has no exit-observation channel yet.
/// Pure and side-effect-free. Staged for the A02 wiring.
#[allow(dead_code)]
pub fn session_readiness(
    main_ready: bool,
    sidecar_ready: &[bool],
    elevated_pending: bool,
) -> SessionReadiness {
    if !main_ready {
        return SessionReadiness::NotReady;
    }
    if elevated_pending || sidecar_ready.iter().any(|ready| !ready) {
        return SessionReadiness::Degraded;
    }
    SessionReadiness::Ready
}

/// State after a sidecar exit while the session record still exists: the live
/// main core keeps its endpoint, but the session must read Degraded, never a
/// clean Running. When the main core is already gone the session is Stopped.
pub fn state_for_sidecar_exit(main_alive: bool) -> RuntimeState {
    if main_alive {
        RuntimeState::Degraded
    } else {
        RuntimeState::Stopped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconcile_is_gated_on_quiescence() {
        assert!(should_reconcile(None, true));
        assert!(!should_reconcile(Some("op-1"), true));
        assert!(!should_reconcile(None, false));
        assert!(!should_reconcile(Some("op-1"), false));
    }

    #[test]
    fn every_exit_advances_the_fact_generation() {
        assert_eq!(next_generation(0), 1);
        assert_eq!(next_generation(7), 8);
        assert_eq!(next_generation(u64::MAX), u64::MAX);
    }

    #[test]
    fn sidecar_exit_degrades_a_live_session() {
        assert_eq!(state_for_sidecar_exit(true), RuntimeState::Degraded);
        assert_eq!(state_for_sidecar_exit(false), RuntimeState::Stopped);
    }

    // -- SP-09 keep-alive preparation (CP-12) -------------------------------

    #[test]
    fn renew_is_due_on_the_provisional_interval() {
        assert!(!renew_due(0, 0));
        assert!(!renew_due(0, HELPER_RENEW_INTERVAL_MS - 1));
        assert!(renew_due(0, HELPER_RENEW_INTERVAL_MS));
        assert!(renew_due(1_000, 1_000 + HELPER_RENEW_INTERVAL_MS));
    }

    #[test]
    fn lease_term_covers_the_renew_cycle_with_margin() {
        // Six renew intervals fit inside one term: a single missed renew
        // never expires an otherwise active lease.
        const {
            assert!(HELPER_LEASE_TERM_MS >= 6 * HELPER_RENEW_INTERVAL_MS);
        }
        assert!(!lease_expired(0, HELPER_LEASE_TERM_MS - 1));
        assert!(lease_expired(0, HELPER_LEASE_TERM_MS));
    }

    #[test]
    fn active_session_survives_24h_of_ui_idle_with_steady_renew() {
        // Virtual 24h: renew every interval, confirm every renew. The lease
        // must never read expired, however long nobody clicks.
        let day_ms: u64 = 24 * 60 * 60 * 1_000;
        let mut last_confirmed = 0u64;
        let mut now = 0u64;
        while now <= day_ms {
            if renew_due(last_confirmed, now) {
                last_confirmed = now;
            }
            assert!(
                !lease_expired(last_confirmed, now),
                "steady renew must hold the lease across 24h of UI idle"
            );
            now += HELPER_RENEW_INTERVAL_MS;
        }
    }
    #[test]
    fn dead_owner_expires_after_the_term_without_renew() {
        // No renew and no confirmation for the whole term: the lease lapses
        // and the owner must reconcile rather than assume ownership.
        assert!(lease_expired(5_000, 5_000 + HELPER_LEASE_TERM_MS));
        assert!(renew_reconcile_due(HELPER_RENEW_FAILURES_BEFORE_RECONCILE));
        assert!(!renew_reconcile_due(
            HELPER_RENEW_FAILURES_BEFORE_RECONCILE - 1
        ));
    }

    // -- SP-10 readiness aggregation (CP-12) --------------------------------
    //
    // Red contract: overall readiness is one pure decision over the main
    // core, every sidecar and the elevated-observation state. A dead main
    // core is never Ready; a failed sidecar or an elevated sidecar without
    // helper observation degrades instead of reporting clean Running.

    #[test]
    fn sp10_session_readiness_aggregates_main_and_sidecars() {
        assert_eq!(
            session_readiness(false, &[], false),
            SessionReadiness::NotReady
        );
        assert_eq!(session_readiness(true, &[], false), SessionReadiness::Ready);
        assert_eq!(
            session_readiness(true, &[true, true], false),
            SessionReadiness::Ready
        );
        assert_eq!(
            session_readiness(true, &[true, false], false),
            SessionReadiness::Degraded
        );
        assert_eq!(
            session_readiness(false, &[true], false),
            SessionReadiness::NotReady
        );
        assert_eq!(
            session_readiness(true, &[true], true),
            SessionReadiness::Degraded
        );
    }
}
