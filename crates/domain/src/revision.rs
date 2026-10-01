//! Desired vs. applied revision model.
//!
//! Saving a setting and applying it to a running core are two different
//! commits (plan §5, §13). A single `activeRevision` must never be used to
//! represent both database state and runtime state.
//!
//! - [`DesiredRevision`] increments when AppEngine persists a mutation.
//! - [`AppliedRevision`] is what net-host reports as actually running.
//!
//! `desired > applied` is a legal, displayable state ("saved, not applied").

use serde::{Deserialize, Serialize};

/// Monotonic revision of the persisted desired state.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(transparent)]
pub struct DesiredRevision(pub u64);

impl DesiredRevision {
    pub const ZERO: DesiredRevision = DesiredRevision(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl std::fmt::Display for DesiredRevision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "desired:{}", self.0)
    }
}

/// Monotonic revision of what net-host has actually applied.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(transparent)]
pub struct AppliedRevision(pub u64);

impl AppliedRevision {
    pub const ZERO: AppliedRevision = AppliedRevision(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for AppliedRevision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "applied:{}", self.0)
    }
}

/// Comparison result of the desired/applied pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevisionState {
    /// No desired state has ever been saved.
    Empty,
    /// Saved and the runtime has caught up.
    InSync,
    /// Saved but not yet applied to the runtime.
    Pending,
    /// The runtime reports a revision ahead of the saved state. This means the
    /// applied revision was advanced by another writer/older session and must
    /// be reconciled, not overwritten blindly.
    Ahead,
}

/// A desired/applied pair plus its derived state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionPair {
    pub desired: DesiredRevision,
    pub applied: AppliedRevision,
}

impl RevisionPair {
    pub const fn new(desired: DesiredRevision, applied: AppliedRevision) -> Self {
        Self { desired, applied }
    }

    pub fn state(&self) -> RevisionState {
        if self.desired == DesiredRevision::ZERO && self.applied == AppliedRevision::ZERO {
            RevisionState::Empty
        } else if self.applied.get() > self.desired.get() {
            RevisionState::Ahead
        } else if self.applied == AppliedRevision(self.desired.get()) {
            RevisionState::InSync
        } else {
            RevisionState::Pending
        }
    }

    /// Reject a mutation whose `expected` does not match the current desired
    /// revision. This is the optimistic-concurrency check used by
    /// `save_profile` / `save_settings`.
    pub fn check_expected(
        &self,
        expected: DesiredRevision,
    ) -> Result<(), crate::error::DomainError> {
        if expected == self.desired {
            Ok(())
        } else {
            Err(crate::error::DomainError::stale_revision(
                expected.get(),
                self.desired.get(),
            ))
        }
    }
}

impl Default for RevisionPair {
    fn default() -> Self {
        Self {
            desired: DesiredRevision::ZERO,
            applied: AppliedRevision::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::codes;

    #[test]
    fn empty_and_in_sync_states() {
        let empty = RevisionPair::default();
        assert_eq!(empty.state(), RevisionState::Empty);

        let in_sync = RevisionPair::new(DesiredRevision::new(3), AppliedRevision::new(3));
        assert_eq!(in_sync.state(), RevisionState::InSync);
    }

    #[test]
    fn pending_when_desired_is_ahead_of_applied() {
        let pending = RevisionPair::new(DesiredRevision::new(5), AppliedRevision::new(2));
        assert_eq!(pending.state(), RevisionState::Pending);
    }

    #[test]
    fn ahead_when_applied_is_ahead_of_desired() {
        let ahead = RevisionPair::new(DesiredRevision::new(2), AppliedRevision::new(9));
        assert_eq!(ahead.state(), RevisionState::Ahead);
    }

    #[test]
    fn check_expected_accepts_matching_and_rejects_stale() {
        let pair = RevisionPair::new(DesiredRevision::new(4), AppliedRevision::new(4));
        assert!(pair.check_expected(DesiredRevision::new(4)).is_ok());

        let error = pair
            .check_expected(DesiredRevision::new(3))
            .expect_err("stale expected revision must be rejected");
        assert_eq!(error.code, codes::REVISION_STALE);
        assert_eq!(error.message_key, "error.revision_stale");
    }

    #[test]
    fn next_advances_desired_by_one() {
        assert_eq!(DesiredRevision::new(7).next(), DesiredRevision::new(8));
        assert_eq!(DesiredRevision::ZERO.next(), DesiredRevision::new(1));
    }
}
