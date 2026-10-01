//! Reference-expression model.
//!
//! Upstream references are *not* all foreign keys. `compat/fields.yaml`
//! (`reference_semantics`) documents six resolution classes:
//!
//! | id | class | target |
//! |---|---|---|
//! | REF-ENT-001 | `string_remarks` | RoutingRule.OutboundTag -> Profile.Remarks |
//! | REF-ENT-002 | `string_remarks` | Subscription.Prev/NextProfile -> Profile.Remarks |
//! | REF-ENT-003 | `comma_separated_index_ids` | Group.ChildItems -> Profile.IndexId |
//! | REF-ENT-004 | `subscription_id_plus_regex` | Group.SubChildItems -> Sub id + filter regex |
//! | REF-ENT-005 | `id_remap` | internal import/export rewriting |
//! | REF-ENT-006 | `sentinel_string` | `"self"` -> owning subscription id |
//!
//! The original expression text, its parsed form and its source are all kept so
//! a round-trip never silently converts everything to a single new id (plan
//! §11).

use serde::{Deserialize, Serialize};

/// Classification of how a reference is resolved, mirroring the ledger ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    /// REF-ENT-001 / REF-ENT-002: a Remarks string resolved by name.
    StringRemarks,
    /// REF-ENT-003: ordered list of Profile.IndexId.
    CommaSeparatedIndexIds,
    /// REF-ENT-004: subscription id list combined with a Remarks filter.
    SubscriptionIdPlusRegex,
    /// REF-ENT-005: internal import/export id remapping.
    IdRemap,
    /// REF-ENT-006: the `"self"` sentinel meaning the owning subscription.
    SentinelString,
}

impl ReferenceKind {
    pub const fn ledger_id(self) -> &'static str {
        match self {
            ReferenceKind::StringRemarks => "REF-ENT-001/002",
            ReferenceKind::CommaSeparatedIndexIds => "REF-ENT-003",
            ReferenceKind::SubscriptionIdPlusRegex => "REF-ENT-004",
            ReferenceKind::IdRemap => "REF-ENT-005",
            ReferenceKind::SentinelString => "REF-ENT-006",
        }
    }
}

/// Where a reference value came from, so dangling/renamed references remain
/// explainable after migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceSource {
    /// Parsed from an upstream guiNConfig.json / guiNDB.db row.
    Imported,
    /// Created in this application.
    Native,
    /// Rewritten by an internal import/export round-trip.
    Remapped,
}

/// Sentinel used in `SubChildItems` to mean "the subscription that owns this
/// group node" (REF-ENT-006).
pub const SELF_SENTINEL: &str = "self";

/// A parsed reference expression. `raw` always preserves the original text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceExpr {
    pub kind: ReferenceKind,
    /// The original, unmodified expression text.
    pub raw: String,
    /// Resolved targets. For `CommaSeparatedIndexIds` this is the id list; for
    /// `StringRemarks` the Remarks strings; for `SubscriptionIdPlusRegex` the
    /// subscription ids.
    pub targets: Vec<String>,
    /// Optional filter regex, only meaningful for
    /// `SubscriptionIdPlusRegex` (REF-ENT-004).
    pub filter: Option<String>,
    /// True when the raw text was exactly the `self` sentinel.
    pub is_self: bool,
    pub source: ReferenceSource,
}

impl ReferenceExpr {
    /// Parse a comma-separated id list (REF-ENT-003) or the `self` sentinel.
    pub fn parse_index_ids(raw: &str, source: ReferenceSource) -> Self {
        let trimmed = raw.trim();
        if trimmed.eq_ignore_ascii_case(SELF_SENTINEL) {
            return Self {
                kind: ReferenceKind::SentinelString,
                raw: raw.to_string(),
                targets: vec![SELF_SENTINEL.to_string()],
                filter: None,
                is_self: true,
                source,
            };
        }
        let targets = split_list(raw);
        Self {
            kind: ReferenceKind::CommaSeparatedIndexIds,
            raw: raw.to_string(),
            targets,
            filter: None,
            is_self: false,
            source,
        }
    }

    /// Build a subscription-id + regex reference (REF-ENT-004).
    pub fn subscription_with_filter(
        subscription_ids: &str,
        filter: Option<&str>,
        source: ReferenceSource,
    ) -> Self {
        let trimmed = subscription_ids.trim();
        if trimmed.eq_ignore_ascii_case(SELF_SENTINEL) {
            return Self {
                kind: ReferenceKind::SentinelString,
                raw: subscription_ids.to_string(),
                targets: vec![SELF_SENTINEL.to_string()],
                filter: filter.map(str::to_string),
                is_self: true,
                source,
            };
        }
        Self {
            kind: ReferenceKind::SubscriptionIdPlusRegex,
            raw: subscription_ids.to_string(),
            targets: split_list(subscription_ids),
            filter: filter.map(str::to_string),
            is_self: false,
            source,
        }
    }

    /// Build a Remarks-name reference (REF-ENT-001 / REF-ENT-002).
    pub fn remarks(raw: &str, source: ReferenceSource) -> Self {
        let targets = split_list(raw);
        Self {
            kind: ReferenceKind::StringRemarks,
            raw: raw.to_string(),
            targets,
            filter: None,
            is_self: false,
            source,
        }
    }

    /// Re-serialize back to the upstream text form. Guaranteed to return the
    /// original `raw` for unmodified imported expressions.
    pub fn to_raw(&self) -> String {
        self.raw.clone()
    }
}

/// Split a comma-separated list, trimming entries and dropping empties.
pub fn split_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_index_ids() {
        let expr = ReferenceExpr::parse_index_ids(" a, b ,,c ", ReferenceSource::Imported);
        assert_eq!(expr.kind, ReferenceKind::CommaSeparatedIndexIds);
        assert_eq!(expr.targets, vec!["a", "b", "c"]);
        assert_eq!(expr.to_raw(), " a, b ,,c ");
    }

    #[test]
    fn detects_self_sentinel() {
        let expr = ReferenceExpr::parse_index_ids("self", ReferenceSource::Imported);
        assert!(expr.is_self);
        assert_eq!(expr.kind, ReferenceKind::SentinelString);
    }

    #[test]
    fn subscription_filter_kept() {
        let expr = ReferenceExpr::subscription_with_filter(
            "sub-1,sub-2",
            Some("^HK"),
            ReferenceSource::Native,
        );
        assert_eq!(expr.targets, vec!["sub-1", "sub-2"]);
        assert_eq!(expr.filter.as_deref(), Some("^HK"));
    }
}
