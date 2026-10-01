//! Import batch bookkeeping and idempotency.
//!
//! A batch is keyed by `source_fingerprint = H(source_id || content_hash)`.
//! Re-importing the same source returns [`ImportStatus::AlreadyImported`] and
//! touches nothing, so a later user edit is never overwritten (plan §11).

use serde::{Deserialize, Serialize};

use crate::hash::sha256_hex;
use crate::report::EntityCount;

/// One recorded import batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportBatch {
    pub source_fingerprint: String,
    pub source_id: String,
    pub content_hash: String,
    pub batch_id: String,
    pub source_kind: String,
    pub source_version: i32,
    pub imported_at: i64,
    pub entity_counts: Vec<EntityCount>,
}

/// Compute the stable fingerprint from the source identity and content hash.
pub fn fingerprint(source_id: &str, content_hash: &str) -> String {
    sha256_hex(format!("{source_id}\u{1f}{content_hash}").as_bytes())
}

/// Source directory identity: a stable id plus the combined content hash of the
/// `guiNConfig.json` and `guiNDB.db` files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceIdentity {
    pub source_id: String,
    pub content_hash: String,
}

impl SourceIdentity {
    pub fn fingerprint(&self) -> String {
        fingerprint(&self.source_id, &self.content_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_stable_and_source_sensitive() {
        let a = fingerprint("src-a", "hash-1");
        assert_eq!(a, fingerprint("src-a", "hash-1"));
        assert_ne!(a, fingerprint("src-b", "hash-1"));
        assert_ne!(a, fingerprint("src-a", "hash-2"));
        assert_eq!(a.len(), 64);
    }
}
