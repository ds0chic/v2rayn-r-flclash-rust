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
}
