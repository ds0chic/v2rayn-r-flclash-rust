//! Canonical node-identity and startup-selection rules (SP-03).
//!
//! Upstream reference (frozen `7d6a967`):
//!
//! - `Config.IndexId` is the persisted default node (written only by
//!   `ConfigHandler.SetDefaultServerIndex`, i.e. an explicit "set as default"
//!   choice); `Config.SubIndexId` is the persisted current group. Clicking a
//!   table row only changes the in-memory `SelectedProfile` and never rewrites
//!   the default.
//! - `ProfilesViewModel.RefreshServersBiz` (361–375) resolves the visible
//!   selection as: in-memory `_pendingSelectIndexId` match, else the persisted
//!   `IndexId` match, else the first visible row.
//! - `ConfigHandler.SetDefaultServer` repairs a dangling default to the first
//!   row with `Port > 0` and persists it; with an empty list it leaves the
//!   config untouched.
//!
//! This module holds the pure (persistence-free) half of that contract. The
//! engine owns the mirror key `active_index_id` next to the canonical
//! `IndexId`; both are always written together (§5.2), so the precedence below
//! only matters when opening a legacy or pre-SP-03 drifted file:
//!
//! 1. the mirror (`active_index_id`, the most recent explicit default choice)
//!    when it names an existing row;
//! 2. the canonical `IndexId` when it names an existing row;
//! 3. otherwise the first visible row (upstream repair), or nothing when the
//!    database has no rows (upstream leaves the config untouched then).

/// Empty and whitespace-only ids behave as absent everywhere in this module.
pub fn present(id: Option<&str>) -> Option<&str> {
    id.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

/// Visible-table selection without persisting anything: a temporary in-memory
/// pending id (e.g. a just-saved node, or a transient table click target)
/// wins, then the persisted default, then the first visible row. Mirrors
/// `RefreshServersBiz` 366–375. A temporary selection of C never rewrites the
/// persisted default B.
pub fn resolve_visible_selection(
    visible_ids: &[String],
    pending: Option<&str>,
    default_id: Option<&str>,
) -> Option<String> {
    if visible_ids.is_empty() {
        return None;
    }
    if let Some(wanted) = present(pending) {
        if visible_ids.iter().any(|id| id == wanted) {
            return Some(wanted.to_string());
        }
    }
    if let Some(wanted) = present(default_id) {
        if visible_ids.iter().any(|id| id == wanted) {
            return Some(wanted.to_string());
        }
    }
    visible_ids.first().cloned()
}

/// Persisted-default resolution with the SP-03 migration priority: the engine
/// mirror first (newest explicit choice), then the canonical `IndexId`, then
/// the caller-supplied visible fallback (upstream `SetDefaultServer` repair).
/// `exists` answers against the restored database; `first_visible` is
/// `None` when there is nothing to fall back to.
pub fn pick_default(
    mirror: Option<&str>,
    canonical: Option<&str>,
    exists: &dyn Fn(&str) -> bool,
    first_visible: Option<String>,
) -> Option<String> {
    if let Some(id) = present(mirror) {
        if exists(id) {
            return Some(id.to_string());
        }
    }
    if let Some(id) = present(canonical) {
        if exists(id) {
            return Some(id.to_string());
        }
    }
    first_visible
}

/// Current-group resolution against the restored subscriptions. A persisted
/// group that no longer exists resolves to `None` (the "All" view), mirroring
/// upstream `RefreshSubscriptions`, which never persists a repair for this
/// identity.
pub fn resolve_current_group(persisted: Option<&str>, existing_ids: &[String]) -> Option<String> {
    let wanted = present(persisted)?;
    if existing_ids.iter().any(|id| id == wanted) {
        return Some(wanted.to_string());
    }
    None
}

/// SP-16 "edit current subscription" entry (independent half).
///
/// Mirrors upstream `ProfilesViewModel.EditSubAsync(blNew=false)`: only a
/// persisted current group `G` that still exists resolves to a direct edit;
/// the "All" view, a blank id, or a deleted group is gated (upstream returns
/// without opening anything, it never falls back to the first group).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubEditTarget {
    EditCurrent(String),
    GatedAll,
}

/// Resolve which subscription object the "edit current" entry must open.
pub fn resolve_sub_edit_target(
    current_group: Option<&str>,
    existing_ids: &[String],
) -> SubEditTarget {
    match resolve_current_group(current_group, existing_ids) {
        Some(id) => SubEditTarget::EditCurrent(id),
        None => SubEditTarget::GatedAll,
    }
}

/// SP-18 frozen command targets (independent half).
///
/// Captured when the context menu opens; a later group switch or a target
/// leaving the visible set invalidates the command instead of falling back
/// to the first row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenCommandTargets {
    pub target_ids: Vec<String>,
    pub primary_id: Option<String>,
    pub group_sub_id: Option<String>,
}

/// Restore frozen targets against the live view. `None` means refuse with a
/// re-select prompt.
pub fn restore_command_targets(
    frozen_ids: &[String],
    frozen_primary: Option<&str>,
    frozen_group: Option<&str>,
    current_group: Option<&str>,
    visible_ids: &[String],
) -> Option<FrozenCommandTargets> {
    if frozen_group != current_group {
        return None;
    }
    if frozen_ids.is_empty() {
        return None;
    }
    if frozen_ids
        .iter()
        .any(|id| !visible_ids.iter().any(|v| v == id))
    {
        return None;
    }
    Some(FrozenCommandTargets {
        target_ids: frozen_ids.to_vec(),
        primary_id: frozen_primary.and_then(|id| present(Some(id)).map(str::to_string)),
        group_sub_id: frozen_group.map(str::to_string),
    })
}

/// Single-object commands (edit/share/activate) require the frozen primary
/// to still be visible.
pub fn is_primary_target_live(primary_id: Option<&str>, visible_ids: &[String]) -> bool {
    match present(primary_id) {
        Some(id) => visible_ids.iter().any(|v| v == id),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn pending_selection_wins_without_touching_default() {
        assert_eq!(
            resolve_visible_selection(&ids(&["a", "b", "c"]), Some("c"), Some("b")),
            Some("c".to_string())
        );
    }

    #[test]
    fn default_wins_over_first_row() {
        assert_eq!(
            resolve_visible_selection(&ids(&["a", "b", "c"]), None, Some("b")),
            Some("b".to_string())
        );
    }

    #[test]
    fn missing_pending_and_default_fall_back_to_first_row() {
        assert_eq!(
            resolve_visible_selection(&ids(&["a", "b"]), Some("ghost"), Some("ghost")),
            Some("a".to_string())
        );
    }

    #[test]
    fn empty_view_selects_nothing() {
        assert_eq!(resolve_visible_selection(&[], Some("c"), Some("b")), None);
    }

    #[test]
    fn blank_ids_behave_as_absent() {
        assert_eq!(
            resolve_visible_selection(&ids(&["a"]), Some("  "), Some("")),
            Some("a".to_string())
        );
    }

    #[test]
    fn pick_default_prefers_resolvable_mirror() {
        let exists = |id: &str| id == "b";
        assert_eq!(
            pick_default(Some("b"), Some("a"), &exists, Some("a".to_string())),
            Some("b".to_string())
        );
    }

    #[test]
    fn pick_default_falls_back_to_canonical() {
        let exists = |id: &str| id == "a";
        assert_eq!(
            pick_default(Some("ghost"), Some("a"), &exists, Some("a".to_string())),
            Some("a".to_string())
        );
    }

    #[test]
    fn pick_default_falls_back_to_first_visible() {
        let exists = |_: &str| false;
        assert_eq!(
            pick_default(Some("ghost"), Some("gone"), &exists, Some("a".to_string())),
            Some("a".to_string())
        );
    }

    #[test]
    fn pick_default_without_rows_resolves_nothing() {
        let exists = |_: &str| false;
        assert_eq!(
            pick_default(Some("ghost"), Some("gone"), &exists, None),
            None
        );
    }

    #[test]
    fn current_group_matches_or_clears_to_all() {
        assert_eq!(
            resolve_current_group(Some("g"), &ids(&["g", "h"])),
            Some("g".to_string())
        );
        assert_eq!(resolve_current_group(Some("gone"), &ids(&["g"])), None);
        assert_eq!(resolve_current_group(Some(""), &ids(&["g"])), None);
        assert_eq!(resolve_current_group(None, &ids(&["g"])), None);
    }

    #[test]
    fn sub_edit_target_direct_or_gated() {
        assert_eq!(
            resolve_sub_edit_target(Some("g"), &ids(&["g", "h"])),
            SubEditTarget::EditCurrent("g".to_string())
        );
        assert_eq!(
            resolve_sub_edit_target(None, &ids(&["g"])),
            SubEditTarget::GatedAll
        );
        assert_eq!(
            resolve_sub_edit_target(Some("gone"), &ids(&["g"])),
            SubEditTarget::GatedAll
        );
        assert_eq!(
            resolve_sub_edit_target(Some("  "), &ids(&["g"])),
            SubEditTarget::GatedAll
        );
    }

    #[test]
    fn frozen_targets_require_same_group_and_visibility() {
        let kept = restore_command_targets(
            &ids(&["n2", "n3"]),
            Some("n2"),
            Some("g"),
            Some("g"),
            &ids(&["n1", "n2", "n3"]),
        )
        .expect("kept");
        assert_eq!(kept.primary_id.as_deref(), Some("n2"));
        assert!(restore_command_targets(
            &ids(&["n2"]),
            Some("n2"),
            Some("g"),
            Some("h"),
            &ids(&["n2"]),
        )
        .is_none());
        assert!(restore_command_targets(
            &ids(&["n2"]),
            Some("n2"),
            Some("g"),
            Some("g"),
            &ids(&["n1"]),
        )
        .is_none());
        assert!(!is_primary_target_live(Some("n9"), &ids(&["n1"])));
        assert!(is_primary_target_live(Some("n1"), &ids(&["n1"])));
    }
}
