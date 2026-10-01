//! `Custom` (2) / `Outbound` (13) draft validation (T10).
//!
//! Mirrors `AddServer2ViewModel.SaveServerAsync`:
//! - remarks and `Address` are required (the address is the custom config file
//!   path; `EditCustomServer` copies remarks/address/core/display-log/pre-socks
//!   and the whole `ProtoExtra` blob);
//! - `PreSocksPort`, when set, must be a valid port (`GetPreSocksItem` only
//!   acts on `> 0 and <= 65535`);
//! - inline pass-through text (`proto_extra.extra["customConfigText"]`, see
//!   [`crate::codegen::CUSTOM_CONFIG_KEY`]) must parse as a JSON object or a
//!   YAML mapping so the generators can consume it.

use domain::{codes, ConfigType, DomainError, Profile};

/// Validate a `Custom` / `Outbound` draft. Other config types pass through.
pub fn validate_custom(profile: &Profile) -> Result<(), DomainError> {
    if !matches!(
        profile.config_type,
        ConfigType::Custom | ConfigType::Outbound
    ) {
        return Ok(());
    }
    if profile.remarks.trim().is_empty() {
        return Err(
            DomainError::new(codes::FIELD_REQUIRED, "error.remarks_required").with_field("remarks"),
        );
    }
    if profile.address.trim().is_empty() {
        return Err(
            DomainError::new(codes::FIELD_REQUIRED, "error.address_required").with_field("address"),
        );
    }
    if let Some(port) = profile.pre_socks_port {
        if !(1..=65535).contains(&port) {
            return Err(DomainError::new(codes::FIELD_RANGE, "error.port_range")
                .with_field("preSocksPort")
                .with_detail(format!("pre-socks port {port}")));
        }
    }
    if let Some(text) = crate::codegen::custom_config_text(profile) {
        validate_custom_text(&text)?;
    }
    Ok(())
}

/// The pass-through text must be a JSON object or a YAML mapping.
fn validate_custom_text(text: &str) -> Result<(), DomainError> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        if value.is_object() {
            return Ok(());
        }
        return Err(
            DomainError::new(codes::FIELD_FORMAT, "error.custom_json_object_required")
                .with_field("proto_extra.extra.customConfigText"),
        );
    }
    match serde_yaml::from_str::<serde_yaml::Value>(text) {
        Ok(serde_yaml::Value::Mapping(_)) => Ok(()),
        _ => Err(
            DomainError::new(codes::FIELD_FORMAT, "error.custom_config_invalid")
                .with_field("proto_extra.extra.customConfigText"),
        ),
    }
}

/// Normalize a `Custom` / `Outbound` draft before save: trim the address and
/// stamp the current config version. Range errors are left for
/// [`validate_custom`] so a bad `PreSocksPort` is rejected, not silently
/// dropped.
pub fn normalize_custom(mut profile: Profile) -> Profile {
    if !matches!(
        profile.config_type,
        ConfigType::Custom | ConfigType::Outbound
    ) {
        return profile;
    }
    profile.address = profile.address.trim().to_string();
    profile.config_version = 4;
    profile
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> Profile {
        Profile {
            index_id: "custom-1".into(),
            config_type: ConfigType::Custom,
            remarks: "custom".into(),
            address: "custom.json".into(),
            ..Default::default()
        }
    }

    #[test]
    fn accepts_json_object_text() {
        let mut p = draft();
        // Stored as a JSON string value.
        p.proto_extra.extra.insert(
            crate::codegen::CUSTOM_CONFIG_KEY.into(),
            serde_json::Value::String(r#"{"log": {"loglevel": "warning"}}"#.into()),
        );
        assert!(validate_custom(&p).is_ok());
    }

    #[test]
    fn accepts_yaml_mapping_text() {
        let mut p = draft();
        p.proto_extra.extra.insert(
            crate::codegen::CUSTOM_CONFIG_KEY.into(),
            serde_json::Value::String("mixed-port: 11808\nmode: rule\n".into()),
        );
        assert!(validate_custom(&p).is_ok());
    }

    #[test]
    fn rejects_scalar_text() {
        let mut p = draft();
        p.proto_extra.extra.insert(
            crate::codegen::CUSTOM_CONFIG_KEY.into(),
            serde_json::Value::String("[1, 2, 3]".into()),
        );
        let err = validate_custom(&p).unwrap_err();
        assert_eq!(err.code, codes::FIELD_FORMAT);
    }

    #[test]
    fn rejects_bad_pre_socks_port() {
        let mut p = draft();
        p.pre_socks_port = Some(70000);
        let err = validate_custom(&p).unwrap_err();
        assert_eq!(err.code, codes::FIELD_RANGE);
        // Normalization never masks the range error.
        let normalized = normalize_custom(p);
        assert_eq!(normalized.pre_socks_port, Some(70000));
    }

    #[test]
    fn requires_remarks_and_address() {
        let mut p = draft();
        p.remarks.clear();
        assert_eq!(validate_custom(&p).unwrap_err().code, codes::FIELD_REQUIRED);
        let mut p = draft();
        p.address.clear();
        assert_eq!(validate_custom(&p).unwrap_err().code, codes::FIELD_REQUIRED);
    }
}
