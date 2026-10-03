//! Frozen `ProfileItem` wire DTO for `v2rayn://` inner URIs.
//!
//! Mirrors the serialized shape of upstream `InnerFmt.ToUriSingle` /
//! `ResolveSingle` at `7d6a967` (`ServiceLib/Handler/Fmt/InnerFmt.cs`,
//! `ServiceLib/Models/Entities/ProfileItem.cs`): PascalCase property names,
//! `ProtoExtraObj` / `TransportExtraObj` object forms, `CustomOutboundObj`
//! for `Outbound` nodes, and verbatim preservation of unknown keys.
//!
//! The domain `Profile` keeps snake_case storage names and must not depend on
//! this module; conversion lives here (`wire_to_profile` / `profile_to_wire`).

use std::collections::BTreeMap;

use domain::{ConfigType, CoreType, MultipleLoad, Profile, ProtocolExtra, TransportExtra};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// `proto_extra.extra` key holding inner-imported outbound JSON text.
///
/// Deliberately the same contract as `application::codegen::CUSTOM_CONFIG_KEY`
/// (duplicated here because this crate only depends on `domain`): the codegen
/// input prefers caller-supplied file content, then this inline text, so an
/// inner-imported `CustomOutboundObj` stays consumable without being confused
/// with a file-path `Address`.
pub const INNER_INLINE_OUTBOUND_KEY: &str = "customConfigText";

/// Top-level `Profile.extra` keys that are local markers and must never leak
/// into the wire payload (upstream only serializes `ProfileItem` properties).
const TOP_LEVEL_DENYLIST: [&str; 1] = ["RawConfig"];

/// Loader resolving an `Outbound` file-path `Address` to its JSON content.
///
/// Kept as a callback so this crate stays I/O-free; the bridge layer supplies
/// the file read (upstream `ToUriSingle` reads `Address`, falling back to the
/// config dir). Returning `None` skips the node, like upstream's `null`.
pub type OutboundLoader<'a> = &'a dyn Fn(&str) -> Option<Value>;

/// Frozen `ProfileItem` serialization shape (PascalCase, case-sensitive here;
/// the legacy snake_case spelling is handled by the fallback path in
/// [`super::inner`], matching upstream `PropertyNameCaseInsensitive` reads).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct InnerProfile {
    pub index_id: String,
    pub config_type: Option<i32>,
    pub core_type: Option<i32>,
    pub config_version: Option<i32>,
    pub subid: String,
    pub is_sub: Option<bool>,
    pub pre_socks_port: Option<i32>,
    pub display_log: Option<bool>,
    pub remarks: String,
    pub address: String,
    pub port: Option<i32>,
    pub password: String,
    pub username: String,
    pub network: String,
    pub stream_security: String,
    pub allow_insecure: String,
    pub sni: String,
    pub alpn: String,
    pub fingerprint: String,
    pub public_key: String,
    pub short_id: String,
    pub spider_x: String,
    pub mldsa65_verify: String,
    pub cert: String,
    pub cert_sha: String,
    pub ech_config_list: String,
    pub verify_peer_cert_by_name: String,
    pub mux_enabled: Option<bool>,
    pub finalmask: String,
    /// Local extension (no upstream counterpart); round-trips verbatim and is
    /// ignored by upstream consumers as an unknown key.
    pub outbound_tag: String,
    pub proto_extra: Option<String>,
    pub transport_extra: Option<String>,
    pub proto_extra_obj: Option<Map<String, Value>>,
    pub transport_extra_obj: Option<Map<String, Value>>,
    pub custom_outbound_obj: Option<Value>,
    #[serde(flatten)]
    pub unknown: Map<String, Value>,
}

/// Frozen `ProtocolExtraItem` shape nested in `ProtoExtraObj`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ProtoExtraWire {
    pub uot: Option<bool>,
    pub congestion_control: Option<String>,
    pub http_headers: Option<String>,
    pub alter_id: Option<String>,
    pub vmess_security: Option<String>,
    pub flow: Option<String>,
    pub vless_encryption: Option<String>,
    pub ss_method: Option<String>,
    pub wg_public_key: Option<String>,
    pub wg_preshared_key: Option<String>,
    pub wg_interface_address: Option<String>,
    pub wg_reserved: Option<String>,
    pub wg_mtu: Option<i32>,
    pub wg_dns: Option<String>,
    pub salamander_pass: Option<String>,
    pub up_mbps: Option<i32>,
    pub down_mbps: Option<i32>,
    pub ports: Option<String>,
    pub hop_interval: Option<String>,
    pub hy2_realm_url: Option<String>,
    pub gecko_min_packet_size: Option<String>,
    pub gecko_max_packet_size: Option<String>,
    pub insecure_concurrency: Option<i32>,
    pub naive_quic: Option<bool>,
    pub group_type: Option<String>,
    pub child_items: Option<String>,
    pub sub_child_items: Option<String>,
    pub filter: Option<String>,
    /// Upstream `EMultipleLoad` serializes as its 0-based ordinal.
    pub multiple_load: Option<i32>,
    pub is_singbox_endpoint: Option<bool>,
    #[serde(flatten)]
    pub unknown: Map<String, Value>,
}

/// Frozen `TransportExtraItem` shape nested in `TransportExtraObj`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct TransportExtraWire {
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
    #[serde(flatten)]
    pub unknown: Map<String, Value>,
}

fn multiple_load_from_value(value: i32) -> Option<MultipleLoad> {
    Some(match value {
        0 => MultipleLoad::LeastPing,
        1 => MultipleLoad::Fallback,
        2 => MultipleLoad::Random,
        3 => MultipleLoad::RoundRobin,
        4 => MultipleLoad::LeastLoad,
        _ => return None,
    })
}

fn multiple_load_to_value(value: MultipleLoad) -> i32 {
    match value {
        MultipleLoad::LeastPing => 0,
        MultipleLoad::Fallback => 1,
        MultipleLoad::Random => 2,
        MultipleLoad::RoundRobin => 3,
        MultipleLoad::LeastLoad => 4,
    }
}

/// Pick the protocol-extra object: `ProtoExtraObj` wins over the `ProtoExtra`
/// string, exactly like upstream `ResolveSingle` overwrites the string.
fn extra_object(
    string_form: &Option<String>,
    obj_form: Option<Map<String, Value>>,
) -> Map<String, Value> {
    if let Some(obj) = obj_form {
        return obj;
    }
    if let Some(raw) = string_form.as_deref().filter(|s| !s.is_empty()) {
        if let Ok(Value::Object(obj)) = serde_json::from_str(raw) {
            return obj;
        }
    }
    Map::new()
}

fn proto_from_wire(obj: Map<String, Value>) -> Option<ProtocolExtra> {
    let wire: ProtoExtraWire = serde_json::from_value(Value::Object(obj)).ok()?;
    let multiple_load = wire.multiple_load.map(multiple_load_from_value);
    if wire.multiple_load.is_some() && multiple_load.flatten().is_none() {
        return None;
    }
    let mut extra = ProtocolExtra {
        uot: wire.uot,
        congestion_control: wire.congestion_control.filter(|s| !s.is_empty()),
        http_headers: wire.http_headers.filter(|s| !s.is_empty()),
        alter_id: wire.alter_id.filter(|s| !s.is_empty()),
        vmess_security: wire.vmess_security.filter(|s| !s.is_empty()),
        flow: wire.flow,
        vless_encryption: wire.vless_encryption.filter(|s| !s.is_empty()),
        ss_method: wire.ss_method.filter(|s| !s.is_empty()),
        wg_public_key: wire.wg_public_key.filter(|s| !s.is_empty()),
        wg_preshared_key: wire.wg_preshared_key.filter(|s| !s.is_empty()),
        wg_interface_address: wire.wg_interface_address.filter(|s| !s.is_empty()),
        wg_reserved: wire.wg_reserved.filter(|s| !s.is_empty()),
        wg_mtu: wire.wg_mtu,
        wg_dns: wire.wg_dns.filter(|s| !s.is_empty()),
        salamander_pass: wire.salamander_pass.filter(|s| !s.is_empty()),
        up_mbps: wire.up_mbps,
        down_mbps: wire.down_mbps,
        ports: wire.ports.filter(|s| !s.is_empty()),
        hop_interval: wire.hop_interval.filter(|s| !s.is_empty()),
        hy2_realm_url: wire.hy2_realm_url.filter(|s| !s.is_empty()),
        gecko_min_packet_size: wire.gecko_min_packet_size.filter(|s| !s.is_empty()),
        gecko_max_packet_size: wire.gecko_max_packet_size.filter(|s| !s.is_empty()),
        insecure_concurrency: wire.insecure_concurrency,
        naive_quic: wire.naive_quic,
        group_type: wire.group_type.filter(|s| !s.is_empty()),
        child_items: wire.child_items.filter(|s| !s.is_empty()),
        sub_child_items: wire.sub_child_items.filter(|s| !s.is_empty()),
        filter: wire.filter.filter(|s| !s.is_empty()),
        multiple_load: multiple_load.flatten(),
        is_singbox_endpoint: wire.is_singbox_endpoint,
        extra: wire.unknown,
    };
    if extra.flow.as_deref() == Some("") {
        extra.flow = None;
    }
    Some(extra)
}

fn transport_from_wire(obj: Map<String, Value>) -> Option<TransportExtra> {
    let wire: TransportExtraWire = serde_json::from_value(Value::Object(obj)).ok()?;
    Some(TransportExtra {
        raw_header_type: wire.raw_header_type.filter(|s| !s.is_empty()),
        host: wire.host.filter(|s| !s.is_empty()),
        path: wire.path.filter(|s| !s.is_empty()),
        xhttp_mode: wire.xhttp_mode.filter(|s| !s.is_empty()),
        xhttp_extra: wire.xhttp_extra.filter(|s| !s.is_empty()),
        grpc_authority: wire.grpc_authority.filter(|s| !s.is_empty()),
        grpc_service_name: wire.grpc_service_name.filter(|s| !s.is_empty()),
        grpc_mode: wire.grpc_mode.filter(|s| !s.is_empty()),
        kcp_header_type: wire.kcp_header_type.filter(|s| !s.is_empty()),
        kcp_seed: wire.kcp_seed.filter(|s| !s.is_empty()),
        kcp_mtu: wire.kcp_mtu,
        extra: wire.unknown,
    })
}

/// Convert a decoded wire object into the unified model.
///
/// Rejects exactly what upstream `ResolveSingle` rejects: wrong
/// `ConfigVersion`, undefined `ConfigType` / `CoreType` / `MultipleLoad`,
/// `Custom` nodes, and `Outbound` nodes without a `CustomOutboundObj` object.
/// Empty `Address` / `Remarks` are accepted: the upstream batch import never
/// required them (only the editor draft does).
pub fn wire_to_profile(item: InnerProfile) -> Option<Profile> {
    if item.config_version != Some(4) {
        return None;
    }
    let config_type = item.config_type.and_then(ConfigType::from_value)?;
    if config_type == ConfigType::Custom {
        return None;
    }
    let core_type = match item.core_type {
        None => None,
        Some(value) => {
            let parsed = CoreType::from_value(value)?;
            if !matches!(parsed, CoreType::Xray | CoreType::SingBox) {
                return None;
            }
            Some(parsed)
        }
    };
    let mut proto_extra = proto_from_wire(extra_object(&item.proto_extra, item.proto_extra_obj))?;
    let transport_extra = transport_from_wire(extra_object(
        &item.transport_extra,
        item.transport_extra_obj,
    ))?;

    let mut address = item.address;
    if config_type == ConfigType::Outbound {
        match item.custom_outbound_obj {
            Some(obj @ Value::Object(_)) => {
                // Inline form: keep the text under the shared inline key and
                // leave `Address` empty so file-type and inline-type never
                // impersonate each other downstream.
                proto_extra.extra.insert(
                    INNER_INLINE_OUTBOUND_KEY.to_string(),
                    Value::String(obj.to_string()),
                );
                address = String::new();
            }
            _ => return None,
        }
    }

    let mut profile = Profile {
        index_id: item.index_id,
        config_type,
        core_type,
        config_version: 4,
        subid: item.subid,
        is_sub: item.is_sub.unwrap_or(true),
        pre_socks_port: item.pre_socks_port,
        display_log: item.display_log.unwrap_or(true),
        remarks: item.remarks,
        address,
        port: item.port.unwrap_or(0),
        password: item.password,
        username: item.username,
        network: item.network,
        mux_enabled: item.mux_enabled,
        finalmask: if item.finalmask.is_empty() {
            None
        } else {
            Some(item.finalmask)
        },
        outbound_tag: if item.outbound_tag.is_empty() {
            None
        } else {
            Some(item.outbound_tag)
        },
        security: domain::SecurityParams {
            stream_security: non_empty(item.stream_security),
            allow_insecure: non_empty(item.allow_insecure),
            sni: non_empty(item.sni),
            alpn: non_empty(item.alpn),
            fingerprint: non_empty(item.fingerprint),
            public_key: non_empty(item.public_key),
            short_id: non_empty(item.short_id),
            spider_x: non_empty(item.spider_x),
            mldsa65_verify: non_empty(item.mldsa65_verify),
            cert: non_empty(item.cert),
            cert_sha: non_empty(item.cert_sha),
            ech_config_list: non_empty(item.ech_config_list),
            verify_peer_cert_by_name: non_empty(item.verify_peer_cert_by_name),
        },
        proto_extra,
        transport_extra,
        extra: item.unknown,
    };
    if profile.finalmask.as_deref() == Some("") {
        profile.finalmask = None;
    }
    Some(profile)
}

fn non_empty(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn proto_to_wire(extra: &ProtocolExtra) -> (ProtoExtraWire, Option<String>) {
    let mut wire = ProtoExtraWire {
        uot: extra.uot,
        congestion_control: extra.congestion_control.clone(),
        http_headers: extra.http_headers.clone(),
        alter_id: extra.alter_id.clone(),
        vmess_security: extra.vmess_security.clone(),
        flow: extra.flow.clone(),
        vless_encryption: extra.vless_encryption.clone(),
        ss_method: extra.ss_method.clone(),
        wg_public_key: extra.wg_public_key.clone(),
        wg_preshared_key: extra.wg_preshared_key.clone(),
        wg_interface_address: extra.wg_interface_address.clone(),
        wg_reserved: extra.wg_reserved.clone(),
        wg_mtu: extra.wg_mtu,
        wg_dns: extra.wg_dns.clone(),
        salamander_pass: extra.salamander_pass.clone(),
        up_mbps: extra.up_mbps,
        down_mbps: extra.down_mbps,
        ports: extra.ports.clone(),
        hop_interval: extra.hop_interval.clone(),
        hy2_realm_url: extra.hy2_realm_url.clone(),
        gecko_min_packet_size: extra.gecko_min_packet_size.clone(),
        gecko_max_packet_size: extra.gecko_max_packet_size.clone(),
        insecure_concurrency: extra.insecure_concurrency,
        naive_quic: extra.naive_quic,
        group_type: extra.group_type.clone(),
        child_items: extra.child_items.clone(),
        sub_child_items: extra.sub_child_items.clone(),
        filter: extra.filter.clone(),
        multiple_load: extra.multiple_load.map(multiple_load_to_value),
        is_singbox_endpoint: extra.is_singbox_endpoint,
        unknown: Map::new(),
    };
    let mut inline_outbound = None;
    for (key, value) in &extra.extra {
        if key == INNER_INLINE_OUTBOUND_KEY {
            if let Some(text) = value.as_str() {
                inline_outbound = Some(text.to_string());
            }
            continue;
        }
        wire.unknown.insert(key.clone(), value.clone());
    }
    (wire, inline_outbound)
}

fn transport_to_wire(extra: &TransportExtra) -> TransportExtraWire {
    TransportExtraWire {
        raw_header_type: extra.raw_header_type.clone(),
        host: extra.host.clone(),
        path: extra.path.clone(),
        xhttp_mode: extra.xhttp_mode.clone(),
        xhttp_extra: extra.xhttp_extra.clone(),
        grpc_authority: extra.grpc_authority.clone(),
        grpc_service_name: extra.grpc_service_name.clone(),
        grpc_mode: extra.grpc_mode.clone(),
        kcp_header_type: extra.kcp_header_type.clone(),
        kcp_seed: extra.kcp_seed.clone(),
        kcp_mtu: extra.kcp_mtu,
        unknown: extra.extra.clone(),
    }
}

/// Build the upstream-shaped JSON object for one profile.
///
/// Returns `None` for nodes upstream would not export (`Custom`, or
/// `Outbound` with neither inline content nor a loadable file). `Subid` /
/// `IsSub` are stripped by the caller pipeline, matching `ToUriSingle`.
pub fn profile_to_wire_object(
    profile: &Profile,
    outbound_loader: OutboundLoader<'_>,
) -> Option<BTreeMap<String, Value>> {
    if profile.config_type == ConfigType::Custom {
        return None;
    }
    let (proto_wire, inline_outbound) = proto_to_wire(&profile.proto_extra);
    let transport_wire = transport_to_wire(&profile.transport_extra);

    let mut custom_outbound_obj: Option<Value> = None;
    let mut address = profile.address.clone();
    if profile.config_type == ConfigType::Outbound {
        if let Some(text) = inline_outbound.filter(|s| !s.trim().is_empty()) {
            match serde_json::from_str::<Value>(&text) {
                Ok(obj @ Value::Object(_)) => {
                    custom_outbound_obj = Some(obj);
                    address.clear();
                }
                _ => return None,
            }
        } else if !address.trim().is_empty() {
            let loaded = outbound_loader(address.trim());
            match loaded {
                Some(Value::Object(_)) => {
                    custom_outbound_obj = loaded;
                    address.clear();
                }
                _ => return None,
            }
        } else {
            return None;
        }
    }

    let wire = InnerProfile {
        index_id: profile.index_id.clone(),
        config_type: Some(profile.config_type.value()),
        core_type: profile.core_type.map(|core| core.value()),
        config_version: Some(4),
        remarks: profile.remarks.clone(),
        address,
        port: Some(profile.port),
        password: profile.password.clone(),
        username: profile.username.clone(),
        network: profile.network.clone(),
        stream_security: profile.security.stream_security.clone().unwrap_or_default(),
        allow_insecure: profile.security.allow_insecure.clone().unwrap_or_default(),
        sni: profile.security.sni.clone().unwrap_or_default(),
        alpn: profile.security.alpn.clone().unwrap_or_default(),
        fingerprint: profile.security.fingerprint.clone().unwrap_or_default(),
        public_key: profile.security.public_key.clone().unwrap_or_default(),
        short_id: profile.security.short_id.clone().unwrap_or_default(),
        spider_x: profile.security.spider_x.clone().unwrap_or_default(),
        mldsa65_verify: profile.security.mldsa65_verify.clone().unwrap_or_default(),
        cert: profile.security.cert.clone().unwrap_or_default(),
        cert_sha: profile.security.cert_sha.clone().unwrap_or_default(),
        ech_config_list: profile.security.ech_config_list.clone().unwrap_or_default(),
        verify_peer_cert_by_name: profile
            .security
            .verify_peer_cert_by_name
            .clone()
            .unwrap_or_default(),
        mux_enabled: profile.mux_enabled,
        finalmask: profile.finalmask.clone().unwrap_or_default(),
        outbound_tag: profile.outbound_tag.clone().unwrap_or_default(),
        pre_socks_port: profile.pre_socks_port,
        display_log: Some(profile.display_log),
        ..InnerProfile::default()
    };

    let mut map: BTreeMap<String, Value> = BTreeMap::new();
    let mut value = serde_json::to_value(&wire).ok()?;
    if let Value::Object(obj) = value.take() {
        for (key, val) in obj {
            map.insert(key, val);
        }
    }
    // Upstream unflatten: structured extras become `*Obj` objects.
    let proto_obj = serde_json::to_value(&proto_wire).ok()?;
    if !is_empty_json(&proto_obj) {
        map.insert("ProtoExtraObj".to_string(), proto_obj);
    }
    let transport_obj = serde_json::to_value(&transport_wire).ok()?;
    if !is_empty_json(&transport_obj) {
        map.insert("TransportExtraObj".to_string(), transport_obj);
    }
    if let Some(obj) = custom_outbound_obj {
        map.insert("CustomOutboundObj".to_string(), obj);
    }
    // Carried unknown top-level keys round-trip verbatim, except local
    // markers that are never part of the upstream wire contract.
    for (key, val) in &profile.extra {
        if TOP_LEVEL_DENYLIST.contains(&key.as_str()) {
            continue;
        }
        map.entry(key.clone()).or_insert_with(|| val.clone());
    }
    // Upstream strips the subscription binding on export.
    map.remove("Subid");
    map.remove("IsSub");
    Some(map)
}

/// Read the preserved inline outbound JSON text of a profile, if any.
///
/// Returns the `CustomOutboundObj` content stored by [`wire_to_profile`] for
/// inline-type `Outbound` nodes. File-type nodes (non-empty `Address`, no
/// inline text) return `None`: the two forms never impersonate each other.
pub fn inline_outbound_text(profile: &Profile) -> Option<String> {
    profile
        .proto_extra
        .extra
        .get(INNER_INLINE_OUTBOUND_KEY)
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .filter(|text| !text.trim().is_empty())
}

fn is_empty_json(node: &Value) -> bool {
    match node {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Object(map) => map.is_empty(),
        Value::Array(items) => items.is_empty(),
        _ => false,
    }
}
