//! `FullConfigTemplateItem` (T10): validation and defaults for the 8-field
//! full-config template model.
//!
//! Upstream stores one row per core in its own SQLite table
//! (`compat/fields.entities.yaml`). This port keeps the same 8-field shape
//! ([`domain::FullConfigTemplate`]) and persists it inside `guiNConfig.json`
//! so a save survives a restart without a schema migration.

use domain::{codes, CoreType, DomainError, FullConfigTemplate};
use serde_json::Value;

/// The two built-in rows upstream seeds on first run (Xray + sing-box).
pub fn builtins() -> Vec<FullConfigTemplate> {
    vec![
        FullConfigTemplate {
            remarks: "V2ray".into(),
            core_type: CoreType::Xray,
            ..Default::default()
        },
        FullConfigTemplate {
            remarks: "sing-box".into(),
            core_type: CoreType::SingBox,
            ..Default::default()
        },
    ]
}

/// Validate an edited template. Both JSON blobs, when present, must parse as a
/// JSON object (a template may legitimately be empty/disabled).
pub fn validate(item: &FullConfigTemplate) -> Result<(), DomainError> {
    if item.remarks.trim().is_empty() {
        return Err(
            DomainError::new(codes::FIELD_REQUIRED, "error.remarks_required").with_field("remarks"),
        );
    }
    if !matches!(item.core_type, CoreType::Xray | CoreType::SingBox) {
        return Err(DomainError::new(
            codes::FIELD_FORMAT,
            "error.template_core_unsupported",
        )
        .with_field("coreType"));
    }
    validate_json_object(item.config.as_deref(), "config")?;
    validate_json_object(item.tun_config.as_deref(), "tunConfig")?;
    Ok(())
}

/// Apply `id`/`remarks` defaults for a new row.
pub fn normalize(mut item: FullConfigTemplate, generated_id: String) -> FullConfigTemplate {
    if item.id.trim().is_empty() {
        item.id = generated_id;
    }
    if item.remarks.trim().is_empty() {
        item.remarks = default_remarks(item.core_type);
    }
    item
}

/// Locate the template for `core` (by core type, stable order).
pub fn for_core<'a>(
    items: &'a [FullConfigTemplate],
    core: CoreType,
) -> Option<&'a FullConfigTemplate> {
    items.iter().find(|t| t.core_type == core)
}

fn validate_json_object(text: Option<&str>, field: &str) -> Result<(), DomainError> {
    let Some(text) = text else { return Ok(()) };
    if text.trim().is_empty() {
        return Ok(());
    }
    let value: Value = serde_json::from_str(text).map_err(|error| {
        DomainError::new(codes::FIELD_FORMAT, "error.template_json_invalid")
            .with_field(field)
            .with_detail(error.to_string())
    })?;
    if !value.is_object() {
        return Err(DomainError::new(
            codes::FIELD_FORMAT,
            "error.template_json_object_required",
        )
        .with_field(field));
    }
    Ok(())
}

fn default_remarks(core: CoreType) -> String {
    match core {
        CoreType::SingBox => "sing-box".into(),
        _ => "V2ray".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item() -> FullConfigTemplate {
        FullConfigTemplate {
            remarks: "t".into(),
            core_type: CoreType::Xray,
            ..Default::default()
        }
    }

    #[test]
    fn accepts_valid_json() {
        let mut it = item();
        it.config = Some(r#"{"log": {"loglevel": "warning"}}"#.into());
        it.tun_config = Some("{}".into());
        assert!(validate(&it).is_ok());
    }

    #[test]
    fn rejects_invalid_json() {
        let mut it = item();
        it.config = Some("{not json".into());
        let err = validate(&it).unwrap_err();
        assert_eq!(err.code, codes::FIELD_FORMAT);
        assert_eq!(err.field_path.as_deref(), Some("config"));
    }

    #[test]
    fn rejects_non_object_json() {
        let mut it = item();
        it.config = Some("[1,2,3]".into());
        assert_eq!(validate(&it).unwrap_err().code, codes::FIELD_FORMAT);
    }

    #[test]
    fn empty_template_is_valid() {
        assert!(validate(&item()).is_ok());
    }

    #[test]
    fn normalize_fills_id_and_remarks() {
        let mut it = item();
        it.remarks = String::new();
        it.id = String::new();
        let out = normalize(it, "gen-1".into());
        assert_eq!(out.id, "gen-1");
        assert_eq!(out.remarks, "V2ray");
    }

    #[test]
    fn builtins_cover_both_cores() {
        let items = builtins();
        assert!(for_core(&items, CoreType::Xray).is_some());
        assert!(for_core(&items, CoreType::SingBox).is_some());
    }
}
