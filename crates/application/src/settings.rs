//! Settings use cases: load, whole-tree save, per-group save, optimistic
//! concurrency and `apply_timing` classification.
//!
//! `guiNConfig.json` is the single source of truth. Storage itself lives in the
//! engine (it owns the atomic writer and the profile meta keys); this module
//! holds the pure state machine so it can be unit tested without a filesystem.

use std::collections::BTreeMap;

use domain::{codes, AppSettings, ApplyTiming, DomainError, SettingsChange};
use serde_json::Value;

/// In-memory settings state plus its optimistic-concurrency counters.
#[derive(Clone, Debug, Default)]
pub struct SettingsState {
    pub settings: AppSettings,
    /// Whole-tree revision (used by `save_settings`).
    pub revision: u64,
    /// Per-group revision (used by `save_settings_group`).
    pub group_revisions: BTreeMap<String, u64>,
}

impl SettingsState {
    pub fn group_revision(&self, group: &str) -> u64 {
        self.group_revisions.get(group).copied().unwrap_or(0)
    }
}

/// Result of `load_settings`.
#[derive(Debug, Clone)]
pub struct LoadedSettings {
    pub settings: AppSettings,
    pub revision: u64,
    pub group_revisions: BTreeMap<String, u64>,
}

/// Result of a successful settings save.
#[derive(Debug, Clone)]
pub struct SaveSettingsOutcome {
    pub new_revision: u64,
    /// Every field that changed, with its `apply_timing`.
    pub changes: Vec<SettingsChange>,
    /// Paths whose `apply_timing` is `restart_core`.
    pub restart_core_fields: Vec<String>,
    /// Paths whose `apply_timing` is `restart_app`.
    pub restart_app_fields: Vec<String>,
    /// Paths whose `apply_timing` is `next_launch`.
    pub next_launch_fields: Vec<String>,
}

impl SaveSettingsOutcome {
    pub fn from_changes(new_revision: u64, changes: Vec<SettingsChange>) -> Self {
        let mut restart_core = Vec::new();
        let mut restart_app = Vec::new();
        let mut next_launch = Vec::new();
        for change in &changes {
            match change.timing {
                ApplyTiming::RestartCore => restart_core.push(change.path()),
                ApplyTiming::RestartApp => restart_app.push(change.path()),
                ApplyTiming::NextLaunch => next_launch.push(change.path()),
                ApplyTiming::Immediate | ApplyTiming::Save => {}
            }
        }
        restart_core.sort();
        restart_core.dedup();
        restart_app.sort();
        restart_app.dedup();
        next_launch.sort();
        next_launch.dedup();
        Self {
            new_revision,
            changes,
            restart_core_fields: restart_core,
            restart_app_fields: restart_app,
            next_launch_fields: next_launch,
        }
    }
}

/// Validate a settings tree before persisting it. The old value is never
/// touched when this fails.
pub fn validate_settings(settings: &AppSettings) -> Result<(), DomainError> {
    for inbound in &settings.inbound {
        if !(1..=65535).contains(&inbound.local_port) {
            return Err(
                DomainError::new(codes::FIELD_RANGE, "error.local_port_range")
                    .with_field("Inbound.LocalPort"),
            );
        }
    }
    if let Some(fragment) = &settings.fragment4_ray_item {
        if let Some(max_split) = &fragment.max_split {
            if !max_split.is_empty() && max_split.parse::<u32>().is_err() {
                return Err(
                    DomainError::new(codes::FIELD_FORMAT, "error.fragment_maxsplit")
                        .with_field("Fragment4RayItem.MaxSplit"),
                );
            }
        }
    }
    if settings.speed_test_item.speed_test_timeout < 0 {
        return Err(
            DomainError::new(codes::FIELD_RANGE, "error.speed_test_timeout")
                .with_field("SpeedTestItem.SpeedTestTimeout"),
        );
    }
    Ok(())
}

/// Apply a single top-level group patch to a settings tree, then re-normalise.
pub fn apply_group_patch(
    current: &AppSettings,
    group: &str,
    patch: Value,
) -> Result<AppSettings, DomainError> {
    if !domain::is_settings_group(group) {
        return Err(
            DomainError::new(codes::INVALID_ARGUMENT, "error.unknown_settings_group")
                .with_field(group),
        );
    }
    let mut value = serde_json::to_value(current).map_err(|error| {
        DomainError::new(codes::INTERNAL, "error.storage").with_detail(error.to_string())
    })?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| DomainError::new(codes::INTERNAL, "error.storage"))?;
    object.insert(group.to_string(), patch);
    let mut next: AppSettings = serde_json::from_value(value).map_err(|error| {
        DomainError::new(codes::FIELD_FORMAT, "error.settings_patch").with_detail(error.to_string())
    })?;
    next.apply_load_defaults();
    Ok(next)
}

/// Input accepted by the engine's whole-tree save.
pub fn normalize_for_save(mut settings: AppSettings) -> AppSettings {
    settings.apply_load_defaults();
    settings
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::SettingsChange;

    fn with_port(port: i32) -> AppSettings {
        let mut settings = AppSettings::default();
        settings.inbound[0].local_port = port;
        settings
    }

    #[test]
    fn validate_rejects_out_of_range_port() {
        assert!(validate_settings(&with_port(0)).is_err());
        assert!(validate_settings(&with_port(70000)).is_err());
        assert!(validate_settings(&with_port(10808)).is_ok());
    }

    #[test]
    fn outcome_splits_timings() {
        let outcome = SaveSettingsOutcome::from_changes(
            3,
            vec![
                SettingsChange::leaf("CoreBasicItem", "Loglevel"),
                SettingsChange::leaf("GuiItem", "EnableStatistics"),
                SettingsChange::leaf("Inbound", "LocalPort"),
            ],
        );
        assert_eq!(outcome.new_revision, 3);
        assert_eq!(
            outcome.restart_core_fields,
            vec![
                "CoreBasicItem.Loglevel".to_string(),
                "Inbound.LocalPort".to_string()
            ]
        );
        assert_eq!(
            outcome.restart_app_fields,
            vec!["GuiItem.EnableStatistics".to_string()]
        );
    }

    #[test]
    fn group_patch_rejects_unknown_group() {
        let current = AppSettings::default();
        assert!(apply_group_patch(&current, "NotAGroup", serde_json::json!({})).is_err());
    }

    #[test]
    fn group_patch_replaces_only_target_group() {
        let current = AppSettings::default();
        let patch = serde_json::json!({"LogEnabled": true, "Loglevel": "debug"});
        let next = apply_group_patch(&current, "CoreBasicItem", patch).unwrap();
        assert!(next.core_basic_item.log_enabled);
        assert_eq!(next.core_basic_item.loglevel.as_deref(), Some("debug"));
        assert_eq!(next.gui_item, current.gui_item);
    }

    #[test]
    fn group_patch_preserves_unknown_keys() {
        let mut current = AppSettings::default();
        current
            .extra
            .insert("CustomRoot".to_string(), serde_json::json!({ "x": 1 }));
        current
            .gui_item
            .extra
            .insert("FutureToggle".to_string(), serde_json::json!(true));
        let next = apply_group_patch(
            &current,
            "GuiItem",
            serde_json::json!({"AutoRun": true, "FutureToggle": false}),
        )
        .unwrap();
        assert!(next.extra.contains_key("CustomRoot"));
        assert_eq!(
            next.gui_item.extra.get("FutureToggle"),
            Some(&serde_json::json!(false))
        );
    }

    #[test]
    fn outcome_reports_next_launch_and_core() {
        let outcome = SaveSettingsOutcome::from_changes(
            2,
            vec![
                SettingsChange::whole("GlobalHotkeys"),
                SettingsChange::leaf("Inbound", "LocalPort"),
            ],
        );
        assert_eq!(
            outcome.next_launch_fields,
            vec!["GlobalHotkeys".to_string()]
        );
        assert_eq!(
            outcome.restart_core_fields,
            vec!["Inbound.LocalPort".to_string()]
        );
    }

    #[test]
    fn validate_rejects_non_numeric_max_split() {
        let mut settings = AppSettings::default();
        settings.fragment4_ray_item.as_mut().unwrap().max_split = Some("abc".to_string());
        assert!(validate_settings(&settings).is_err());
    }

    #[test]
    fn normalize_is_idempotent() {
        let settings = AppSettings::default();
        let once = normalize_for_save(settings.clone());
        let twice = normalize_for_save(once.clone());
        assert_eq!(once, twice);
    }
}
