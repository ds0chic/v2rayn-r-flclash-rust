//! Structured audit log for helper operations.
//!
//! Records operation type, a bounded summary and the outcome. Summaries are
//! deliberately built from counts and allow-listed identifiers only: no config
//! bodies, no executable paths, no argument contents, no credentials.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// Outcome of one audited operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Ok,
    Rejected,
    Error,
}

/// One audit record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    pub ts_ms: i64,
    pub session_id: String,
    pub operation: String,
    pub summary: String,
    pub outcome: AuditOutcome,
}

/// In-memory audit log. The backing vector is bounded to avoid unbounded growth
/// in a long-lived helper.
#[derive(Debug)]
pub struct AuditLog {
    records: Mutex<Vec<AuditRecord>>,
    capacity: usize,
}

const DEFAULT_AUDIT_CAPACITY: usize = 4096;

impl Default for AuditLog {
    fn default() -> Self {
        Self::new(DEFAULT_AUDIT_CAPACITY)
    }
}

impl AuditLog {
    pub fn new(capacity: usize) -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            capacity: capacity.max(1),
        }
    }

    pub fn record(&self, session_id: &str, operation: &str, summary: &str, outcome: AuditOutcome) {
        let mut records = self.records.lock().expect("audit log poisoned");
        if records.len() >= self.capacity {
            records.remove(0);
        }
        records.push(AuditRecord {
            ts_ms: now_ms(),
            session_id: session_id.to_string(),
            operation: operation.to_string(),
            summary: summary.to_string(),
            outcome,
        });
    }

    pub fn records(&self) -> Vec<AuditRecord> {
        self.records.lock().expect("audit log poisoned").clone()
    }

    pub fn len(&self) -> usize {
        self.records.lock().expect("audit log poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Wall-clock milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_are_appended_and_bounded() {
        let log = AuditLog::new(2);
        log.record("s1", "ping", "ping", AuditOutcome::Ok);
        log.record("s1", "add_routes", "add_routes count=1", AuditOutcome::Ok);
        log.record("s1", "shutdown", "shutdown", AuditOutcome::Ok);
        let records = log.records();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].operation, "add_routes");
        assert_eq!(records[1].operation, "shutdown");
    }
}
