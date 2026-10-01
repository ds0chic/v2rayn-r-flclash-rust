//! PolicyGroup / ProxyChain validation and child resolution (T10).
//!
//! Mirrors the upstream `GroupProfileManager` + `CoreConfigContextBuilder`
//! semantics:
//! - `ChildItems` is an ordered comma-separated `IndexId` list (REF-ENT-003).
//! - `SubChildItems` is a subscription-id list plus a Remarks filter regex
//!   (REF-ENT-004); the `self` sentinel resolves to the group's own `Subid`.
//! - Cycles across `ChildItems` are rejected (`E_GRAPH_CYCLE`).
//! - The filter regex must compile (`E_FIELD_FORMAT`).

use std::collections::{HashMap, HashSet};

use domain::{codes, ConfigType, DomainError, Profile, ReferenceSource};

use crate::repository::new_index_id;

/// True for the composite group kinds (`ConfigType.IsGroupType()`).
pub fn is_group(config_type: ConfigType) -> bool {
    matches!(
        config_type,
        ConfigType::PolicyGroup | ConfigType::ProxyChain
    )
}

/// A node can be a group child when it is a leaf or an `Outbound`; complex
/// group/custom nodes are filtered out (upstream `!IsComplexType() || Outbound`).
pub fn is_eligible_child(profile: &Profile) -> bool {
    !profile.config_type.is_complex() || profile.config_type == ConfigType::Outbound
}

/// Ordered `ChildItems` id list, without the `self` sentinel entry.
pub fn child_index_ids(profile: &Profile) -> Vec<String> {
    profile
        .child_items_ref()
        .map(|expr| {
            expr.targets
                .into_iter()
                .filter(|id| !id.eq_ignore_ascii_case("self"))
                .collect()
        })
        .unwrap_or_default()
}

/// Subscription ids from `SubChildItems`, resolving the `self` sentinel against
/// the owning profile's `subid`.
pub fn sub_child_ids(profile: &Profile) -> Vec<String> {
    profile
        .sub_child_items_ref()
        .map(|expr| {
            expr.targets
                .into_iter()
                .map(|id| {
                    if id.eq_ignore_ascii_case("self") {
                        profile.subid.clone()
                    } else {
                        id
                    }
                })
                .filter(|id| !id.trim().is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Compile the Remarks filter regex, if one is configured.
fn compile_filter(profile: &Profile) -> Result<Option<regex::Regex>, DomainError> {
    let Some(filter) = profile.proto_extra.filter.as_deref() else {
        return Ok(None);
    };
    if filter.trim().is_empty() {
        return Ok(None);
    }
    regex::Regex::new(filter).map(Some).map_err(|error| {
        DomainError::new(codes::FIELD_FORMAT, "error.group_filter_invalid")
            .with_field("proto_extra.filter")
            .with_detail(error.to_string())
    })
}

/// Validate a group/chain draft against the current profile set. Leaves and
/// non-group kinds pass through unchanged.
pub fn validate_group(
    profile: &Profile,
    all: &HashMap<String, Profile>,
) -> Result<(), DomainError> {
    if !is_group(profile.config_type) {
        return Ok(());
    }
    if profile.remarks.trim().is_empty() {
        return Err(
            DomainError::new(codes::FIELD_REQUIRED, "error.remarks_required").with_field("remarks"),
        );
    }
    compile_filter(profile)?;

    let child_ids = child_index_ids(profile);
    let sub_ids = sub_child_ids(profile);
    if child_ids.is_empty() && sub_ids.is_empty() {
        return Err(DomainError::new(
            codes::FIELD_REQUIRED,
            "error.group_children_required",
        )
        .with_field("proto_extra.childItems"));
    }
    for id in &child_ids {
        if !all.contains_key(id) {
            return Err(
                DomainError::new(codes::DANGLING_REFERENCE, "error.group_child_missing")
                    .with_field("proto_extra.childItems")
                    .with_detail(format!("child `{id}` not found")),
            );
        }
    }
    if has_cycle(profile, all) {
        return Err(DomainError::new(codes::GRAPH_CYCLE, "error.group_cycle")
            .with_field("proto_extra.childItems")
            .with_detail(format!("cycle through group `{}`", profile.remarks)));
    }
    Ok(())
}

/// Depth-first cycle check over `ChildItems` (upstream `HasCycle`).
pub fn has_cycle(profile: &Profile, all: &HashMap<String, Profile>) -> bool {
    let mut visited = HashSet::new();
    let mut stack = HashSet::new();
    has_cycle_inner(&profile.index_id, all, &mut visited, &mut stack)
}

fn has_cycle_inner(
    index_id: &str,
    all: &HashMap<String, Profile>,
    visited: &mut HashSet<String>,
    stack: &mut HashSet<String>,
) -> bool {
    if index_id.is_empty() {
        return false;
    }
    if stack.contains(index_id) {
        return true;
    }
    if visited.contains(index_id) {
        return false;
    }
    visited.insert(index_id.to_string());
    stack.insert(index_id.to_string());
    if let Some(node) = all.get(index_id) {
        for child in child_index_ids(node) {
            if has_cycle_inner(&child, all, visited, stack) {
                return true;
            }
        }
    }
    stack.remove(index_id);
    false
}

/// Resolve the ordered child list of a group/chain: subscription matches first,
/// then explicitly selected children (upstream order in
/// `GetChildProfileItemsByProtocolExtra`).
pub fn resolve_children<'a>(
    profile: &Profile,
    all: &'a HashMap<String, Profile>,
) -> Vec<&'a Profile> {
    let mut items: Vec<&Profile> = Vec::new();
    items.extend(resolve_sub_children(profile, all));
    for id in child_index_ids(profile) {
        if let Some(child) = all.get(&id) {
            if !items.iter().any(|p| p.index_id == child.index_id) {
                items.push(child);
            }
        }
    }
    items
}

/// Profiles belonging to the `SubChildItems` subscriptions whose Remarks match
/// the filter regex, in stable index order.
pub fn resolve_sub_children<'a>(
    profile: &Profile,
    all: &'a HashMap<String, Profile>,
) -> Vec<&'a Profile> {
    let sub_ids = sub_child_ids(profile);
    if sub_ids.is_empty() {
        return Vec::new();
    }
    let filter = match compile_filter(profile) {
        Ok(filter) => filter,
        Err(_) => None,
    };
    let mut matched: Vec<&Profile> = all
        .values()
        .filter(|p| sub_ids.contains(&p.subid) && is_eligible_child(p))
        .filter(|p| match &filter {
            Some(re) => re.is_match(&p.remarks),
            None => true,
        })
        .collect();
    matched.sort_by(|a, b| a.index_id.cmp(&b.index_id));
    matched
}

/// Normalize a group draft before save: fill `GroupType` from `ConfigType`, the
/// default `MultipleLoad`, and de-duplicate / trim the `ChildItems` list.
pub fn normalize_group(mut profile: Profile) -> Profile {
    if !is_group(profile.config_type) {
        return profile;
    }
    let group_type = profile.config_type.as_str().to_string();
    let mut seen = HashSet::new();
    let ids: Vec<String> = child_index_ids(&profile)
        .into_iter()
        .filter(|id| seen.insert(id.clone()))
        .collect();
    profile.proto_extra.group_type = Some(group_type);
    profile.proto_extra.child_items = if ids.is_empty() {
        None
    } else {
        Some(ids.join(","))
    };
    if profile.proto_extra.multiple_load.is_none() {
        profile.proto_extra.multiple_load = Some(domain::MultipleLoad::LeastPing);
    }
    profile
}

/// Build a fresh group draft with a stable id and the given load strategy.
pub fn new_group(
    config_type: ConfigType,
    remarks: String,
    multiple_load: domain::MultipleLoad,
) -> Profile {
    let mut profile = Profile {
        index_id: new_index_id(),
        config_type,
        is_sub: false,
        remarks,
        ..Default::default()
    };
    profile.proto_extra.group_type = Some(config_type.as_str().to_string());
    profile.proto_extra.multiple_load = Some(multiple_load);
    profile
}

/// Reconstruct a reference expression from an explicit id list (test helper).
pub fn set_child_index_ids(profile: &mut Profile, ids: &[String]) {
    profile.proto_extra.child_items = if ids.is_empty() {
        None
    } else {
        Some(ids.join(","))
    };
    let _ = ReferenceSource::Native;
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::MultipleLoad;

    fn leaf(id: &str, subid: &str, remarks: &str) -> Profile {
        Profile {
            index_id: id.into(),
            config_type: ConfigType::Vless,
            subid: subid.into(),
            remarks: remarks.into(),
            address: "192.0.2.1".into(),
            port: 443,
            ..Default::default()
        }
    }

    fn map(profiles: Vec<Profile>) -> HashMap<String, Profile> {
        profiles.into_iter().map(|p| (p.index_id.clone(), p)).collect()
    }

    #[test]
    fn rejects_missing_child() {
        let mut group = new_group(
            ConfigType::PolicyGroup,
            "g".into(),
            MultipleLoad::LeastPing,
        );
        set_child_index_ids(&mut group, &["missing".into()]);
        let err = validate_group(&group, &HashMap::new()).unwrap_err();
        assert_eq!(err.code, codes::DANGLING_REFERENCE);
    }

    #[test]
    fn rejects_cycle() {
        let mut a = new_group(ConfigType::PolicyGroup, "a".into(), MultipleLoad::LeastPing);
        a.index_id = "a".into();
        let mut b = new_group(ConfigType::PolicyGroup, "b".into(), MultipleLoad::LeastPing);
        b.index_id = "b".into();
        set_child_index_ids(&mut a, &["b".into()]);
        set_child_index_ids(&mut b, &["a".into()]);
        let all = map(vec![a.clone(), b.clone()]);
        let err = validate_group(&a, &all).unwrap_err();
        assert_eq!(err.code, codes::GRAPH_CYCLE);
        assert!(has_cycle(&a, &all));
    }

    #[test]
    fn rejects_invalid_filter_regex() {
        let mut group = new_group(
            ConfigType::PolicyGroup,
            "g".into(),
            MultipleLoad::LeastPing,
        );
        group.proto_extra.sub_child_items = Some("sub-1".into());
        group.proto_extra.filter = Some("(".into());
        let err = validate_group(&group, &HashMap::new()).unwrap_err();
        assert_eq!(err.code, codes::FIELD_FORMAT);
    }

    #[test]
    fn normalizes_child_order_and_group_type() {
        let mut group = new_group(
            ConfigType::ProxyChain,
            "chain".into(),
            MultipleLoad::RoundRobin,
        );
        group.proto_extra.child_items = Some(" c2 , c1 ,c2".into());
        let normalized = normalize_group(group);
        assert_eq!(
            normalized.proto_extra.child_items.as_deref(),
            Some("c2,c1")
        );
        assert_eq!(
            normalized.proto_extra.group_type.as_deref(),
            Some("ProxyChain")
        );
    }

    #[test]
    fn sub_children_filter_and_order() {
        let mut group = new_group(
            ConfigType::PolicyGroup,
            "g".into(),
            MultipleLoad::LeastPing,
        );
        group.index_id = "g".into();
        group.subid = "sub-1".into();
        group.proto_extra.sub_child_items = Some("self".into());
        group.proto_extra.filter = Some("^HK".into());
        let all = map(vec![
            leaf("b", "sub-1", "HK-2"),
            leaf("a", "sub-1", "HK-1"),
            leaf("c", "sub-1", "US-1"),
            leaf("d", "sub-2", "HK-3"),
        ]);
        let resolved: Vec<&str> = resolve_children(&group, &all)
            .iter()
            .map(|p| p.index_id.as_str())
            .collect();
        assert_eq!(resolved, vec!["a", "b"]);
    }
}
