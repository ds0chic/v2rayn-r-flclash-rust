//! Reference resolution (REF-ENT-001..006).
//!
//! Upstream references are not all foreign keys; routing outbound tags and
//! subscription prev/next nodes resolve by **Remarks**, group children by
//! **IndexId list**, and dynamic subscription children by **Sub id + filter
//! regex**. These functions are pure and independently testable, and they
//! degrade to warnings instead of aborting an import.

use domain::{ConfigType, Profile, ReferenceExpr, ReferenceSource, Subscription, SELF_SENTINEL};
use regex::Regex;

/// Result of resolving a Remarks-based reference (REF-ENT-001/002).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RemarksResolution {
    /// Matching profile index ids, in profile order.
    pub resolved_ids: Vec<String>,
    /// Remarks values with no matching profile.
    pub missing_remarks: Vec<String>,
}

impl RemarksResolution {
    pub fn is_resolved(&self) -> bool {
        !self.resolved_ids.is_empty()
    }
}

/// Resolve a comma-separated Remarks expression against known profiles.
pub fn resolve_remarks(profiles: &[Profile], raw: &str) -> RemarksResolution {
    let expr = ReferenceExpr::remarks(raw, ReferenceSource::Imported);
    resolve_remarks_expr(profiles, &expr)
}

/// Resolve a pre-parsed Remarks expression. First match wins for each target
/// while all matches are reported in `resolved_ids`.
pub fn resolve_remarks_expr(profiles: &[Profile], expr: &ReferenceExpr) -> RemarksResolution {
    let mut resolved_ids = Vec::new();
    let mut missing_remarks = Vec::new();
    for target in &expr.targets {
        let mut matched = false;
        for profile in profiles {
            if profile.remarks == *target {
                matched = true;
                if !resolved_ids.contains(&profile.index_id) {
                    resolved_ids.push(profile.index_id.clone());
                }
            }
        }
        if !matched && !missing_remarks.contains(target) {
            missing_remarks.push(target.clone());
        }
    }
    RemarksResolution {
        resolved_ids,
        missing_remarks,
    }
}

/// Result of resolving a group `ChildItems` list (REF-ENT-003).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChildResolution {
    /// Child index ids in the original order, de-duplicated.
    pub ordered_ids: Vec<String>,
    /// Referenced ids that do not exist.
    pub missing_ids: Vec<String>,
    /// True when the raw value was the `self` sentinel (REF-ENT-006).
    pub is_self: bool,
}

pub fn resolve_child_items(profiles: &[Profile], raw: &str) -> ChildResolution {
    let expr = ReferenceExpr::parse_index_ids(raw, ReferenceSource::Imported);
    if expr.is_self {
        return ChildResolution {
            is_self: true,
            ..Default::default()
        };
    }
    let known: std::collections::HashSet<&str> =
        profiles.iter().map(|p| p.index_id.as_str()).collect();
    let mut ordered_ids = Vec::new();
    let mut missing_ids = Vec::new();
    for target in &expr.targets {
        if known.contains(target.as_str()) {
            if !ordered_ids.contains(target) {
                ordered_ids.push(target.clone());
            }
        } else if !missing_ids.contains(target) {
            missing_ids.push(target.clone());
        }
    }
    ChildResolution {
        ordered_ids,
        missing_ids,
        is_self: expr.is_self,
    }
}

/// Result of resolving a dynamic `SubChildItems` reference (REF-ENT-004).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubChildResolution {
    /// Matching profile index ids, in profile order.
    pub matched_ids: Vec<String>,
    /// Subscription ids that do not exist.
    pub missing_subscriptions: Vec<String>,
    /// The filter expression carried by the reference.
    pub filter: Option<String>,
    /// Set when the filter failed to compile; matching then yields nothing.
    pub invalid_filter: Option<String>,
    pub is_self: bool,
    /// `self` was used but no owning subscription was supplied.
    pub self_unresolved: bool,
}

/// Resolve `SubChildItems` against subscriptions and profiles. `owner_subid`
/// supplies the owning subscription for the `self` sentinel (REF-ENT-006).
pub fn resolve_sub_child_items(
    profiles: &[Profile],
    subscriptions: &[Subscription],
    raw: &str,
    filter: Option<&str>,
    owner_subid: Option<&str>,
) -> SubChildResolution {
    let expr = ReferenceExpr::subscription_with_filter(raw, filter, ReferenceSource::Imported);
    let filter = expr.filter.clone();
    let (targets, is_self, self_unresolved) = if expr.is_self {
        match owner_subid {
            Some(id) if !id.is_empty() => (vec![id.to_string()], true, false),
            _ => (Vec::new(), true, true),
        }
    } else {
        (expr.targets.clone(), false, false)
    };

    let matcher = match filter.as_deref() {
        Some(pattern) if !pattern.is_empty() => match Regex::new(pattern) {
            Ok(regex) => Some(regex),
            Err(err) => {
                return SubChildResolution {
                    filter,
                    invalid_filter: Some(err.to_string()),
                    is_self,
                    self_unresolved,
                    ..Default::default()
                }
            }
        },
        _ => None,
    };

    let mut missing_subscriptions = Vec::new();
    let mut valid_targets = Vec::new();
    for target in &targets {
        if subscriptions.iter().any(|s| s.id == *target) {
            if !valid_targets.contains(target) {
                valid_targets.push(target.clone());
            }
        } else if !missing_subscriptions.contains(target) {
            missing_subscriptions.push(target.clone());
        }
    }

    let mut matched_ids = Vec::new();
    for profile in profiles {
        if !valid_targets.contains(&profile.subid) {
            continue;
        }
        // Dynamic sources only include leaf nodes plus Outbound (REF-ENT-004).
        if profile.config_type.is_complex() && profile.config_type != ConfigType::Outbound {
            continue;
        }
        if let Some(regex) = &matcher {
            if !regex.is_match(&profile.remarks) {
                continue;
            }
        }
        if !matched_ids.contains(&profile.index_id) {
            matched_ids.push(profile.index_id.clone());
        }
    }

    SubChildResolution {
        matched_ids,
        missing_subscriptions,
        filter,
        invalid_filter: None,
        is_self,
        self_unresolved,
    }
}

/// The `self` sentinel literal, surfaced for the UI/import layer.
pub const SELF: &str = SELF_SENTINEL;

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ConfigType;

    fn profile(id: &str, remarks: &str, subid: &str, ty: ConfigType) -> Profile {
        Profile {
            index_id: id.into(),
            remarks: remarks.into(),
            subid: subid.into(),
            config_type: ty,
            ..Default::default()
        }
    }

    fn subscription(id: &str) -> Subscription {
        Subscription {
            id: id.into(),
            ..Default::default()
        }
    }

    #[test]
    fn remarks_resolution_finds_and_reports_missing() {
        let profiles = vec![
            profile("p1", "HK-1", "s1", ConfigType::Vless),
            profile("p2", "US-1", "s1", ConfigType::Vless),
        ];
        let ok = resolve_remarks(&profiles, "US-1");
        assert_eq!(ok.resolved_ids, vec!["p2"]);
        assert!(ok.missing_remarks.is_empty());

        let missing = resolve_remarks(&profiles, "NOPE");
        assert!(missing.resolved_ids.is_empty());
        assert_eq!(missing.missing_remarks, vec!["NOPE"]);
    }

    #[test]
    fn child_items_preserve_order_and_dedupe() {
        let profiles = vec![
            profile("a", "A", "", ConfigType::Vless),
            profile("b", "B", "", ConfigType::Vless),
            profile("c", "C", "", ConfigType::Vless),
        ];
        let res = resolve_child_items(&profiles, " c, a , c , zz ");
        assert_eq!(res.ordered_ids, vec!["c", "a"]);
        assert_eq!(res.missing_ids, vec!["zz"]);
        assert!(!res.is_self);
    }

    #[test]
    fn self_sentinel_is_detected_and_not_treated_as_id() {
        let profiles = vec![profile("a", "A", "", ConfigType::Vless)];
        let res = resolve_child_items(&profiles, "self");
        assert!(res.is_self);
        assert!(res.ordered_ids.is_empty());
        assert!(res.missing_ids.is_empty());
    }

    #[test]
    fn sub_child_items_match_regex_and_skip_children_groups() {
        let profiles = vec![
            profile("p1", "HK-01", "s1", ConfigType::Vless),
            profile("p2", "US-01", "s1", ConfigType::Vless),
            profile("g1", "HK-group", "s1", ConfigType::PolicyGroup),
        ];
        let subs = vec![subscription("s1")];
        let res = resolve_sub_child_items(&profiles, &subs, "s1", Some("^HK"), None);
        assert_eq!(res.matched_ids, vec!["p1"]);
        assert!(res.missing_subscriptions.is_empty());
    }

    #[test]
    fn self_reference_uses_owner_subscription_and_flags_missing() {
        let profiles = vec![profile("p1", "HK-01", "s9", ConfigType::Vless)];
        let subs = vec![subscription("s9")];
        let res = resolve_sub_child_items(&profiles, &subs, "self", None, Some("s9"));
        assert!(res.is_self);
        assert_eq!(res.matched_ids, vec!["p1"]);

        let unresolved = resolve_sub_child_items(&profiles, &subs, "self", None, None);
        assert!(unresolved.self_unresolved);
        assert!(unresolved.matched_ids.is_empty());
    }

    #[test]
    fn invalid_regex_degrades_without_panicking() {
        let profiles = vec![profile("p1", "HK", "s1", ConfigType::Vless)];
        let subs = vec![subscription("s1")];
        let res = resolve_sub_child_items(&profiles, &subs, "s1", Some("("), None);
        assert!(res.invalid_filter.is_some());
        assert!(res.matched_ids.is_empty());
    }

    #[test]
    fn missing_subscription_is_reported_not_fatal() {
        let profiles = vec![profile("p1", "HK", "ghost", ConfigType::Vless)];
        let subs = vec![subscription("s1")];
        let res = resolve_sub_child_items(&profiles, &subs, "ghost", None, None);
        assert_eq!(res.missing_subscriptions, vec!["ghost"]);
        assert!(res.matched_ids.is_empty());
    }
}
