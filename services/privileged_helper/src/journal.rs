//! Helper-side owned-resource journal (SP-08).
//!
//! Records every resource the helper owns on behalf of a session (routes,
//! TUN addresses, elevated kernel handles), per-resource release
//! confirmations, and failure retention for retry. Labels are bounded
//! identifiers only: no executable paths, arguments, or credentials.

use serde::{Deserialize, Serialize};

/// Maximum stored label length (identifiers only, never secret bodies).
const MAX_LABEL_LEN: usize = 128;
/// Maximum stored per-resource error text.
const MAX_ERROR_LEN: usize = 256;
/// Bound on journal growth for a long-lived helper session.
const DEFAULT_JOURNAL_CAPACITY: usize = 1024;

/// What kind of owned resource a journal entry describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JournalKind {
    Route,
    TunAddress,
    Core,
}

/// Lifecycle state of one owned resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JournalState {
    /// Recorded when the helper took ownership; not yet released.
    Owned,
    /// A release call for this resource was confirmed.
    Released,
    /// A release was attempted and failed; the resource must be retried.
    CleanupFailed,
}

/// One journal line: an owned resource plus its cleanup state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    pub kind: JournalKind,
    pub label: String,
    pub state: JournalState,
    pub attempts: u32,
    pub last_error: Option<String>,
}

/// In-memory journal of owned resources for one session.
#[derive(Debug, Clone, Default)]
pub struct ResourceJournal {
    entries: Vec<JournalEntry>,
    capacity: usize,
}

impl ResourceJournal {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            capacity: DEFAULT_JOURNAL_CAPACITY,
        }
    }

    /// Record a newly owned resource. Duplicate owned records for the same
    /// kind+label are not added twice.
    pub fn record_owned(&mut self, kind: JournalKind, label: impl Into<String>) {
        let label = truncate(&label.into(), MAX_LABEL_LEN);
        if self.entries.iter().any(|entry| {
            entry.kind == kind && entry.label == label && entry.state != JournalState::Released
        }) {
            return;
        }
        self.push(JournalEntry {
            kind,
            label,
            state: JournalState::Owned,
            attempts: 0,
            last_error: None,
        });
    }

    /// Confirm a release. Returns true when a live record was confirmed.
    pub fn mark_released(&mut self, kind: JournalKind, label: &str) -> bool {
        match self.entries.iter_mut().find(|entry| {
            entry.kind == kind && entry.label == label && entry.state != JournalState::Released
        }) {
            Some(entry) => {
                entry.state = JournalState::Released;
                entry.last_error = None;
                true
            }
            None => false,
        }
    }

    /// Retain a resource whose release failed. Always visible afterwards:
    /// when no live record exists one is created so the failure cannot be
    /// forgotten.
    pub fn mark_failed(
        &mut self,
        kind: JournalKind,
        label: impl Into<String>,
        error: impl Into<String>,
    ) {
        let label = truncate(&label.into(), MAX_LABEL_LEN);
        let error = truncate(&error.into(), MAX_ERROR_LEN);
        match self.entries.iter_mut().find(|entry| {
            entry.kind == kind && entry.label == label && entry.state != JournalState::Released
        }) {
            Some(entry) => {
                entry.state = JournalState::CleanupFailed;
                entry.attempts += 1;
                entry.last_error = Some(error);
            }
            None => self.push(JournalEntry {
                kind,
                label,
                state: JournalState::CleanupFailed,
                attempts: 1,
                last_error: Some(error),
            }),
        }
    }

    /// Resources awaiting a confirmed release.
    pub fn pending(&self) -> Vec<JournalEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.state == JournalState::CleanupFailed)
            .cloned()
            .collect()
    }

    pub fn pending_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.state == JournalState::CleanupFailed)
            .count()
    }

    pub fn owned_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.state != JournalState::Released)
            .count()
    }

    pub fn snapshot(&self) -> Vec<JournalEntry> {
        self.entries.clone()
    }

    fn push(&mut self, entry: JournalEntry) {
        if self.entries.len() >= self.capacity.max(1) {
            // Never evict an unconfirmed resource to make room: drop the
            // oldest already-released record instead. When everything is
            // unconfirmed the journal grows rather than forgets ownership.
            if let Some(index) = self
                .entries
                .iter()
                .position(|entry| entry.state == JournalState::Released)
            {
                self.entries.remove(index);
            }
        }
        self.entries.push(entry);
    }
}

fn truncate(value: &str, max: usize) -> String {
    if value.len() <= max {
        value.to_string()
    } else {
        value[..max].to_string()
    }
}

/// Bounded journal label for a route (destination + interface only).
pub fn route_label(destination: &str, interface_index: u32) -> String {
    truncate(
        &format!("route {destination} if={interface_index}"),
        MAX_LABEL_LEN,
    )
}

/// Bounded journal label for a TUN address assignment.
pub fn tun_label(interface_index: u32) -> String {
    format!("tun if={interface_index}")
}

/// Bounded journal label for an elevated kernel handle.
pub fn core_label(handle: u64) -> String {
    format!("core handle={handle}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_resources_stay_pending_until_released() {
        let mut journal = ResourceJournal::new();
        journal.record_owned(JournalKind::TunAddress, tun_label(7));
        journal.mark_failed(JournalKind::TunAddress, tun_label(7), "boom");
        assert_eq!(journal.pending_count(), 1);
        assert_eq!(journal.owned_count(), 1);
        assert!(journal.mark_released(JournalKind::TunAddress, &tun_label(7)));
        assert_eq!(journal.pending_count(), 0);
        assert_eq!(journal.owned_count(), 0);
    }

    #[test]
    fn unknown_failures_are_recorded_not_forgotten() {
        let mut journal = ResourceJournal::new();
        journal.mark_failed(JournalKind::Route, route_label("10.9.0.0/16", 7), "boom");
        assert_eq!(journal.pending_count(), 1);
    }

    #[test]
    fn owned_records_are_not_duplicated() {
        let mut journal = ResourceJournal::new();
        journal.record_owned(JournalKind::Core, core_label(3));
        journal.record_owned(JournalKind::Core, core_label(3));
        assert_eq!(journal.owned_count(), 1);
    }
}
