//! Event bus for net-host (plan §14).
//!
//! Control events are monotonic per epoch and broadcast to every subscriber.
//! The epoch is bumped whenever a fresh runtime generation starts.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use domain::event::{EventEnvelope, EventEpoch, EventKind, EventSeq};
use serde_json::Value;
use tokio::sync::broadcast;

const CHANNEL_CAPACITY: usize = 1024;

/// Control event asking a lagged/disconnected subscriber to resynchronize.
///
/// Carried as `EventKind::Other(RESYNC_REQUIRED_EVENT)` so no IPC/stable DTO
/// changes: the authoritative cursor (`epoch`/`last_seq`) travels in the
/// payload, and the client answers with `GetSnapshot` + a fresh subscribe.
pub const RESYNC_REQUIRED_EVENT: &str = "resync_required";

/// Payload field carrying the originating operation id (SP-07 control identity).
pub const OPERATION_ID_FIELD: &str = "operation_id";
/// Payload field carrying the fact generation at emit time.
pub const GENERATION_FIELD: &str = "generation";

/// Build a control payload that always carries `operation_id`/`generation`
/// alongside the envelope `epoch`/`seq`, without changing the wire DTO.
pub fn control_payload(operation_id: Option<&str>, generation: u64, body: Value) -> Value {
    let mut object = match body {
        Value::Object(map) => map,
        other => {
            let mut map = serde_json::Map::new();
            map.insert("body".to_string(), other);
            map
        }
    };
    match operation_id {
        Some(id) => {
            object.insert(
                OPERATION_ID_FIELD.to_string(),
                Value::String(id.to_string()),
            );
        }
        None => {
            object.remove(OPERATION_ID_FIELD);
        }
    }
    object.insert(GENERATION_FIELD.to_string(), Value::from(generation));
    Value::Object(object)
}

/// Read the `(operation_id, generation)` control identity from an envelope.
/// Envelope `epoch`/`seq` stay authoritative for ordering; these fields only
/// attribute the event to the operation/fact generation that produced it.
pub fn parse_event_identity(envelope: &EventEnvelope) -> (Option<String>, Option<u64>) {
    let operation_id = envelope
        .payload
        .get(OPERATION_ID_FIELD)
        .and_then(Value::as_str)
        .map(str::to_string);
    let generation = envelope
        .payload
        .get(GENERATION_FIELD)
        .and_then(Value::as_u64);
    (operation_id, generation)
}

/// Cloneable so background log readers can emit onto the same bus. The
/// counters are shared through `Arc<AtomicU64>`; cloning shares epoch/seq.
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<EventEnvelope>,
    epoch: Arc<AtomicU64>,
    seq: Arc<AtomicU64>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(CHANNEL_CAPACITY);
        Self {
            tx,
            epoch: Arc::new(AtomicU64::new(1)),
            seq: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.tx.subscribe()
    }

    pub fn epoch(&self) -> EventEpoch {
        EventEpoch(self.epoch.load(Ordering::Acquire))
    }

    pub fn last_seq(&self) -> u64 {
        self.seq.load(Ordering::Acquire)
    }

    /// Begin a new runtime generation.
    pub fn bump_epoch(&self) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
        self.seq.store(0, Ordering::Release);
    }

    /// Build an envelope (allocating a sequence) without broadcasting it.
    pub fn make(&self, kind: EventKind, payload: Value) -> EventEnvelope {
        let seq = self.seq.fetch_add(1, Ordering::AcqRel) + 1;
        EventEnvelope::new(self.epoch(), EventSeq(seq), kind, payload)
    }

    pub fn emit(&self, kind: EventKind, payload: Value) -> EventEnvelope {
        let envelope = self.make(kind, payload);
        let _ = self.tx.send(envelope.clone());
        envelope
    }

    /// Emit a named event (mapped to `EventKind::Other`).
    pub fn emit_named(&self, name: &str, payload: Value) -> EventEnvelope {
        self.emit(EventKind::Other(name.to_string()), payload)
    }

    /// Emit a control event attributed to an operation/fact generation.
    ///
    /// Producer contract for the session emitter (`session.rs` owner): the
    /// reconcile paths attach the operation/fact generation they transition.
    #[allow(dead_code)]
    pub fn emit_control(
        &self,
        kind: EventKind,
        operation_id: Option<&str>,
        generation: u64,
        body: Value,
    ) -> EventEnvelope {
        debug_assert!(
            kind.is_control(),
            "control identity is only meaningful on the control channel"
        );
        self.emit(kind, control_payload(operation_id, generation, body))
    }

    /// Build (and broadcast) the resync notice a lagged subscriber must act on.
    pub fn resync_notice(&self, reason: &str, epoch: u64, last_seq: u64) -> EventEnvelope {
        self.emit(
            EventKind::Other(RESYNC_REQUIRED_EVENT.to_string()),
            serde_json::json!({
                "reason": reason,
                "epoch": epoch,
                "last_seq": last_seq,
            }),
        )
    }

    /// Resync notice attributed with the last forwarded control identity, so
    /// the client's snapshot reconcile knows which operation was last seen.
    pub fn resync_notice_attributed(
        &self,
        reason: &str,
        epoch: u64,
        last_seq: u64,
        operation_id: Option<&str>,
        generation: Option<u64>,
    ) -> EventEnvelope {
        let body = serde_json::json!({
            "reason": reason,
            "epoch": epoch,
            "last_seq": last_seq,
        });
        let payload = match generation {
            Some(generation) => control_payload(operation_id, generation, body),
            // No known generation: attribute the operation only, never
            // fabricate a generation number.
            None => {
                let mut object = body.as_object().cloned().unwrap_or_default();
                if let Some(id) = operation_id {
                    object.insert(
                        OPERATION_ID_FIELD.to_string(),
                        Value::String(id.to_string()),
                    );
                }
                Value::Object(object)
            }
        };
        self.emit(EventKind::Other(RESYNC_REQUIRED_EVENT.to_string()), payload)
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn burst_overflow_reports_lagged_but_stream_continues() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        for i in 0..2049u64 {
            bus.emit(EventKind::LogBatch, json!({"i": i}));
        }
        match rx.try_recv() {
            Err(broadcast::error::TryRecvError::Lagged(skipped)) => {
                assert!(skipped > 0, "burst must skip messages");
            }
            other => panic!("expected Lagged after 2049-event burst, got {other:?}"),
        }
        let env = bus.emit(EventKind::RuntimeStateChanged, json!({"state": "running"}));
        // A lagged receiver re-lags if the producer keeps writing, so a slow
        // subscriber must tolerate repeated Lagged while draining; once the
        // burst ends the control event is still reachable (never EOF).
        let mut seen = false;
        for _ in 0..4096 {
            match rx.try_recv() {
                Ok(got) => {
                    if got.seq == env.seq {
                        seen = true;
                        break;
                    }
                }
                Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
        assert!(seen, "post-lag control event must still be receivable");
    }

    #[test]
    fn control_event_carries_operation_id_and_generation() {
        let bus = EventBus::new();
        let env = bus.emit_control(
            EventKind::JobFinished,
            Some("op-sp07"),
            9,
            json!({"ok": true}),
        );
        assert_eq!(env.epoch, bus.epoch());
        assert!(env.kind.is_control());
        let (operation_id, generation) = parse_event_identity(&env);
        assert_eq!(operation_id.as_deref(), Some("op-sp07"));
        assert_eq!(generation, Some(9));
    }

    #[test]
    fn resync_notice_is_control_and_names_reason() {
        let bus = EventBus::new();
        let env = bus.resync_notice("lagged", bus.epoch().get(), bus.last_seq());
        assert!(env.kind.is_control());
        assert_eq!(env.kind.as_str(), RESYNC_REQUIRED_EVENT);
        assert_eq!(
            env.payload.get("reason").and_then(|v| v.as_str()),
            Some("lagged")
        );
    }
}
