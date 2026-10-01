//! Event bus for net-host (plan §14).
//!
//! Control events are monotonic per epoch and broadcast to every subscriber.
//! The epoch is bumped whenever a fresh runtime generation starts.

use std::sync::atomic::{AtomicU64, Ordering};

use domain::event::{EventEnvelope, EventEpoch, EventKind, EventSeq};
use serde_json::Value;
use tokio::sync::broadcast;

const CHANNEL_CAPACITY: usize = 1024;

pub struct EventBus {
    tx: broadcast::Sender<EventEnvelope>,
    epoch: AtomicU64,
    seq: AtomicU64,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(CHANNEL_CAPACITY);
        Self {
            tx,
            epoch: AtomicU64::new(1),
            seq: AtomicU64::new(0),
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
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
