//! `ProtoExtra` / `TransportExtra` blob (de)serialization.
//!
//! Upstream stores these as JSON text with **PascalCase** property names and
//! `DefaultIgnoreCondition = WhenWritingNull` (`JsonUtils.cs`). The blobs are
//! modelled here rather than in `domain` so the exact upstream casing and any
//! unknown keys survive a round-trip; [`ProtocolExtraBlob::to_domain`] bridges
//! to the typed domain model on demand.

use domain::{MultipleLoad, ProtocolExtra, TransportExtra};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::Result;

/// `ProtocolExtraItem` (30 properties) as persisted upstream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct ProtocolExtraBlob {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uot: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub congestion_control: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_headers: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alter_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vmess_security: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vless_encryption: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ss_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wg_public_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wg_preshared_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wg_interface_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wg_reserved: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wg_mtu: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wg_dns: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salamander_pass: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub up_mbps: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub down_mbps: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hop_interval: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hy2_realm_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gecko_min_packet_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gecko_max_packet_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insecure_concurrency: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub naive_quic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_items: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_child_items: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    /// Kept as the upstream numeric `EMultipleLoad` value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiple_load: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_singbox_endpoint: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl ProtocolExtraBlob {
    /// Parse the `ProtoExtra` column. Empty/NULL yields the default blob.
    pub fn parse(raw: Option<&str>) -> Result<Self> {
        match raw {
            None => Ok(Self::default()),
            Some(text) if text.trim().is_empty() => Ok(Self::default()),
            Some(text) => Ok(serde_json::from_str(text)?),
        }
    }

    /// Serialize back to the upstream JSON text form.
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    /// Bridge to the typed domain model. Unknown keys stay on `extra`.
    pub fn to_domain(&self) -> ProtocolExtra {
        let mut extra = self.extra.clone();
        let multiple_load = match self.multiple_load {
            Some(0) => Some(MultipleLoad::LeastPing),
            Some(1) => Some(MultipleLoad::Fallback),
            Some(2) => Some(MultipleLoad::Random),
            Some(3) => Some(MultipleLoad::RoundRobin),
            Some(4) => Some(MultipleLoad::LeastLoad),
            Some(other) => {
                extra.insert("MultipleLoad".to_string(), Value::from(other));
                None
            }
            None => None,
        };
        ProtocolExtra {
            uot: self.uot,
            congestion_control: self.congestion_control.clone(),
            http_headers: self.http_headers.clone(),
            alter_id: self.alter_id.clone(),
            vmess_security: self.vmess_security.clone(),
            flow: self.flow.clone(),
            vless_encryption: self.vless_encryption.clone(),
            ss_method: self.ss_method.clone(),
            wg_public_key: self.wg_public_key.clone(),
            wg_preshared_key: self.wg_preshared_key.clone(),
            wg_interface_address: self.wg_interface_address.clone(),
            wg_reserved: self.wg_reserved.clone(),
            wg_mtu: self.wg_mtu,
            wg_dns: self.wg_dns.clone(),
            salamander_pass: self.salamander_pass.clone(),
            up_mbps: self.up_mbps,
            down_mbps: self.down_mbps,
            ports: self.ports.clone(),
            hop_interval: self.hop_interval.clone(),
            hy2_realm_url: self.hy2_realm_url.clone(),
            gecko_min_packet_size: self.gecko_min_packet_size.clone(),
            gecko_max_packet_size: self.gecko_max_packet_size.clone(),
            insecure_concurrency: self.insecure_concurrency,
            naive_quic: self.naive_quic,
            group_type: self.group_type.clone(),
            child_items: self.child_items.clone(),
            sub_child_items: self.sub_child_items.clone(),
            filter: self.filter.clone(),
            multiple_load,
            is_singbox_endpoint: self.is_singbox_endpoint,
            extra,
        }
    }

    /// Build a blob from the typed model (unknown keys carried over).
    pub fn from_domain(value: &ProtocolExtra) -> Self {
        Self {
            uot: value.uot,
            congestion_control: value.congestion_control.clone(),
            http_headers: value.http_headers.clone(),
            alter_id: value.alter_id.clone(),
            vmess_security: value.vmess_security.clone(),
            flow: value.flow.clone(),
            vless_encryption: value.vless_encryption.clone(),
            ss_method: value.ss_method.clone(),
            wg_public_key: value.wg_public_key.clone(),
            wg_preshared_key: value.wg_preshared_key.clone(),
            wg_interface_address: value.wg_interface_address.clone(),
            wg_reserved: value.wg_reserved.clone(),
            wg_mtu: value.wg_mtu,
            wg_dns: value.wg_dns.clone(),
            salamander_pass: value.salamander_pass.clone(),
            up_mbps: value.up_mbps,
            down_mbps: value.down_mbps,
            ports: value.ports.clone(),
            hop_interval: value.hop_interval.clone(),
            hy2_realm_url: value.hy2_realm_url.clone(),
            gecko_min_packet_size: value.gecko_min_packet_size.clone(),
            gecko_max_packet_size: value.gecko_max_packet_size.clone(),
            insecure_concurrency: value.insecure_concurrency,
            naive_quic: value.naive_quic,
            group_type: value.group_type.clone(),
            child_items: value.child_items.clone(),
            sub_child_items: value.sub_child_items.clone(),
            filter: value.filter.clone(),
            multiple_load: value.multiple_load.map(multiple_load_value),
            is_singbox_endpoint: value.is_singbox_endpoint,
            extra: value.extra.clone(),
        }
    }
}

fn multiple_load_value(value: MultipleLoad) -> i32 {
    match value {
        MultipleLoad::LeastPing => 0,
        MultipleLoad::Fallback => 1,
        MultipleLoad::Random => 2,
        MultipleLoad::RoundRobin => 3,
        MultipleLoad::LeastLoad => 4,
    }
}

/// `TransportExtraItem` (11 properties) as persisted upstream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct TransportExtraBlob {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_header_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xhttp_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xhttp_extra: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grpc_authority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grpc_service_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grpc_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kcp_header_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kcp_seed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kcp_mtu: Option<i32>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl TransportExtraBlob {
    pub fn parse(raw: Option<&str>) -> Result<Self> {
        match raw {
            None => Ok(Self::default()),
            Some(text) if text.trim().is_empty() => Ok(Self::default()),
            Some(text) => Ok(serde_json::from_str(text)?),
        }
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn to_domain(&self) -> TransportExtra {
        TransportExtra {
            raw_header_type: self.raw_header_type.clone(),
            host: self.host.clone(),
            path: self.path.clone(),
            xhttp_mode: self.xhttp_mode.clone(),
            xhttp_extra: self.xhttp_extra.clone(),
            grpc_authority: self.grpc_authority.clone(),
            grpc_service_name: self.grpc_service_name.clone(),
            grpc_mode: self.grpc_mode.clone(),
            kcp_header_type: self.kcp_header_type.clone(),
            kcp_seed: self.kcp_seed.clone(),
            kcp_mtu: self.kcp_mtu,
            extra: self.extra.clone(),
        }
    }

    pub fn from_domain(value: &TransportExtra) -> Self {
        Self {
            raw_header_type: value.raw_header_type.clone(),
            host: value.host.clone(),
            path: value.path.clone(),
            xhttp_mode: value.xhttp_mode.clone(),
            xhttp_extra: value.xhttp_extra.clone(),
            grpc_authority: value.grpc_authority.clone(),
            grpc_service_name: value.grpc_service_name.clone(),
            grpc_mode: value.grpc_mode.clone(),
            kcp_header_type: value.kcp_header_type.clone(),
            kcp_seed: value.kcp_seed.clone(),
            kcp_mtu: value.kcp_mtu,
            extra: value.extra.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_extra_uses_pascal_case_and_skips_none() {
        let blob = ProtocolExtraBlob {
            ss_method: Some("aes-256-gcm".into()),
            child_items: Some("a,b".into()),
            ..Default::default()
        };
        let json = blob.to_json().unwrap();
        assert!(json.contains("\"SsMethod\":\"aes-256-gcm\""), "{json}");
        assert!(json.contains("\"ChildItems\":\"a,b\""));
        assert!(!json.contains("GroupType"));
    }

    #[test]
    fn unknown_keys_are_preserved() {
        let raw = r#"{"SsMethod":"x","BrandNewFlag":true}"#;
        let blob = ProtocolExtraBlob::parse(Some(raw)).unwrap();
        assert!(blob.extra.contains_key("BrandNewFlag"));
        let round = blob.to_json().unwrap();
        assert!(round.contains("\"BrandNewFlag\":true"));
    }

    #[test]
    fn multiple_load_maps_both_ways() {
        let blob = ProtocolExtraBlob {
            multiple_load: Some(3),
            ..Default::default()
        };
        assert_eq!(
            blob.to_domain().multiple_load,
            Some(MultipleLoad::RoundRobin)
        );
        let back = ProtocolExtraBlob::from_domain(&blob.to_domain());
        assert_eq!(back.multiple_load, Some(3));
    }

    #[test]
    fn empty_or_null_blob_is_default() {
        assert_eq!(
            ProtocolExtraBlob::parse(None).unwrap(),
            ProtocolExtraBlob::default()
        );
        assert_eq!(
            ProtocolExtraBlob::parse(Some("")).unwrap(),
            ProtocolExtraBlob::default()
        );
        assert!(ProtocolExtraBlob::parse(Some("{bad")).is_err());
    }

    #[test]
    fn transport_extra_roundtrips() {
        let raw = r#"{"GrpcAuthority":"host","GrpcServiceName":"svc","Extra":1}"#;
        let blob = TransportExtraBlob::parse(Some(raw)).unwrap();
        assert_eq!(blob.grpc_authority.as_deref(), Some("host"));
        let domain = blob.to_domain();
        assert_eq!(domain.grpc_service_name.as_deref(), Some("svc"));
        assert!(domain.extra.contains_key("Extra"));
        let out = TransportExtraBlob::from_domain(&domain).to_json().unwrap();
        assert!(out.contains("\"GrpcAuthority\":\"host\""));
    }
}
