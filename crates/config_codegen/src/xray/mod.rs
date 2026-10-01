//! Xray (`CoreConfigV2rayService`) structured generator.

pub(crate) mod config;
pub(crate) mod dns;
pub(crate) mod inbound;
pub(crate) mod log;
pub(crate) mod outbound;
pub(crate) mod routing;
pub(crate) mod stat;

use serde_json::{Map, Value};

use crate::input::{CodegenInput, ConfigType};
use crate::util::*;
use crate::{CodegenError, Diagnostic, GeneratedConfigs};

/// Mutable generation state shared by the per-section builders.
pub(crate) struct XrayState<'a> {
    pub input: &'a CodegenInput,
    pub config: Map<String, Value>,
    pub outbounds: Vec<Value>,
    /// outbound tag -> custom outbound `IndexId` for `ApplyCustomOutboundReplace`.
    pub custom_tags: Vec<(String, String)>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Xray capability set (`Global.XraySupportConfigType`).
fn xray_supported(config_type: ConfigType) -> bool {
    matches!(
        config_type,
        ConfigType::Vmess
            | ConfigType::Vless
            | ConfigType::Shadowsocks
            | ConfigType::Trojan
            | ConfigType::Hysteria2
            | ConfigType::WireGuard
            | ConfigType::Socks
            | ConfigType::Http
    )
}

fn validate(input: &CodegenInput) -> Result<(), CodegenError> {
    let node = &input.profile;
    if node.config_type == ConfigType::Custom {
        // Pass-through is handled separately; it is valid.
        return Ok(());
    }
    if let Some(error) = reserved_port_error(input) {
        return Err(error);
    }
    if !xray_supported(node.config_type)
        && node.config_type != ConfigType::Outbound
        && !node.config_type.is_group()
    {
        return Err(CodegenError::unsupported_combination(
            format!(
                "config type {:?} is not in Global.XraySupportConfigType",
                node.config_type
            ),
            "profile.configType",
        ));
    }
    if node.network.trim() == "quic" {
        // T07 §8: quic is a blocked transport. Upstream GetNetwork() would
        // normalize it to raw (UNC-X-004), but the frozen contract rejects it.
        return Err(CodegenError::unsupported_combination(
            "quic transport is rejected by CoreConfigV2rayService",
            "profile.network",
        ));
    }
    if node.config_type.is_group() {
        outbound::resolve_children(input, node)?;
        return Ok(());
    }
    if node.config_type == ConfigType::Outbound {
        return Ok(());
    }
    if node.address.is_empty() {
        return Err(CodegenError::missing_required_field(
            "address is required",
            "profile.address",
        ));
    }
    if node.port <= 0 || node.port >= 65536 {
        return Err(CodegenError::missing_required_field(
            "port must be in 1..65535",
            "profile.port",
        ));
    }
    match node.config_type {
        ConfigType::Vmess | ConfigType::Vless | ConfigType::Shadowsocks | ConfigType::Trojan
            if node.password.is_empty() =>
        {
            return Err(CodegenError::missing_required_field(
                "password/uuid is required for this protocol",
                "profile.password",
            ));
        }
        _ => {}
    }
    if node.stream_security == STREAM_SECURITY_REALITY && node.public_key.is_empty() {
        return Err(CodegenError::missing_required_field(
            "publicKey is required for reality",
            "profile.publicKey",
        ));
    }
    Ok(())
}

/// `Custom` config type: pass the provided raw content through unchanged.
fn custom_passthrough(input: &CodegenInput) -> Result<GeneratedConfigs, CodegenError> {
    let raw = input.profile.custom_config.as_deref().ok_or_else(|| {
        CodegenError::missing_required_field(
            "custom profile has no custom_config content",
            "profile.customConfig",
        )
    })?;
    let main =
        serde_json::from_str::<Value>(raw).unwrap_or_else(|_| Value::String(raw.to_string()));
    let diagnostics = vec![Diagnostic::info(
        "custom_passthrough",
        "Custom profile content passed through without structural generation",
    )];
    Ok(GeneratedConfigs::new(main, diagnostics))
}

pub fn generate(input: &CodegenInput) -> Result<GeneratedConfigs, CodegenError> {
    if input.profile.config_type == ConfigType::Custom {
        return custom_passthrough(input);
    }
    validate(input)?;
    config::build(input)
}
