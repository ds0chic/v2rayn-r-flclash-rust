//! Event envelope and event classification (plan §14).
//!
//! Control events (state transitions, job progress, errors) travel on a
//! reliable channel and must never be dropped by telemetry pressure.
//! Telemetry events (traffic, logs, connections, speedtest increments) are
//! rate-limited independently.
//!
//! Every event carries an `epoch` (bumped when the producing runtime restarts)
//! and a monotonic `seq`. A client that loses the stream can detect the gap
//! from `seq` and re-fetch a snapshot instead of guessing.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::DomainError;
use crate::job::{JobId, JobState};

/// Epoch identifier. Incremented by net-host on each fresh runtime generation.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(transparent)]
pub struct EventEpoch(pub u64);

impl EventEpoch {
    pub const fn get(self) -> u64 {
        self.0
    }

    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// Monotonic sequence number within an epoch.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(transparent)]
pub struct EventSeq(pub u64);

impl EventSeq {
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Whether `other` is the expected immediate successor of `self`.
    pub fn is_contiguous_with(self, other: EventSeq) -> bool {
        other.0 == self.0 + 1
    }
}

/// Whether an event is on the reliable control channel or the throttled
/// telemetry channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventChannel {
    /// Reliable; must not be dropped under log/traffic pressure.
    Control,
    /// Best-effort; independently rate-limited per family.
    Telemetry,
}

/// Known event kinds. Unknown kinds deserialize into [`EventKind::Other`] so a
/// newer producer never crashes an older consumer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    /// Runtime state machine transition (Stopped/Preparing/Running/...).
    RuntimeStateChanged,
    /// A job advanced its lifecycle state.
    JobProgress,
    /// A job reached a terminal state.
    JobFinished,
    /// A structured, user-relevant error.
    ErrorRaised,
    /// Desired/applied revisions changed.
    RevisionChanged,
    /// Traffic counters delta (telemetry).
    TrafficDelta,
    /// Log line batch (telemetry).
    LogBatch,
    /// Connection list batch (telemetry).
    ConnectionsBatch,
    /// Speedtest result batch (telemetry).
    SpeedTestBatch,
    /// Any kind produced by a newer peer.
    Other(String),
}

impl EventKind {
    /// The wire token for this kind.
    pub fn as_str(&self) -> &str {
        match self {
            EventKind::RuntimeStateChanged => "runtime_state_changed",
            EventKind::JobProgress => "job_progress",
            EventKind::JobFinished => "job_finished",
            EventKind::ErrorRaised => "error_raised",
            EventKind::RevisionChanged => "revision_changed",
            EventKind::TrafficDelta => "traffic_delta",
            EventKind::LogBatch => "log_batch",
            EventKind::ConnectionsBatch => "connections_batch",
            EventKind::SpeedTestBatch => "speed_test_batch",
            EventKind::Other(raw) => raw,
        }
    }
}

impl Serialize for EventKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for EventKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Ok(match raw.as_str() {
            "runtime_state_changed" => EventKind::RuntimeStateChanged,
            "job_progress" => EventKind::JobProgress,
            "job_finished" => EventKind::JobFinished,
            "error_raised" => EventKind::ErrorRaised,
            "revision_changed" => EventKind::RevisionChanged,
            "traffic_delta" => EventKind::TrafficDelta,
            "log_batch" => EventKind::LogBatch,
            "connections_batch" => EventKind::ConnectionsBatch,
            "speed_test_batch" => EventKind::SpeedTestBatch,
            other => EventKind::Other(other.to_string()),
        })
    }
}

impl EventKind {
    pub const fn channel(&self) -> EventChannel {
        match self {
            EventKind::TrafficDelta
            | EventKind::LogBatch
            | EventKind::ConnectionsBatch
            | EventKind::SpeedTestBatch => EventChannel::Telemetry,
            _ => EventChannel::Control,
        }
    }

    pub const fn is_control(&self) -> bool {
        matches!(self.channel(), EventChannel::Control)
    }
}

/// The universal event envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub epoch: EventEpoch,
    pub seq: EventSeq,
    pub kind: EventKind,
    pub payload: Value,
}

impl EventEnvelope {
    pub fn new(epoch: EventEpoch, seq: EventSeq, kind: EventKind, payload: Value) -> Self {
        Self {
            epoch,
            seq,
            kind,
            payload,
        }
    }

    /// Detect a gap between a previously seen sequence and this event.
    pub fn follows(&self, previous: EventSeq) -> bool {
        previous.is_contiguous_with(self.seq)
    }
}

/// Control payload for a runtime state transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeStateChanged {
    pub state: RuntimeState,
    /// Applied revision reported by net-host after the transition.
    pub applied_revision: u64,
    pub message_key: Option<String>,
}

/// The runtime state machine (plan §13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeState {
    #[default]
    Stopped,
    Validating,
    Preparing,
    Starting,
    Checking,
    Running,
    RollingBack,
    Degraded,
}

/// Control payload for job progress / completion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobEvent {
    pub job_id: JobId,
    pub state: JobState,
    /// 0..=100 when known, `None` when the stage is unknown (never fake it).
    pub percent: Option<u8>,
    pub stage_key: Option<String>,
    pub error: Option<DomainError>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn control_and_telemetry_channels() {
        assert!(EventKind::RuntimeStateChanged.is_control());
        assert!(!EventKind::TrafficDelta.is_control());
        assert_eq!(EventKind::LogBatch.channel(), EventChannel::Telemetry);
    }

    #[test]
    fn unknown_kind_roundtrips() {
        let kind: EventKind = serde_json::from_value(json!("brand_new")).unwrap();
        assert_eq!(kind, EventKind::Other("brand_new".into()));
        assert_eq!(serde_json::to_value(&kind).unwrap(), json!("brand_new"));
    }

    #[test]
    fn known_kind_roundtrips() {
        let kind: EventKind = serde_json::from_value(json!("log_batch")).unwrap();
        assert_eq!(kind, EventKind::LogBatch);
        assert_eq!(serde_json::to_value(&kind).unwrap(), json!("log_batch"));
    }

    #[test]
    fn seq_contiguity() {
        assert!(EventSeq(4).is_contiguous_with(EventSeq(5)));
        assert!(!EventSeq(4).is_contiguous_with(EventSeq(6)));
    }
}
