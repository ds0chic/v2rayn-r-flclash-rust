//! `FullConfigTemplateItem` (T10): the 8-field full-config template model,
//! CRUD validation and the built-in defaults.
//!
//! Upstream stores one row per core in its own SQLite table. This port keeps
//! the same shape and persists it in `guiNConfig.json` so a save survives a
//! restart without a schema migration.

use domain::{codes, CoreType, DomainError};
use serde::{Deserialize, Serialize};

/// One full-config template row (upstream `FullConfigTemplateItem`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FullConfigTemplate {
    pub id: String,
    pub remarks: String,
    pub enabled: bool,
    pub core_type: CoreType,
    pub config: Option<String>,
    pub tun_config: Option<String>,
    pub add_proxy_only: bool,
    pub proxy_detour: Option<String>,
}

impl Default for FullConfigTemplate {
    fn default() -> Self {
        Self {
            id: String::new(),
            remarks: String::new(),
            enabled: false,
            core_type: CoreType::Xray,
            config: None,
            tun_config: None,
            add_proxy_only: false,
            proxy_detour: None,
        }
    }
}

impl FullConfigTemplate {
    /// The two built-in rows upstream seeds on first run.
    pub fn builtins() -> Vec<Self> {
        vec![
            Self {
                remarks: "V2ray".into(),
                core_type: CoreType::Xray,
                ..Default::default()
            },
            Self {
                remarks: "sing-box".into(),
                core_type: CoreType::SingBox,
                ..Default::default()
            },
        ]
    }

    /// Validate an edited template. Both JSON blobs, when present, must parse
    /// as a JSON object (a template may legitimately be empty/disabled).
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.remarks.trim().is_empty() {
            return Err(
                DomainError::new(codes::FIELD_REQUIRED, "error.remarks_required")
                    .with_field("remarks"),
            );
        }
        if !matches!(self.core_type, CoreType::Xray | CoreType::SingBox) {
            return Err(DomainError::new(
                codes::FIELD_FORMAT,
                "error.template_core_unsupported",
            )
            .with_field("coreType"));
        }
        validate_json_object(self.config.as_deref(), "config")?;
        validate_json_object(self.tun_config.as_deref(), "tunConfig")?;
        Ok(())
    }

    /// Apply `id`/`remarks` defaults for a new row.
    pub fn normalize(mut self, generated_id: String) -> Self {
        if self.id.trim().is_empty() {
            self.id = generated_id;
        }
        if self.remarks.trim().is_empty() {
            self.remarks = default_remarks(self.core_type);
        }
        self
    }
}

fn validate_json_object(text: Option<&str>, field: &str) -> Result<(), DomainError> {
    let Some(text) = text else { return Ok(()) };
    if text.trim().is_empty() {
        return Ok(());
    }
    let value: serde_json::Value = serde_json::from_str(text).map_err(|error| {
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

    #[test]
    fn accepts_valid_json() {
        let item = FullConfigTemplate {
            remarks: "t".into(),
            core_type: CoreType::Xray,
            config: Some(r#"{"log": {"loglevel": "warning"}}"#.into()),
            tun_config: Some("{}".into()),
            ..Default::default()
        };
        assert!(item.validate().is_ok());
    }

    #[test]
    fn rejects_invalid_json() {
        let item = FullConfigTemplate {
            remarks: "t".into(),
            config: Some("{not json".into()),
            ..Default::default()
        };
        let err = item.validate().unwrap_err();
        assert_eq!(err.code, codes::FIELD_FORMAT);
        assert_eq!(err.field_path.as_deref(), Some("config"));
    }

    #[test]
    fn rejects_non_object_json() {
        let item = FullConfigTemplate {
            remarks: "t".into(),
            config: Some("[1,2,3]".into()),
            ..Default::default()
        };
        assert_eq!(item.validate().unwrap_err().code, codes::FIELD_FORMAT);
    }

    #[test]
    fn empty_template_is_valid() {
        let item = FullConfigTemplate {
            remarks: "t".into(),
            ..Default::default()
        };
        assert!(item.validate().is_ok());
    }
}
