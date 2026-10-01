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
}
