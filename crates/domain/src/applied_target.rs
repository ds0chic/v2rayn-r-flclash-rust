//! Frozen applied target and the single operation/job vocabulary (SP-05).
//!
//! Stable-port plan §3.1/§3.2 (four identities, three revisions): the default
//! the user edits (`desiredDefaultProfileId`) is never the fact of what runs.
//! The target a runtime plan is accepted for is frozen **at submit time**
//! ([`FrozenAppliedTarget`]) together with the plan, revision, operation id,
//! intent sequence and actual generation. A later default change must not
//! rewrite it, and a stop withdraws the live session without rewriting the
//! history entry.
//!
//! `ipc_contract::stable::OperationState` stays the versioned wire vocabulary
//! while [`crate::job::JobState`] stays the historical job facility enum. The
//! two are not parallel unmapped enums: [`stable_operation_name`] and
//! [`job_state_from_stable_name`] define the one canonical mapping both sides
//! share. This module owns no wire type (it cannot depend on `ipc_contract`),
//! so the mapping is expressed with the stable snake-case names.

use serde::{Deserialize, Serialize};

use crate::enums::CoreType;
use crate::job::JobState;

/// The runtime target frozen when one apply was accepted.
///
/// Every field is a submit-time fact: `target_profile_id` is the node the
/// accepted plan was built for (never the current desired default),
/// `operation_id`/`intent_seq` correlate the submit, and `actual_generation`
/// is the fact generation this accept created. The record is retained as
/// history across stop so reopen can report applied-vs-actual honestly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrozenAppliedTarget {
    pub target_profile_id: String,
    pub plan_id: String,
    pub config_sha256: String,
    pub core: CoreType,
    pub desired_revision: u64,
    pub operation_id: String,
    pub intent_seq: u64,
    pub actual_generation: u64,
}

impl FrozenAppliedTarget {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        target_profile_id: String,
        plan_id: String,
        config_sha256: String,
        core: CoreType,
        desired_revision: u64,
        operation_id: String,
        intent_seq: u64,
        actual_generation: u64,
    ) -> Self {
        Self {
            target_profile_id,
            plan_id,
            config_sha256,
            core,
            desired_revision,
            operation_id,
            intent_seq,
            actual_generation,
        }
    }
}

/// Canonical `stable::OperationState` name for one [`JobState`].
///
/// The job facility has no `Accepted` (a started job is already admitted) and
/// no `Superseded` (a superseded apply never runs): both map onto the nearest
/// job lifecycle fact. `Cancelling`/`Compensating` are still in flight, so
/// they read as `executing`, never as a terminal state.
pub fn stable_operation_name(state: JobState) -> &'static str {
    match state {
        JobState::Running | JobState::Cancelling | JobState::Compensating => "executing",
        JobState::Done => "succeeded",
        JobState::Failed => "failed",
        JobState::Cancelled => "cancelled",
    }
}

/// [`JobState`] for one canonical `stable::OperationState` name.
///
/// `accepted` has not executed yet, so it reads as `Running`; `superseded`
/// never executes, so it reads as the terminal `Cancelled` (replaced), never
/// as success. Unknown names return `None` instead of guessing.
pub fn job_state_from_stable_name(name: &str) -> Option<JobState> {
    Some(match name {
        "accepted" | "executing" => JobState::Running,
        "succeeded" => JobState::Done,
        "failed" => JobState::Failed,
        "cancelled" => JobState::Cancelled,
        "superseded" => JobState::Cancelled,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_target_round_trips() {
        let frozen = FrozenAppliedTarget::new(
            "profile-a".into(),
            "rt-xray-profile-a-3".into(),
            "ab12".into(),
            CoreType::Xray,
            3,
            "op-1".into(),
            7,
            2,
        );
        let json = serde_json::to_string(&frozen).unwrap();
        let back: FrozenAppliedTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(back, frozen);
        assert_eq!(back.target_profile_id, "profile-a");
        assert_eq!(back.intent_seq, 7);
        assert_eq!(back.actual_generation, 2);
    }

    #[test]
    fn every_job_state_has_one_stable_name() {
        use JobState::*;
        let cases = [
            (Running, "executing"),
            (Cancelling, "executing"),
            (Compensating, "executing"),
            (Done, "succeeded"),
            (Failed, "failed"),
            (Cancelled, "cancelled"),
        ];
        for (job, name) in cases {
            assert_eq!(stable_operation_name(job), name);
        }
    }

    #[test]
    fn stable_names_map_back_without_guessing() {
        use JobState::*;
        assert_eq!(job_state_from_stable_name("accepted"), Some(Running));
        assert_eq!(job_state_from_stable_name("executing"), Some(Running));
        assert_eq!(job_state_from_stable_name("succeeded"), Some(Done));
        assert_eq!(job_state_from_stable_name("failed"), Some(Failed));
        assert_eq!(job_state_from_stable_name("cancelled"), Some(Cancelled));
        // Superseded never runs: terminal cancelled, never success.
        assert_eq!(job_state_from_stable_name("superseded"), Some(Cancelled));
        assert_eq!(job_state_from_stable_name("running"), None);
        assert_eq!(job_state_from_stable_name(""), None);
    }
}
