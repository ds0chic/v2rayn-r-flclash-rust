//! System proxy backend: per-field snapshot/apply/restore with ownership.
//!
//! Mirrors `ServiceLib/Handler/SysProxy/SysProxyHandler.cs` and the four
//! `ESysProxyType` semantics (`ForcedClear` / `ForcedChange` / `Unchanged` /
//! `Pac`). The recovery contract follows plan section 13: capture the previous
//! value per field, remember exactly what this application wrote, and on exit
//! only restore a field while its current value still equals the value we
//! wrote. Fields the user (or another tool) changed are left alone and
//! reported as conflicts.

use serde::{Deserialize, Serialize};

use crate::error::{PlatformError, Result};

pub mod fake;
#[cfg(windows)]
pub mod windows;

pub use fake::FakeSystemProxyBackend;

/// The four upstream system-proxy modes (`ESysProxyType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SysProxyMode {
    ForcedClear,
    ForcedChange,
    Unchanged,
    Pac,
}

/// Per-field view of the current-user system proxy settings.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProxyState {
    pub enabled: bool,
    pub server: Option<String>,
    pub bypass: Option<String>,
    pub auto_config_url: Option<String>,
    pub auto_detect: bool,
}

/// Settings consumed by [`SystemProxyBackend::apply`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProxySettings {
    /// Named proxy, e.g. `127.0.0.1:10809` (never this crate's PAC port).
    pub server: Option<String>,
    /// Proxy bypass/exception list (`ProxyOverride`).
    pub bypass: Option<String>,
    /// PAC URL used by [`SysProxyMode::Pac`].
    pub auto_config_url: Option<String>,
    /// Optional explicit autodetect flag. `None` keeps the per-mode default.
    pub auto_detect: Option<bool>,
}

/// A single owned field transition recorded by [`SystemProxyBackend::apply`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedChange {
    pub field: ProxyField,
    /// Value observed before the write (`None` = absent).
    pub before: Option<String>,
    /// Value this application wrote (`None` = field cleared).
    pub after: Option<String>,
}

/// The five field identifiers used for ownership comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProxyField {
    Enabled,
    Server,
    Bypass,
    AutoConfigUrl,
    AutoDetect,
}

impl ProxyField {
    /// All fields in canonical order.
    pub const ALL: [ProxyField; 5] = [
        ProxyField::Enabled,
        ProxyField::Server,
        ProxyField::Bypass,
        ProxyField::AutoConfigUrl,
        ProxyField::AutoDetect,
    ];
}

impl ProxyState {
    /// Read a field as a canonical string (`"1"`/`"0"` for booleans, `None`
    /// for absent text). Empty strings are normalised to `None`.
    pub fn field(&self, field: ProxyField) -> Option<String> {
        match field {
            ProxyField::Enabled => Some(bool_token(self.enabled)),
            ProxyField::AutoDetect => Some(bool_token(self.auto_detect)),
            ProxyField::Server => normalize_text(self.server.as_deref()),
            ProxyField::Bypass => normalize_text(self.bypass.as_deref()),
            ProxyField::AutoConfigUrl => normalize_text(self.auto_config_url.as_deref()),
        }
    }

    /// Write a canonical field value. Used by fakes and restore tests.
    pub fn set_field(&mut self, field: ProxyField, value: Option<&str>) {
        match field {
            ProxyField::Enabled => self.enabled = parse_bool(value),
            ProxyField::AutoDetect => self.auto_detect = parse_bool(value),
            ProxyField::Server => self.server = normalize_text(value),
            ProxyField::Bypass => self.bypass = normalize_text(value),
            ProxyField::AutoConfigUrl => self.auto_config_url = normalize_text(value),
        }
    }

    fn with_fields<'a, I>(pairs: I) -> ProxyState
    where
        I: IntoIterator<Item = (ProxyField, Option<&'a str>)>,
    {
        let mut state = ProxyState::default();
        for (field, value) in pairs {
            state.set_field(field, value);
        }
        state
    }
}

fn bool_token(value: bool) -> String {
    if value {
        "1".to_string()
    } else {
        "0".to_string()
    }
}

fn parse_bool(value: Option<&str>) -> bool {
    matches!(value, Some(v) if v == "1" || v.eq_ignore_ascii_case("true"))
}

fn normalize_text(value: Option<&str>) -> Option<String> {
    value.filter(|s| !s.is_empty()).map(str::to_string)
}

/// An owned field that will be written back to its pre-apply value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreAction {
    pub field: ProxyField,
    /// The value this application wrote (proof of ownership).
    pub owned_value: Option<String>,
    /// The value to write back.
    pub restore_to: Option<String>,
}

/// A field that was changed externally and is deliberately left untouched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreConflict {
    pub field: ProxyField,
    pub owned_value: Option<String>,
    pub current_value: Option<String>,
}

/// Outcome of [`restore_if_owned`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RestoreReport {
    pub restored: Vec<RestoreAction>,
    pub conflicts: Vec<RestoreConflict>,
}

impl RestoreReport {
    pub fn is_clean(&self) -> bool {
        self.conflicts.is_empty()
    }

    pub fn restored_fields(&self) -> Vec<ProxyField> {
        self.restored.iter().map(|a| a.field).collect()
    }

    pub fn conflicted_fields(&self) -> Vec<ProxyField> {
        self.conflicts.iter().map(|c| c.field).collect()
    }
}

/// Pure ownership check: decide, per recorded change, whether the field is
/// still ours to restore.
///
/// * current == after  -> still owned, schedule restore to `before`.
/// * current == before -> already at the original value (idempotent repeat, or
///   the user manually reset it); nothing to do and no conflict.
/// * otherwise         -> externally modified; keep it and report a conflict.
pub fn restore_if_owned(applied: &[AppliedChange], current: &ProxyState) -> RestoreReport {
    let mut report = RestoreReport::default();
    for change in applied {
        let current_value = current.field(change.field);
        if current_value == change.after {
            report.restored.push(RestoreAction {
                field: change.field,
                owned_value: change.after.clone(),
                restore_to: change.before.clone(),
            });
        } else if current_value != change.before {
            report.conflicts.push(RestoreConflict {
                field: change.field,
                owned_value: change.after.clone(),
                current_value,
            });
        }
    }
    report
}

/// OS-facing system proxy backend.
///
/// Implementors provide `snapshot` and `set_field`; `apply`/`restore` are
/// derived so ownership bookkeeping is identical everywhere.
pub trait SystemProxyBackend {
    /// Read the current per-field proxy state.
    fn snapshot(&self) -> Result<ProxyState>;

    /// Write one field (used to commit apply/restore transitions).
    fn set_field(&self, field: ProxyField, value: Option<&str>) -> Result<()>;

    /// Notify the OS that proxy configuration changed. Default is a no-op.
    fn notify_changed(&self) -> Result<()> {
        Ok(())
    }

    /// Apply a mode, returning only the fields actually changed by us.
    ///
    /// Fields already equal to the target value are not recorded, so we never
    /// claim ownership of a value we did not write.
    fn apply(&self, mode: SysProxyMode, settings: &ProxySettings) -> Result<Vec<AppliedChange>> {
        let target = match mode {
            SysProxyMode::Unchanged => return Ok(Vec::new()),
            SysProxyMode::ForcedClear => ProxyState::with_fields([
                (ProxyField::Enabled, Some("0")),
                (ProxyField::Server, None),
                (ProxyField::Bypass, None),
                (ProxyField::AutoConfigUrl, None),
                (ProxyField::AutoDetect, Some("0")),
            ]),
            SysProxyMode::ForcedChange => {
                let detect = settings.auto_detect.unwrap_or(false);
                ProxyState::with_fields([
                    (ProxyField::Enabled, Some("1")),
                    (ProxyField::Server, settings.server.as_deref()),
                    (ProxyField::Bypass, settings.bypass.as_deref()),
                    (ProxyField::AutoConfigUrl, None),
                    (ProxyField::AutoDetect, Some(if detect { "1" } else { "0" })),
                ])
            }
            SysProxyMode::Pac => {
                if settings.auto_config_url.as_deref().unwrap_or("").is_empty() {
                    return Err(PlatformError::Invalid(
                        "Pac mode requires a non-empty auto_config_url".to_string(),
                    ));
                }
                let detect = settings.auto_detect.unwrap_or(false);
                ProxyState::with_fields([
                    (ProxyField::Enabled, Some("0")),
                    (ProxyField::Server, None),
                    (ProxyField::Bypass, None),
                    (
                        ProxyField::AutoConfigUrl,
                        settings.auto_config_url.as_deref(),
                    ),
                    (ProxyField::AutoDetect, Some(if detect { "1" } else { "0" })),
                ])
            }
        };

        let before = self.snapshot()?;
        let mut changes = Vec::new();
        for field in ProxyField::ALL {
            let after = target.field(field);
            let prior = before.field(field);
            if prior != after {
                changes.push(AppliedChange {
                    field,
                    before: prior,
                    after,
                });
            }
        }

        for change in &changes {
            self.set_field(change.field, change.after.as_deref())?;
        }
        self.notify_changed()?;
        Ok(changes)
    }

    /// Canonical applied-content hash (SP-12/SP-15 platform dedupe).
    ///
    /// Covers mode, server, bypass, PAC url and autodetect together with the
    /// applied session key (never the desired port alone), so editing only the
    /// bypass/PAC list at the same port still changes the identity and a
    /// failed OS apply never advances the applied hash.
    fn applied_content_hash(
        &self,
        mode: SysProxyMode,
        settings: &ProxySettings,
        session_key: &str,
    ) -> String {
        let _ = self;
        applied_content_hash(mode, settings, session_key)
    }

    /// Restore only the fields this application still owns.
    fn restore(&self, applied: &[AppliedChange]) -> Result<RestoreReport> {
        let current = self.snapshot()?;
        let report = restore_if_owned(applied, &current);
        for action in &report.restored {
            self.set_field(action.field, action.restore_to.as_deref())?;
        }
        if !report.restored.is_empty() {
            self.notify_changed()?;
        }
        Ok(report)
    }
}

/// Canonical applied-content hash (SP-12/SP-15 platform dedupe).
///
/// Free function so callers without a backend instance can hash too. Covers
/// mode, server, bypass, PAC url and autodetect plus the applied session key.
pub fn applied_content_hash(
    mode: SysProxyMode,
    settings: &ProxySettings,
    session_key: &str,
) -> String {
    let mode_token = match mode {
        SysProxyMode::ForcedClear => "clear",
        SysProxyMode::ForcedChange => "change",
        SysProxyMode::Unchanged => "unchanged",
        SysProxyMode::Pac => "pac",
    };
    let canonical = format!(
        "{}|{}|{}|{}|{}|{}",
        mode_token,
        settings.server.as_deref().unwrap_or(""),
        settings.bypass.as_deref().unwrap_or(""),
        settings.auto_config_url.as_deref().unwrap_or(""),
        settings
            .auto_detect
            .map(|v| if v { "1" } else { "0" })
            .unwrap_or("-"),
        session_key,
    );
    crate::hash::md5_hex(canonical.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(field: ProxyField, before: Option<&str>, after: Option<&str>) -> AppliedChange {
        AppliedChange {
            field,
            before: before.map(str::to_string),
            after: after.map(str::to_string),
        }
    }

    #[test]
    fn bool_tokens_are_canonical() {
        let mut state = ProxyState::default();
        state.set_field(ProxyField::Enabled, Some("1"));
        assert_eq!(state.field(ProxyField::Enabled).as_deref(), Some("1"));
        state.set_field(ProxyField::Enabled, Some("true"));
        assert!(state.enabled);
        state.set_field(ProxyField::Enabled, None);
        assert!(!state.enabled);
        assert_eq!(state.field(ProxyField::Enabled).as_deref(), Some("0"));
    }

    #[test]
    fn empty_text_normalizes_to_absent() {
        let mut state = ProxyState::default();
        state.set_field(ProxyField::Server, Some(""));
        assert_eq!(state.server, None);
        assert_eq!(state.field(ProxyField::Server), None);
    }

    #[test]
    fn restore_owned_field_schedules_write_back() {
        let applied = [change(
            ProxyField::Server,
            Some("old:1"),
            Some("127.0.0.1:1"),
        )];
        let current = ProxyState::with_fields([(ProxyField::Server, Some("127.0.0.1:1"))]);
        let report = restore_if_owned(&applied, &current);
        assert_eq!(report.restored.len(), 1);
        assert_eq!(report.restored[0].restore_to.as_deref(), Some("old:1"));
        assert!(report.is_clean());
    }

    #[test]
    fn externally_changed_field_is_reported_not_restored() {
        let applied = [change(
            ProxyField::Server,
            Some("old:1"),
            Some("127.0.0.1:1"),
        )];
        let current = ProxyState::with_fields([(ProxyField::Server, Some("user:9"))]);
        let report = restore_if_owned(&applied, &current);
        assert!(report.restored.is_empty());
        assert_eq!(report.conflicts.len(), 1);
        assert_eq!(report.conflicts[0].current_value.as_deref(), Some("user:9"));
    }

    #[test]
    fn applied_hash_changes_on_bypass_only_edit() {
        let base = ProxySettings {
            server: Some("127.0.0.1:11809".to_string()),
            bypass: Some("<local>".to_string()),
            ..ProxySettings::default()
        };
        let edited = ProxySettings {
            bypass: Some("<local>;192.0.2.0/24".to_string()),
            ..base.clone()
        };
        let a = applied_content_hash(SysProxyMode::ForcedChange, &base, "sess:11809");
        let b = applied_content_hash(SysProxyMode::ForcedChange, &edited, "sess:11809");
        let c = applied_content_hash(SysProxyMode::ForcedChange, &base, "sess:11809");
        assert_ne!(a, b, "same port with different bypass must re-apply");
        assert_eq!(a, c);
    }

    #[test]
    fn already_restored_field_is_silent_on_repeat() {
        let applied = [change(ProxyField::Bypass, Some("old"), Some("new"))];
        let current = ProxyState::with_fields([(ProxyField::Bypass, Some("old"))]);
        let report = restore_if_owned(&applied, &current);
        assert!(report.restored.is_empty());
        assert!(report.is_clean());
    }
}
