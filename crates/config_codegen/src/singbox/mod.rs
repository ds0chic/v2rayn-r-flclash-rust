//! sing-box (`CoreConfigSingboxService`) structured generator.

pub(crate) mod config;
pub(crate) mod dns;
pub(crate) mod inbound;
pub(crate) mod log;
pub(crate) mod outbound;
pub(crate) mod routing;
pub(crate) mod ruleset;
pub(crate) mod stat;

use serde_json::{Map, Value};

use crate::input::{CodegenInput, ConfigType};
use crate::util::*;
use crate::{CodegenError, Diagnostic, GeneratedConfigs};

/// Mutable generation state shared by the per-section builders.
pub(crate) struct SboxState<'a> {
    pub input: &'a CodegenInput,
    pub config: Map<String, Value>,
    pub outbounds: Vec<Value>,
    pub endpoints: Vec<Value>,
    /// outbound/endpoint tag -> custom outbound `IndexId`.
    pub custom_tags: Vec<(String, String)>,
    pub diagnostics: Vec<Diagnostic>,
}

/// sing-box capability set (`Global.SingboxSupportConfigType`).
fn singbox_supported(config_type: ConfigType) -> bool {
    matches!(
        config_type,
        ConfigType::Vmess
            | ConfigType::Vless
            | ConfigType::Shadowsocks
            | ConfigType::Trojan
            | ConfigType::Hysteria2
            | ConfigType::Tuic
            | ConfigType::Anytls
            | ConfigType::Naive
            | ConfigType::WireGuard
            | ConfigType::Socks
            | ConfigType::Http
    )
}

fn validate(input: &CodegenInput) -> Result<(), CodegenError> {
    let node = &input.profile;
    if node.config_type == ConfigType::Custom {
        return Ok(());
    }
    if !singbox_supported(node.config_type)
        && node.config_type != ConfigType::Outbound
        && !node.config_type.is_group()
    {
        return Err(CodegenError::unsupported_combination(
            format!(
                "config type {:?} is not in Global.SingboxSupportConfigType",
                node.config_type
            ),
            "profile.configType",
        ));
    }
    let network = outbound::get_network(node);
    if SINGBOX_REJECTED_NETWORKS.contains(&network.as_str()) {
        return Err(CodegenError::unsupported_combination(
            format!("{network} transport is rejected by CoreConfigSingboxService"),
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
        ConfigType::Tuic if node.username.is_empty() || node.password.is_empty() => {
            return Err(CodegenError::missing_required_field(
                "uuid/password are required for TUIC",
                "profile.username",
            ));
        }
        _ => {}
    }
    if matches!(node.config_type, ConfigType::Vless | ConfigType::Trojan)
        && node.stream_security == STREAM_SECURITY_REALITY
        && node.public_key.is_empty()
    {
        return Err(CodegenError::missing_required_field(
            "publicKey is required for reality",
            "profile.publicKey",
        ));
    }
    Ok(())
}

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
