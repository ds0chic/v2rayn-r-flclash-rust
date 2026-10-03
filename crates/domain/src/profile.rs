//! Profile entity and its grouped protocol/transport extras.
//!
//! Field coverage follows `compat/fields.entities.yaml`:
//! - `ProfileItem` (40 properties)
//! - `ProtocolExtraItem` (30 properties)
//! - `TransportExtraItem` (11 properties)
//!
//! Unknown keys are preserved via `#[serde(flatten)] extra`, so a newer
//! upstream field never gets dropped when this app round-trips a config
//! (plan §11: "不能为了强类型而删除未知扩展").

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::enums::{ConfigType, CoreType, MultipleLoad};
use crate::reference::ReferenceExpr;

/// Bag of preserved-but-unknown JSON keys.
pub type ExtraMap = Map<String, Value>;

/// Protocol-specific, non-transport parameters (`ProtoExtra` column, a JSON
/// blob upstream). Grouped by protocol but kept in one struct so the JSON
/// blob column maps 1:1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProtocolExtra {
    /// Shadowsocks / Naive / TUIC UDP-over-TCP.
    pub uot: Option<bool>,
    /// TUIC / Naive congestion control.
    pub congestion_control: Option<String>,
    /// HTTP outbound custom headers (raw text).
    pub http_headers: Option<String>,

    // vmess
    /// Kept as string upstream even though it is numeric.
    pub alter_id: Option<String>,
    pub vmess_security: Option<String>,

    // vless
    pub flow: Option<String>,
    pub vless_encryption: Option<String>,

    // shadowsocks
    pub ss_method: Option<String>,

    // wireguard
    pub wg_public_key: Option<String>,
    pub wg_preshared_key: Option<String>,
    pub wg_interface_address: Option<String>,
    pub wg_reserved: Option<String>,
    pub wg_mtu: Option<i32>,
    pub wg_dns: Option<String>,

    // hysteria2
    pub salamander_pass: Option<String>,
    pub up_mbps: Option<i32>,
    pub down_mbps: Option<i32>,
    pub ports: Option<String>,
    pub hop_interval: Option<String>,
    pub hy2_realm_url: Option<String>,
    pub gecko_min_packet_size: Option<String>,
    pub gecko_max_packet_size: Option<String>,

    // naiveproxy
    pub insecure_concurrency: Option<i32>,
    pub naive_quic: Option<bool>,

    // group profile
    pub group_type: Option<String>,
    pub child_items: Option<String>,
    pub sub_child_items: Option<String>,
    pub filter: Option<String>,
    pub multiple_load: Option<MultipleLoad>,

    // custom outbound
    pub is_singbox_endpoint: Option<bool>,

    /// Unknown keys copied verbatim from the upstream JSON blob.
    #[serde(flatten)]
    pub extra: ExtraMap,
}

impl ProtocolExtra {
    pub fn child_items_ref(&self) -> Option<ReferenceExpr> {
        self.child_items.as_deref().map(|raw| {
            ReferenceExpr::parse_index_ids(raw, crate::reference::ReferenceSource::Imported)
        })
    }

    pub fn sub_child_items_ref(&self) -> Option<ReferenceExpr> {
        self.sub_child_items.as_deref().map(|raw| {
            ReferenceExpr::subscription_with_filter(
                raw,
                self.filter.as_deref(),
                crate::reference::ReferenceSource::Imported,
            )
        })
    }
}

/// Transport-specific parameters (`TransportExtra` column, JSON blob).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TransportExtra {
    pub raw_header_type: Option<String>,
    pub host: Option<String>,
    pub path: Option<String>,
    pub xhttp_mode: Option<String>,
    pub xhttp_extra: Option<String>,
    pub grpc_authority: Option<String>,
    pub grpc_service_name: Option<String>,
    pub grpc_mode: Option<String>,
    pub kcp_header_type: Option<String>,
    pub kcp_seed: Option<String>,
    pub kcp_mtu: Option<i32>,

    /// Unknown keys copied verbatim from the upstream JSON blob.
    #[serde(flatten)]
    pub extra: ExtraMap,
}

/// TLS/Reality parameters that live directly on `ProfileItem`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SecurityParams {
    /// `StreamSecurity`: "", "tls" or "reality".
    pub stream_security: Option<String>,
    /// Legacy string boolean ("true"/"").
    pub allow_insecure: Option<String>,
    pub sni: Option<String>,
    /// Comma-separated ALPN list as stored upstream.
    pub alpn: Option<String>,
    pub fingerprint: Option<String>,
    pub public_key: Option<String>,
    pub short_id: Option<String>,
    pub spider_x: Option<String>,
    pub mldsa65_verify: Option<String>,
    pub cert: Option<String>,
    pub cert_sha: Option<String>,
    pub ech_config_list: Option<String>,
    pub verify_peer_cert_by_name: Option<String>,
}

/// One profile node.
///
/// `proto_extra` / `transport_extra` are structured rather than a giant
/// `Map<String, String>` (plan §11 forbids the blob-everything design), while
/// `extra` preserves unknown *top-level* ProfileItem keys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    /// Primary key. Never a row number (plan §11).
    pub index_id: String,
    pub config_type: ConfigType,
    /// Optional explicit core override.
    pub core_type: Option<CoreType>,
    /// On-disk config version (V4 after ProtoExtra/TransportExtra migration).
    pub config_version: i32,
    /// Owning subscription id (empty when not from a subscription).
    pub subid: String,
    pub is_sub: bool,
    pub pre_socks_port: Option<i32>,
    pub display_log: bool,
    /// User remarks; also the resolution target for `string_remarks` refs.
    pub remarks: String,
    pub address: String,
    pub port: i32,
    pub password: String,
    pub username: String,
    /// Raw network token; use [`Profile::network`] for the normalized enum.
    pub network: String,
    pub mux_enabled: Option<bool>,
    pub finalmask: Option<String>,

    /// Outbound tag used when this profile is referenced by routing rules.
    pub outbound_tag: Option<String>,

    pub security: SecurityParams,
    pub proto_extra: ProtocolExtra,
    pub transport_extra: TransportExtra,

    /// Preserved unknown top-level keys.
    #[serde(flatten)]
    pub extra: ExtraMap,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            index_id: String::new(),
            config_type: ConfigType::Vmess,
            core_type: None,
            config_version: 4,
            subid: String::new(),
            is_sub: true,
            pre_socks_port: None,
            display_log: true,
            remarks: String::new(),
            address: String::new(),
            port: 0,
            password: String::new(),
            username: String::new(),
            network: String::new(),
            mux_enabled: None,
            finalmask: None,
            outbound_tag: None,
            security: SecurityParams::default(),
            proto_extra: ProtocolExtra::default(),
            transport_extra: TransportExtra::default(),
            extra: ExtraMap::new(),
        }
    }
}

impl Profile {
    /// Normalize the raw `network` token to the [`Network`](crate::enums::Network)
    /// enum, applying the upstream default (`raw`) for empty/unknown values.
    pub fn network(&self) -> crate::enums::Network {
        match self.network.to_ascii_lowercase().as_str() {
            "kcp" => crate::enums::Network::Kcp,
            "ws" => crate::enums::Network::Ws,
            "httpupgrade" => crate::enums::Network::HttpUpgrade,
            "xhttp" => crate::enums::Network::Xhttp,
            "h2" => crate::enums::Network::H2,
            "http" => crate::enums::Network::Http,
            "quic" => crate::enums::Network::Quic,
            "grpc" => crate::enums::Network::Grpc,
            _ => crate::enums::Network::Raw,
        }
    }

    /// Effective transport after upstream normalization (`h2`/`http` -> raw).
    pub fn effective_network(&self) -> crate::enums::Network {
        match self.network() {
            crate::enums::Network::H2 | crate::enums::Network::Http => crate::enums::Network::Raw,
            other => other,
        }
    }

    pub fn security_kind(&self) -> Option<crate::enums::Security> {
        match self
            .security
            .stream_security
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("tls") => Some(crate::enums::Security::Tls),
            Some("reality") => Some(crate::enums::Security::Reality),
            _ => None,
        }
    }

    pub fn alpn_list(&self) -> Vec<String> {
        self.security
            .alpn
            .as_deref()
            .map(crate::reference::split_list)
            .unwrap_or_default()
    }

    /// The `child_items` reference for composite/group kinds.
    pub fn child_items_ref(&self) -> Option<ReferenceExpr> {
        self.proto_extra.child_items_ref()
    }

    pub fn sub_child_items_ref(&self) -> Option<ReferenceExpr> {
        self.proto_extra.sub_child_items_ref()
    }

    /// Minimal structural validation that does not depend on the network layer.
    /// Returns the first field error, or `Ok(())`.
    pub fn validate(&self) -> Result<(), crate::error::DomainError> {
        use crate::error::{codes, DomainError};

        if self.index_id.trim().is_empty() {
            return Err(
                DomainError::new(codes::FIELD_REQUIRED, "error.index_id_required")
                    .with_field("index_id"),
            );
        }
        if self.remarks.trim().is_empty() {
            return Err(
                DomainError::new(codes::FIELD_REQUIRED, "error.remarks_required")
                    .with_field("remarks"),
            );
        }
        if self.config_type.is_complex() || self.config_type == ConfigType::Outbound {
            return Ok(());
        }
        if self.address.trim().is_empty() {
            return Err(
                DomainError::new(codes::FIELD_REQUIRED, "error.address_required")
                    .with_field("address"),
            );
        }
        if !(1..=65535).contains(&self.port) {
            return Err(DomainError::new(codes::FIELD_RANGE, "error.port_range")
                .with_field("port")
                .with_detail(format!("port {}", self.port)));
        }
        Ok(())
    }

    /// Actual node validity, mirroring upstream `ProfileItem.IsValid()`.
    ///
    /// Unlike [`Profile::validate`] (the editor-draft contract requiring
    /// remarks/address), this is the import/display-side check: complex and
    /// `Outbound` kinds are always valid, ordinary nodes need address/port,
    /// and Vmess/VLESS/Shadowsocks carry their credential rules. Reality
    /// nodes additionally require a public key.
    pub fn is_valid(&self) -> bool {
        use crate::enums::ConfigType;

        if self.config_type.is_complex() || self.config_type == ConfigType::Outbound {
            return true;
        }
        if self.address.trim().is_empty() || !(1..=65535).contains(&self.port) {
            return false;
        }
        match self.config_type {
            ConfigType::Vmess => {
                if !is_guid(&self.password) {
                    return false;
                }
            }
            ConfigType::Vless => {
                if self.password.is_empty()
                    || (!is_guid(&self.password) && self.password.len() > 30)
                {
                    return false;
                }
                if !VLESS_FLOWS.contains(&self.proto_extra.flow.as_deref().unwrap_or("")) {
                    return false;
                }
            }
            ConfigType::Shadowsocks => {
                if self.password.is_empty() {
                    return false;
                }
                match self.proto_extra.ss_method.as_deref() {
                    Some(method) if SS_METHODS_SINGBOX.contains(&method) => {}
                    _ => return false,
                }
            }
            _ => {}
        }
        if matches!(self.config_type, ConfigType::Vless | ConfigType::Trojan)
            && self
                .security
                .stream_security
                .as_deref()
                .is_some_and(|s| s.eq_ignore_ascii_case("reality"))
            && self.security.public_key.as_deref().unwrap_or("").is_empty()
        {
            return false;
        }
        true
    }
}

/// Upstream `Global.Flows` (`ServiceLib/Global.cs`).
const VLESS_FLOWS: [&str; 3] = ["", "xtls-rprx-vision", "xtls-rprx-vision-udp443"];

/// Upstream `Global.SsSecuritiesInSingbox`, the validity reference used by
/// `ProfileItem.IsValid()`.
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

/// Upstream `Utils.IsGuidByParse`: `8-4-4-4-12` hex with braces/parentheses
/// rejected (only the canonical dashed form counts here).
fn is_guid(value: &str) -> bool {
    const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != GROUPS.len() {
        return false;
    }
    parts
        .iter()
        .zip(GROUPS)
        .all(|(part, len)| part.len() == len && part.bytes().all(|b| b.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_top_level_key_is_preserved() {
        let raw = r#"{
            "index_id": "p1",
            "config_type": 5,
            "remarks": "node",
            "address": "192.0.2.1",
            "port": 443,
            "future_field": {"nested": [1, 2, 3]}
        }"#;
        let profile: Profile = serde_json::from_str(raw).unwrap();
        assert_eq!(profile.config_type, ConfigType::Vless);
        assert!(profile.extra.contains_key("future_field"));

        let out = serde_json::to_value(&profile).unwrap();
        assert_eq!(out["future_field"]["nested"][1], 2);
    }

    #[test]
    fn unknown_proto_extra_key_is_preserved() {
        let raw = r#"{"ss_method":"aes-256-gcm","brand_new_flag":true}"#;
        let extra: ProtocolExtra = serde_json::from_str(raw).unwrap();
        assert!(extra.extra.contains_key("brand_new_flag"));
        let out = serde_json::to_value(&extra).unwrap();
        assert_eq!(out["brand_new_flag"], true);
    }

    #[test]
    fn network_normalization_uses_raw_default() {
        let p = Profile {
            network: "quic".into(),
            ..Default::default()
        };
        assert_eq!(p.network(), crate::enums::Network::Quic);
        let empty = Profile::default();
        assert_eq!(empty.network(), crate::enums::Network::Raw);
        let h2 = Profile {
            network: "h2".into(),
            ..Default::default()
        };
        assert_eq!(h2.effective_network(), crate::enums::Network::Raw);
    }

    #[test]
    fn validate_requires_address_and_port() {
        let base = Profile {
            index_id: "p1".into(),
            remarks: "r".into(),
            ..Default::default()
        };
        assert_eq!(
            base.validate().unwrap_err().code,
            crate::error::codes::FIELD_REQUIRED
        );
        let valid = Profile {
            address: "192.0.2.1".into(),
            port: 443,
            ..base
        };
        assert!(valid.validate().is_ok());
        let mut p = valid;
        p.port = 0;
        assert_eq!(
            p.validate().unwrap_err().code,
            crate::error::codes::FIELD_RANGE
        );
    }

    #[test]
    fn is_valid_mirrors_upstream_rules() {
        // Complex and Outbound kinds are always valid, even empty.
        for config_type in [
            ConfigType::PolicyGroup,
            ConfigType::ProxyChain,
            ConfigType::Custom,
            ConfigType::Outbound,
        ] {
            let p = Profile {
                config_type,
                ..Default::default()
            };
            assert!(p.is_valid(), "{config_type:?} is always valid");
        }
        // Ordinary nodes need address and port; remarks are not required.
        let mut vless = Profile {
            config_type: ConfigType::Vless,
            address: "192.0.2.1".into(),
            port: 443,
            password: "11111111-2222-3333-4444-555555555555".into(),
            ..Default::default()
        };
        assert!(vless.is_valid());
        vless.address.clear();
        assert!(!vless.is_valid());
        vless.address = "192.0.2.1".into();
        vless.password = "short-non-uuid".into();
        assert!(vless.is_valid());
        vless.password = "this-password-is-longer-than-thirty-chars".into();
        assert!(!vless.is_valid());
        vless.password = "11111111-2222-3333-4444-555555555555".into();
        vless.proto_extra.flow = Some("not-a-flow".into());
        assert!(!vless.is_valid());

        let mut vmess = Profile {
            config_type: ConfigType::Vmess,
            address: "192.0.2.1".into(),
            port: 443,
            password: "not-a-guid".into(),
            ..Default::default()
        };
        assert!(!vmess.is_valid());
        vmess.password = "11111111-2222-3333-4444-555555555555".into();
        assert!(vmess.is_valid());

        let mut ss = Profile {
            config_type: ConfigType::Shadowsocks,
            address: "192.0.2.1".into(),
            port: 8388,
            password: "secret".into(),
            ..Default::default()
        };
        assert!(!ss.is_valid());
        ss.proto_extra.ss_method = Some("plain".into());
        assert!(!ss.is_valid());
        ss.proto_extra.ss_method = Some("aes-256-gcm".into());
        assert!(ss.is_valid());

        // Reality without a public key is invalid for VLESS/Trojan.
        let mut reality = Profile {
            config_type: ConfigType::Trojan,
            address: "192.0.2.1".into(),
            port: 443,
            password: "secret".into(),
            ..Default::default()
        };
        reality.security.stream_security = Some("reality".into());
        assert!(!reality.is_valid());
        reality.security.public_key = Some("key".into());
        assert!(reality.is_valid());
    }
}
