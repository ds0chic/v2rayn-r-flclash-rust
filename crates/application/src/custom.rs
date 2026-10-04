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

use domain::{codes, ConfigType, CoreType, DomainError, Profile};

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

// ---------------------------------------------------------------------------
// Ordinary-protocol normalization (RE-PROF-07)
//
// Mirrors the per-type `Add*Server` entry points in the frozen
// `ServiceLib/Handler/ConfigHandler.cs` (commit `7d6a967`) plus the shared
// `AddServerCommon` tail. Applied on both the editor save and the
// import/subscription save so the backend - not the UI defaults - owns the
// persisted shape.
// ---------------------------------------------------------------------------

/// Upstream `Global.StreamSecurity`.
pub const STREAM_SECURITY_TLS: &str = "tls";
/// Upstream `Global.StreamSecurityReality`.
const STREAM_SECURITY_REALITY: &str = "reality";
/// Upstream `Global.DefaultNetwork`.
const DEFAULT_NETWORK: &str = "raw";
/// Upstream `Global.Networks`.
const NETWORKS: [&str; 6] = ["raw", "xhttp", "kcp", "grpc", "ws", "httpupgrade"];
/// Upstream `Global.VmessSecurities`.
const VMESS_SECURITIES: [&str; 5] = ["auto", "aes-128-gcm", "chacha20-poly1305", "none", "zero"];
/// Upstream `Global.Flows`.
const VLESS_FLOWS: [&str; 3] = ["", "xtls-rprx-vision", "xtls-rprx-vision-udp443"];
/// Upstream `Global.TuicCongestionControls`; the first entry is the default.
const TUIC_CONGESTION_CONTROLS: [&str; 3] = ["cubic", "new_reno", "bbr"];
/// Upstream `Global.SsSecuritiesInXray`.
const SS_METHODS_XRAY: [&str; 11] = [
    "aes-256-gcm",
    "aes-128-gcm",
    "chacha20-poly1305",
    "chacha20-ietf-poly1305",
    "xchacha20-poly1305",
    "xchacha20-ietf-poly1305",
    "none",
    "plain",
    "2022-blake3-aes-128-gcm",
    "2022-blake3-aes-256-gcm",
    "2022-blake3-chacha20-poly1305",
];
/// Upstream `Global.SsSecuritiesInSingbox`.
const SS_METHODS_SINGBOX: [&str; 18] = [
    "aes-256-gcm",
    "aes-192-gcm",
    "aes-128-gcm",
    "chacha20-ietf-poly1305",
    "xchacha20-ietf-poly1305",
    "none",
    "2022-blake3-aes-128-gcm",
    "2022-blake3-aes-256-gcm",
    "2022-blake3-chacha20-poly1305",
    "aes-128-ctr",
    "aes-192-ctr",
    "aes-256-ctr",
    "aes-128-cfb",
    "aes-192-cfb",
    "aes-256-cfb",
    "rc4-md5",
    "chacha20-ietf",
    "xchacha20",
];

/// Normalize one ordinary protocol draft like its frozen `Add*Server`.
///
/// `Custom` / `Outbound` and the composite kinds are left untouched (they use
/// [`normalize_custom`] or the group normalizer).
pub fn normalize_server(mut profile: Profile) -> Profile {
    use ConfigType::*;
    match profile.config_type {
        Vmess => {
            profile.address = trim(&profile.address);
            profile.password = trim(&profile.password);
            profile.network = trim(&profile.network);
            profile.security.stream_security = trim_opt(profile.security.stream_security.take());
            profile.proto_extra.vmess_security =
                trim_opt(profile.proto_extra.vmess_security.take());
        }
        Vless => {
            profile.address = trim(&profile.address);
            profile.password = trim(&profile.password);
            profile.network = trim(&profile.network);
            profile.security.stream_security = trim_opt(profile.security.stream_security.take());
            // Empty encryption becomes `Global.None`.
            let encryption = profile.proto_extra.vless_encryption.take();
            let encryption = encryption.map(|v| v.trim().to_string());
            profile.proto_extra.vless_encryption = Some(match encryption {
                Some(v) if !v.is_empty() => v,
                _ => "none".to_string(),
            });
            // An unknown flow falls back to `Global.Flows.First()`.
            let flow = profile
                .proto_extra
                .flow
                .take()
                .map(|v| v.trim().to_string())
                .unwrap_or_default();
            profile.proto_extra.flow = Some(if VLESS_FLOWS.contains(&flow.as_str()) {
                flow
            } else {
                VLESS_FLOWS[0].to_string()
            });
        }
        Shadowsocks => {
            profile.address = trim(&profile.address);
            profile.password = trim(&profile.password);
            profile.proto_extra.ss_method = trim_opt(profile.proto_extra.ss_method.take());
        }
        Socks | Http => {
            profile.address = trim(&profile.address);
        }
        Trojan => {
            profile.address = trim(&profile.address);
            profile.password = trim(&profile.password);
            profile.security.stream_security = default_tls(profile.security.stream_security.take());
        }
        Hysteria2 => {
            profile.address = trim(&profile.address);
            profile.password = trim(&profile.password);
            profile.security.fingerprint = None;
            profile.security.alpn = None;
            profile.network = String::new();
            profile.security.stream_security = default_tls(profile.security.stream_security.take());
            profile.proto_extra.salamander_pass =
                trim_opt(profile.proto_extra.salamander_pass.take());
            profile.proto_extra.hop_interval = trim_opt(profile.proto_extra.hop_interval.take());
            clamp_gecko(&mut profile);
        }
        Tuic => {
            profile.core_type = Some(CoreType::SingBox);
            profile.address = trim(&profile.address);
            profile.username = trim(&profile.username);
            profile.password = trim(&profile.password);
            profile.network = String::new();
            profile.security.fingerprint = None;
            let control = profile.proto_extra.congestion_control.take();
            let valid = control
                .as_deref()
                .map(|v| TUIC_CONGESTION_CONTROLS.contains(&v))
                .unwrap_or(false);
            profile.proto_extra.congestion_control = Some(if valid {
                control.unwrap()
            } else {
                TUIC_CONGESTION_CONTROLS[0].to_string()
            });
            profile.security.stream_security = default_tls(profile.security.stream_security.take());
            if profile.security.alpn.as_deref().unwrap_or("").is_empty() {
                profile.security.alpn = Some("h3".to_string());
            }
        }
        Anytls => {
            profile.core_type = Some(CoreType::SingBox);
            profile.address = trim(&profile.address);
            profile.password = trim(&profile.password);
            profile.network = String::new();
            profile.security.stream_security = default_tls(profile.security.stream_security.take());
        }
        Naive => {
            profile.core_type = Some(CoreType::SingBox);
            profile.address = trim(&profile.address);
            profile.username = trim(&profile.username);
            profile.password = trim(&profile.password);
            profile.security.fingerprint = None;
            profile.security.alpn = None;
            profile.network = String::new();
            profile.security.allow_insecure = None;
            profile.security.stream_security = default_tls(profile.security.stream_security.take());
        }
        WireGuard => {
            profile.address = trim(&profile.address);
            profile.password = trim(&profile.password);
            profile.proto_extra.wg_public_key = trim_opt(profile.proto_extra.wg_public_key.take());
            profile.proto_extra.wg_preshared_key =
                trim_opt(profile.proto_extra.wg_preshared_key.take());
            profile.proto_extra.wg_interface_address =
                trim_opt(profile.proto_extra.wg_interface_address.take());
            profile.proto_extra.wg_reserved = trim_opt(profile.proto_extra.wg_reserved.take());
            profile.proto_extra.wg_dns = trim_opt(profile.proto_extra.wg_dns.take());
            if profile.proto_extra.wg_mtu.map(|m| m <= 0).unwrap_or(true) {
                profile.proto_extra.wg_mtu = Some(1280);
            }
        }
        Custom | Outbound | PolicyGroup | ProxyChain => return profile,
    }

    // Shared `AddServerCommon` tail.
    if !profile
        .security
        .stream_security
        .as_deref()
        .unwrap_or("")
        .is_empty()
        && profile.security.stream_security.as_deref() != Some(STREAM_SECURITY_TLS)
        && profile.security.stream_security.as_deref() != Some(STREAM_SECURITY_REALITY)
    {
        profile.security.stream_security = None;
    }
    if !profile.network.is_empty() && !NETWORKS.contains(&profile.network.as_str()) {
        profile.network = DEFAULT_NETWORK.to_string();
    }
    profile.config_version = 4;
    profile
}

/// Validate one ordinary protocol draft like its frozen `Add*Server` `-1`
/// branches (missing/invalid credentials and enum values). Explicit errors, no
/// silent correction.
pub fn validate_server(profile: &Profile) -> Result<(), DomainError> {
    use ConfigType::*;
    match profile.config_type {
        Vmess => {
            let security = profile.proto_extra.vmess_security.as_deref().unwrap_or("");
            if !VMESS_SECURITIES.contains(&security) {
                return Err(field_error(
                    codes::FIELD_FORMAT,
                    "vmessSecurity",
                    "error.vmess_security_invalid",
                ));
            }
            require_password(profile)?;
        }
        Shadowsocks => {
            let method = profile.proto_extra.ss_method.as_deref().unwrap_or("");
            if !ss_methods(profile.core_type).contains(&method) {
                return Err(field_error(
                    codes::FIELD_FORMAT,
                    "ssMethod",
                    "error.ss_method_invalid",
                ));
            }
            require_password(profile)?;
        }
        Vless | Trojan | Hysteria2 | Tuic | Anytls | Naive | WireGuard => {
            require_password(profile)?;
        }
        _ => {}
    }
    Ok(())
}

fn trim(value: &str) -> String {
    value.trim().to_string()
}

fn trim_opt(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_string())
}

/// `if IsNullOrEmpty() -> Global.StreamSecurity`.
fn default_tls(value: Option<String>) -> Option<String> {
    match value {
        Some(v) if !v.is_empty() => Some(v),
        _ => Some(STREAM_SECURITY_TLS.to_string()),
    }
}

/// Upstream Hysteria2 gecko packet-size clamp (invalid ranges become 512/1200).
fn clamp_gecko(profile: &mut Profile) {
    let min = profile
        .proto_extra
        .gecko_min_packet_size
        .as_deref()
        .unwrap_or("")
        .trim();
    let max = profile
        .proto_extra
        .gecko_max_packet_size
        .as_deref()
        .unwrap_or("")
        .trim();
    if min.is_empty() && max.is_empty() {
        return;
    }
    let min_value = min.parse::<i32>().unwrap_or(0);
    let max_value = max.parse::<i32>().unwrap_or(0);
    if min_value <= 0 || min_value > max_value || max_value > 2048 {
        profile.proto_extra.gecko_min_packet_size = Some("512".to_string());
        profile.proto_extra.gecko_max_packet_size = Some("1200".to_string());
    } else {
        profile.proto_extra.gecko_min_packet_size = Some(min_value.to_string());
        profile.proto_extra.gecko_max_packet_size = Some(max_value.to_string());
    }
}

/// Shadowsocks method list for the profile's core, mirroring
/// `AppManager.GetShadowsocksSecurities`.
fn ss_methods(core: Option<CoreType>) -> &'static [&'static str] {
    match core {
        Some(CoreType::SingBox) => &SS_METHODS_SINGBOX,
        _ => &SS_METHODS_XRAY,
    }
}

fn field_error(code: &str, field: &str, message: &str) -> DomainError {
    DomainError::new(code, message).with_field(field)
}

fn require_password(profile: &Profile) -> Result<(), DomainError> {
    if profile.password.is_empty() {
        return Err(field_error(
            codes::FIELD_REQUIRED,
            "password",
            "error.password_required",
        ));
    }
    Ok(())
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
