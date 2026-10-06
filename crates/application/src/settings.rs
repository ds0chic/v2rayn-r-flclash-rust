//! Settings use cases: load, whole-tree save, per-group save, optimistic
//! concurrency and `apply_timing` classification.
//!
//! `guiNConfig.json` is the single source of truth. Storage itself lives in the
//! engine (it owns the atomic writer and the profile meta keys); this module
//! holds the pure state machine so it can be unit tested without a filesystem.

use std::collections::BTreeMap;

use domain::{codes, AppSettings, ApplyTiming, DomainError, SettingsChange, WindowState};
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
            if !try_parse_max_split(max_split, 0, 10_000) {
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

/// Upstream `Utils.TryParseMaxSplit(input, min, max)`: blank is accepted,
/// otherwise one `int` or a `from-to` pair with both ends inside
/// `[min, max]` and `from <= to`.
fn try_parse_max_split(input: &str, min: i32, max: i32) -> bool {
    if input.trim().is_empty() {
        return true;
    }
    let mut parts = input.split('-');
    let from = match parts.next().and_then(|s| s.trim().parse::<i32>().ok()) {
        Some(value) => value,
        None => return false,
    };
    let to = match parts.next() {
        None => from,
        Some(s) => match s.trim().parse::<i32>().ok() {
            Some(value) => value,
            None => return false,
        },
    };
    if parts.next().is_some() {
        return false;
    }
    from >= min && to <= max && from <= to
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

/// Upstream `ConfigHandler.GetWindowSizeItem`: look up a window geometry row by
/// its `TypeName`. Degenerate rows (non-positive size) are treated as absent so
/// the caller falls back to the default window size.
pub fn get_window_size<'a>(settings: &'a AppSettings, type_name: &str) -> Option<&'a WindowState> {
    settings
        .ui_item
        .window_size_item
        .iter()
        .find(|item| item.type_name == type_name && item.width > 0 && item.height > 0)
}

/// Upstream `ConfigHandler.SaveWindowSizeItem`: upsert a geometry row by
/// `TypeName`, leaving `MainGirdHeight*`/orientation untouched.
pub fn save_window_size(settings: &mut AppSettings, type_name: &str, width: i32, height: i32) {
    if let Some(item) = settings
        .ui_item
        .window_size_item
        .iter_mut()
        .find(|item| item.type_name == type_name)
    {
        item.width = width;
        item.height = height;
    } else {
        settings.ui_item.window_size_item.push(WindowState {
            type_name: type_name.to_string(),
            width,
            height,
            ..WindowState::default()
        });
    }
}

/// Upstream `ConfigHandler.SaveMainGirdHeight`: persist the two star sizes used
/// by the horizontal/vertical main layouts (`UiItem.MainGirdHeight1/2`).
pub fn save_main_grid_height(settings: &mut AppSettings, height1: i32, height2: i32) {
    settings.ui_item.main_gird_height1 = height1;
    settings.ui_item.main_gird_height2 = height2;
}

/// Canonical content hash of an already-serialised settings document (SP-12).
///
/// Points at the saved version so `retrySettingsApply` can refuse a stale
/// retry without re-persisting. FNV-1a/32 hex over the exact bytes; callers
/// must pass the canonical `guiNConfig.json` bytes (same key order as saved).
pub fn settings_content_hash(canonical_json: &str) -> String {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in canonical_json.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    format!("{hash:08x}")
}

/// Whether a retry may run its apply phases: the draft hash and the currently
/// persisted hash must both equal the saved version's hash (SP-12).
pub fn retry_content_matches_saved(
    saved_hash: &str,
    draft_hash: &str,
    persisted_hash: &str,
) -> bool {
    !saved_hash.is_empty() && draft_hash == saved_hash && persisted_hash == saved_hash
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

    // SP-24 G-02 (upstream `Utils.TryParseMaxSplit(_, 0, 10000)`): the
    // original client accepts range strings, so the save gate must too.
    #[test]
    fn validate_accepts_max_split_range_form() {
        for raw in ["0", "5", "1-3", "0-10000", "10000", "", "   ", "1 - 3"] {
            let mut settings = AppSettings::default();
            settings.fragment4_ray_item.as_mut().unwrap().max_split = Some(raw.to_string());
            assert!(
                validate_settings(&settings).is_ok(),
                "{raw:?} must be accepted like upstream TryParseMaxSplit"
            );
        }
        let mut settings = AppSettings::default();
        settings.fragment4_ray_item.as_mut().unwrap().max_split = None;
        assert!(validate_settings(&settings).is_ok());
    }

    #[test]
    fn validate_rejects_bad_max_split_range() {
        for raw in [
            "abc", "3-1", "0-10001", "10001", "1-2-3", "-", "1-", "-1", "1.5",
        ] {
            let mut settings = AppSettings::default();
            settings.fragment4_ray_item.as_mut().unwrap().max_split = Some(raw.to_string());
            let err = validate_settings(&settings).unwrap_err();
            assert_eq!(err.code, codes::FIELD_FORMAT, "{raw:?}");
            assert_eq!(
                err.field_path.as_deref(),
                Some("Fragment4RayItem.MaxSplit"),
                "{raw:?}"
            );
        }
    }

    #[test]
    fn normalize_is_idempotent() {
        let settings = AppSettings::default();
        let once = normalize_for_save(settings.clone());
        let twice = normalize_for_save(once.clone());
        assert_eq!(once, twice);
    }

    #[test]
    fn window_size_upserts_and_reads_by_type_name() {
        let mut settings = AppSettings::default();
        save_window_size(&mut settings, "MainWindow", 1200, 800);
        save_window_size(&mut settings, "OptionSettingWindow", 900, 600);

        let main = get_window_size(&settings, "MainWindow").unwrap();
        assert_eq!((main.width, main.height), (1200, 800));

        // Second save updates in place instead of appending a duplicate row.
        save_window_size(&mut settings, "MainWindow", 1440, 900);
        assert_eq!(settings.ui_item.window_size_item.len(), 2);
        let main = get_window_size(&settings, "MainWindow").unwrap();
        assert_eq!((main.width, main.height), (1440, 900));
    }

    #[test]
    fn degenerate_window_size_is_treated_as_absent() {
        let mut settings = AppSettings::default();
        save_window_size(&mut settings, "MainWindow", 0, 0);
        assert!(get_window_size(&settings, "MainWindow").is_none());
        assert!(get_window_size(&settings, "UnknownWindow").is_none());
    }

    #[test]
    fn main_grid_height_round_trips() {
        let mut settings = AppSettings::default();
        save_main_grid_height(&mut settings, 320, 480);
        assert_eq!(settings.ui_item.main_gird_height1, 320);
        assert_eq!(settings.ui_item.main_gird_height2, 480);
    }

    #[test]
    fn content_hash_is_stable_and_content_sensitive() {
        let a = super::settings_content_hash(r#"{"GuiItem":{"AutoRun":true}}"#);
        let b = super::settings_content_hash(r#"{"GuiItem":{"AutoRun":true}}"#);
        let c = super::settings_content_hash(r#"{"GuiItem":{"AutoRun":false}}"#);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 8);
    }

    #[test]
    fn retry_guard_rejects_diverged_content() {
        let saved = super::settings_content_hash("v1");
        let same = super::settings_content_hash("v1");
        let other = super::settings_content_hash("v2");
        assert!(super::retry_content_matches_saved(&saved, &same, &same));
        assert!(!super::retry_content_matches_saved(&saved, &other, &same));
        assert!(!super::retry_content_matches_saved(&saved, &same, &other));
        assert!(!super::retry_content_matches_saved("", &same, &same));
    }
}
