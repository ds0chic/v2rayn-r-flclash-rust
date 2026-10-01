//! Candidate validation (plan §11 step 4).
//!
//! Counts and primary-key integrity are **fatal**: a candidate that loses or
//! duplicates rows must never be committed. Field, reference, active-node and
//! path problems are **warnings** — upstream configs legitimately contain
//! dangling references and stale ids, and the import must still preserve the
//! data (plan §11: "失效降级告警不中断").

use std::collections::{HashMap, HashSet};

use domain::{DnsProfile, FullConfigTemplate, Profile, RoutingProfile, Subscription};
use serde_json::Value;

use crate::mapping::ProfileExRow;
use crate::references::{resolve_child_items, resolve_remarks, resolve_sub_child_items};
use crate::report::{EntityCount, ReportIssue, ValidationSummary};

/// Warnings carry stable codes.
pub mod warn {
    pub const FIELD: &str = "W_FIELD";
    pub const REF_OUTBOUND: &str = "W_REF_OUTBOUND";
    pub const REF_SUB_CHAIN: &str = "W_REF_SUB_CHAIN";
    pub const REF_CHILD: &str = "W_REF_CHILD";
    pub const REF_SUBCHILD: &str = "W_REF_SUBCHILD";
    pub const ACTIVE_NODE: &str = "W_ACTIVE_NODE";
    pub const PATH: &str = "W_PATH";
    pub const GROUP_CYCLE: &str = "W_GROUP_CYCLE";
}

/// Fatal validation codes.
pub mod err {
    pub const COUNT: &str = "E_VALIDATE_COUNT";
    pub const DUPLICATE_ID: &str = "E_VALIDATE_DUPLICATE_ID";
    pub const MISSING_ID: &str = "E_VALIDATE_MISSING_ID";
}

/// Everything validation needs, borrowed from the candidate.
#[derive(Default)]
pub struct CandidateView<'a> {
    pub profiles: &'a [Profile],
    pub subscriptions: &'a [Subscription],
    pub routing: &'a [RoutingProfile],
    pub dns: &'a [DnsProfile],
    pub templates: &'a [FullConfigTemplate],
    pub profile_ex: &'a [ProfileExRow],
    pub expected_counts: &'a [EntityCount],
    pub active_profile_id: Option<&'a str>,
    pub active_sub_id: Option<&'a str>,
    pub resource_paths: &'a [String],
}

/// Validation result: a summary plus the warning/error entries.
#[derive(Debug, Clone, Default)]
pub struct ValidationOutcome {
    pub summary: ValidationSummary,
    pub warnings: Vec<ReportIssue>,
    pub errors: Vec<ReportIssue>,
}

impl ValidationOutcome {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Run every validation check. `traffic` is accepted for signature stability
/// even though no traffic-specific rule is defined yet.
pub fn validate_candidate(view: &CandidateView<'_>) -> ValidationOutcome {
    let mut outcome = ValidationOutcome::default();

    check_counts(view, &mut outcome);
    check_unique_ids(view, &mut outcome);
    check_profile_fields(view, &mut outcome);
    check_references(view, &mut outcome);
    check_active_nodes(view, &mut outcome);
    check_paths(view, &mut outcome);
    check_group_cycles(view, &mut outcome);

    outcome.summary.unknown_fields_preserved = count_unknown_fields(view);
    outcome
}

fn check_counts(view: &CandidateView<'_>, outcome: &mut ValidationOutcome) {
    for count in view.expected_counts {
        outcome.summary.checks_run += 1;
        let accounted = count.imported_rows + count.skipped_rows;
        if accounted != count.source_rows {
            outcome.summary.checks_failed += 1;
            outcome.errors.push(
                ReportIssue::new(
                    err::COUNT,
                    format!(
                        "{}: source {} != imported {} + skipped {}",
                        count.table, count.source_rows, count.imported_rows, count.skipped_rows
                    ),
                )
                .with_entity(count.table.clone(), ""),
            );
        }
    }
}

fn check_unique_ids(view: &CandidateView<'_>, outcome: &mut ValidationOutcome) {
    outcome.summary.checks_run += 1;
    let mut seen: HashSet<&str> = HashSet::new();
    let mut failed = false;
    for profile in view.profiles {
        if profile.index_id.trim().is_empty() {
            failed = true;
            outcome.errors.push(
                ReportIssue::new(err::MISSING_ID, "profile with empty IndexId")
                    .with_entity("ProfileItem", ""),
            );
            continue;
        }
        if !seen.insert(&profile.index_id) {
            failed = true;
            outcome.errors.push(
                ReportIssue::new(err::DUPLICATE_ID, "duplicate profile IndexId")
                    .with_entity("ProfileItem", &profile.index_id),
            );
        }
    }
    if failed {
        outcome.summary.checks_failed += 1;
    }
}

fn check_profile_fields(view: &CandidateView<'_>, outcome: &mut ValidationOutcome) {
    for profile in view.profiles {
        outcome.summary.checks_run += 1;
        if let Err(error) = profile.validate() {
            outcome.warnings.push(
                ReportIssue::new(warn::FIELD, error.to_string())
                    .with_entity("ProfileItem", &profile.index_id),
            );
        }
    }
}

fn check_references(view: &CandidateView<'_>, outcome: &mut ValidationOutcome) {
    // Routing outbound tags (REF-ENT-001).
    for routing in view.routing {
        let rules = match crate::mapping::parse_rules(&routing.rule_set) {
            Ok(rules) => rules,
            Err(error) => {
                outcome.warnings.push(
                    ReportIssue::new(warn::REF_OUTBOUND, format!("RuleSet parse error: {error}"))
                        .with_entity("RoutingItem", &routing.id),
                );
                continue;
            }
        };
        for rule in rules {
            let Some(raw) = rule.outbound_tag.as_deref() else {
                continue;
            };
            if raw.is_empty() || is_builtin_tag(raw) {
                continue;
            }
            outcome.summary.checks_run += 1;
            let resolution = resolve_remarks(view.profiles, raw);
            if !resolution.is_resolved() {
                outcome.summary.reference_warnings += 1;
                outcome.warnings.push(
                    ReportIssue::new(
                        warn::REF_OUTBOUND,
                        format!("outbound rule references missing Remarks `{raw}`"),
                    )
                    .with_entity("RulesItem", &rule.id),
                );
            }
        }
    }

    // Subscription prev/next chain by Remarks (REF-ENT-002).
    for subscription in view.subscriptions {
        for raw in [&subscription.prev_profile, &subscription.next_profile]
            .into_iter()
            .flatten()
        {
            if raw.trim().is_empty() {
                continue;
            }
            outcome.summary.checks_run += 1;
            let resolution = resolve_remarks(view.profiles, raw);
            if !resolution.is_resolved() {
                outcome.summary.reference_warnings += 1;
                outcome.warnings.push(
                    ReportIssue::new(
                        warn::REF_SUB_CHAIN,
                        format!("subscription chain references missing Remarks `{raw}`"),
                    )
                    .with_entity("SubItem", &subscription.id),
                );
            }
        }
    }

    // Group children (REF-ENT-003) and dynamic children (REF-ENT-004/006).
    for profile in view.profiles {
        let Some(child_items) = profile.proto_extra.child_items.as_deref() else {
            continue;
        };
        if child_items.trim().is_empty() {
            continue;
        }
        outcome.summary.checks_run += 1;
        let child = resolve_child_items(view.profiles, child_items);
        for missing in &child.missing_ids {
            outcome.summary.reference_warnings += 1;
            outcome.warnings.push(
                ReportIssue::new(
                    warn::REF_CHILD,
                    format!("group child `{missing}` not found"),
                )
                .with_entity("ProfileItem", &profile.index_id),
            );
        }
        let Some(sub_raw) = profile.proto_extra.sub_child_items.as_deref() else {
            continue;
        };
        if sub_raw.trim().is_empty() {
            continue;
        }
        outcome.summary.checks_run += 1;
        let owner = if profile.subid.is_empty() {
            None
        } else {
            Some(profile.subid.as_str())
        };
        let sub = resolve_sub_child_items(
            view.profiles,
            view.subscriptions,
            sub_raw,
            profile.proto_extra.filter.as_deref(),
            owner,
        );
        for missing in &sub.missing_subscriptions {
            outcome.summary.reference_warnings += 1;
            outcome.warnings.push(
                ReportIssue::new(
                    warn::REF_SUBCHILD,
                    format!("group sub-child subscription `{missing}` not found"),
                )
                .with_entity("ProfileItem", &profile.index_id),
            );
        }
        if sub.self_unresolved {
            outcome.summary.reference_warnings += 1;
            outcome.warnings.push(
                ReportIssue::new(
                    warn::REF_SUBCHILD,
                    "group sub-child uses `self` without an owning subscription",
                )
                .with_entity("ProfileItem", &profile.index_id),
            );
        }
        if let Some(invalid) = &sub.invalid_filter {
            outcome.summary.reference_warnings += 1;
            outcome.warnings.push(
                ReportIssue::new(
                    warn::REF_SUBCHILD,
                    format!("group sub-child filter is not a valid regex: {invalid}"),
                )
                .with_entity("ProfileItem", &profile.index_id),
            );
        }
    }
}

fn is_builtin_tag(tag: &str) -> bool {
    matches!(tag, "proxy" | "direct" | "block" | "dns")
}

fn check_active_nodes(view: &CandidateView<'_>, outcome: &mut ValidationOutcome) {
    if let Some(active) = view.active_profile_id {
        if !active.is_empty() {
            outcome.summary.checks_run += 1;
            if !view.profiles.iter().any(|p| p.index_id == active) {
                outcome.summary.reference_warnings += 1;
                outcome.warnings.push(
                    ReportIssue::new(warn::ACTIVE_NODE, "active profile id not found")
                        .with_entity("Config.IndexId", active),
                );
            }
        }
    }
    if let Some(active) = view.active_sub_id {
        if !active.is_empty() {
            outcome.summary.checks_run += 1;
            if !view.subscriptions.iter().any(|s| s.id == active) {
                outcome.summary.reference_warnings += 1;
                outcome.warnings.push(
                    ReportIssue::new(warn::ACTIVE_NODE, "active subscription id not found")
                        .with_entity("Config.SubIndexId", active),
                );
            }
        }
    }
}

fn check_paths(view: &CandidateView<'_>, outcome: &mut ValidationOutcome) {
    for path in view.resource_paths {
        outcome.summary.checks_run += 1;
        if is_portable_relative(path) {
            continue;
        }
        outcome.warnings.push(ReportIssue::new(
            warn::PATH,
            format!("resource path is not portable/relative: {path}"),
        ));
    }
}

/// Absolute paths and parent traversal are rejected for portability
/// (plan §11): a record must not hard-code another machine's directory.
fn is_portable_relative(path: &str) -> bool {
    if path.trim().is_empty() {
        return true;
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return false;
    }
    // Windows drive letter.
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' {
        return false;
    }
    !path.split(['/', '\\']).any(|segment| segment == "..")
}

fn check_group_cycles(view: &CandidateView<'_>, outcome: &mut ValidationOutcome) {
    let ids: HashSet<String> = view.profiles.iter().map(|p| p.index_id.clone()).collect();
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
    for profile in view.profiles {
        let Some(children) = profile.proto_extra.child_items.as_deref() else {
            continue;
        };
        let resolved = resolve_child_items(view.profiles, children);
        let edges: Vec<String> = resolved
            .ordered_ids
            .into_iter()
            .filter(|id| ids.contains(id))
            .collect();
        adjacency.insert(profile.index_id.clone(), edges);
    }
    outcome.summary.checks_run += 1;
    let mut visiting: HashSet<String> = HashSet::new();
    let mut done: HashSet<String> = HashSet::new();
    for id in adjacency.keys().cloned().collect::<Vec<_>>() {
        if detect_cycle(&id, &adjacency, &mut visiting, &mut done) {
            outcome.warnings.push(
                ReportIssue::new(warn::GROUP_CYCLE, "group ChildItems cycle detected")
                    .with_entity("ProfileItem", &id),
            );
        }
    }
}

fn detect_cycle(
    node: &str,
    adjacency: &HashMap<String, Vec<String>>,
    visiting: &mut HashSet<String>,
    done: &mut HashSet<String>,
) -> bool {
    if done.contains(node) {
        return false;
    }
    if !visiting.insert(node.to_string()) {
        return true;
    }
    if let Some(children) = adjacency.get(node) {
        for child in children {
            if detect_cycle(child, adjacency, visiting, done) {
                return true;
            }
        }
    }
    visiting.remove(node);
    done.insert(node.to_string());
    false
}

fn count_unknown_fields(view: &CandidateView<'_>) -> u32 {
    fn count(value: &Value) -> u32 {
        match value {
            Value::Object(map) => map.values().map(count).sum(),
            Value::Array(items) => items.iter().map(count).sum(),
            _ => 0,
        }
    }
    let mut total = 0u32;
    for profile in view.profiles {
        total = total.saturating_add(count(&Value::Object(profile.extra.clone())));
        total = total.saturating_add(count(&Value::Object(profile.proto_extra.extra.clone())));
        total = total.saturating_add(count(&Value::Object(profile.transport_extra.extra.clone())));
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{ConfigType, ProtocolExtra};

    fn profile(id: &str, remarks: &str) -> Profile {
        Profile {
            index_id: id.into(),
            remarks: remarks.into(),
            config_type: ConfigType::Vless,
            address: "192.0.2.1".into(),
            port: 443,
            ..Default::default()
        }
    }

    #[test]
    fn count_mismatch_is_fatal() {
        let profiles = vec![profile("a", "A")];
        let counts = vec![EntityCount {
            table: "ProfileItem".into(),
            source_rows: 2,
            imported_rows: 1,
            migrated_rows: 0,
            skipped_rows: 0,
        }];
        let view = CandidateView {
            profiles: &profiles,
            expected_counts: &counts,
            ..Default::default()
        };
        let outcome = validate_candidate(&view);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.summary.checks_failed, 1);
    }

    #[test]
    fn duplicate_id_is_fatal() {
        let profiles = vec![profile("dup", "A"), profile("dup", "B")];
        let view = CandidateView {
            profiles: &profiles,
            ..Default::default()
        };
        let outcome = validate_candidate(&view);
        assert!(!outcome.is_ok());
    }

    #[test]
    fn dangling_outbound_reference_is_only_a_warning() {
        let profiles = vec![profile("a", "A")];
        let rule = domain::RoutingRule {
            id: "r1".into(),
            outbound_tag: Some("MISSING".into()),
            ..Default::default()
        };
        let routing = vec![RoutingProfile {
            id: "route".into(),
            rule_set: serde_json::to_string(&[serde_json::json!({
                "Id": "r1", "OutboundTag": "MISSING", "Enabled": true, "RuleType": 1
            })])
            .unwrap(),
            ..Default::default()
        }];
        let _ = rule;
        let view = CandidateView {
            profiles: &profiles,
            routing: &routing,
            ..Default::default()
        };
        let outcome = validate_candidate(&view);
        assert!(outcome.is_ok());
        assert!(outcome
            .warnings
            .iter()
            .any(|w| w.code == warn::REF_OUTBOUND));
    }

    #[test]
    fn group_cycle_is_flagged_as_warning() {
        let mut a = profile("a", "A");
        let mut b = profile("b", "B");
        a.proto_extra = ProtocolExtra {
            child_items: Some("b".into()),
            ..Default::default()
        };
        b.proto_extra = ProtocolExtra {
            child_items: Some("a".into()),
            ..Default::default()
        };
        let profiles = vec![a, b];
        let view = CandidateView {
            profiles: &profiles,
            ..Default::default()
        };
        let outcome = validate_candidate(&view);
        assert!(outcome.is_ok());
        assert!(outcome.warnings.iter().any(|w| w.code == warn::GROUP_CYCLE));
    }

    #[test]
    fn absolute_resource_path_is_warned() {
        let paths = vec![
            "C:\\Users\\x\\geo.dat".to_string(),
            "geo/geosite.dat".to_string(),
        ];
        let view = CandidateView {
            resource_paths: &paths,
            ..Default::default()
        };
        let outcome = validate_candidate(&view);
        assert!(outcome.warnings.iter().any(|w| w.code == warn::PATH));
        assert_eq!(
            outcome
                .warnings
                .iter()
                .filter(|w| w.code == warn::PATH)
                .count(),
            1
        );
    }
}
