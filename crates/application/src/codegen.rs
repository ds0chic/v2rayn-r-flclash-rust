//! Bridge from the persisted domain model to the pure `config_codegen` input
//! model, plus the grouped generate entry point (T10).
//!
//! The generators stay pure (no IO); this module performs the small assembly the
//! T07/T08 notes list as caller responsibility: profile-set projection, resolved
//! ports/paths and the custom-outbound content map read by the engine.

use std::collections::BTreeMap;

use config_codegen::input::{
    CodegenDns, CodegenInput, CodegenProfile, CodegenRouting, CodegenSettings, CodegenTemplate,
    ConfigType as CodegenConfigType, MultipleLoad as CodegenMultipleLoad, ProtocolExtra,
    TransportExtra,
};
use config_codegen::{generate_singbox, generate_xray, CodegenError, GeneratedConfigs};
use domain::{ConfigType, CoreType, MultipleLoad, Profile};

/// Deterministic generation context assembled by the caller.
#[derive(Debug, Clone)]
pub struct CodegenOptions {
    pub local_port: i32,
    pub state_port: i32,
    pub state_port2: i32,
    pub log_directory: String,
    pub bin_directory: String,
    pub log_date: String,
    pub speed_ping_test_url: Option<String>,
}

impl Default for CodegenOptions {
    fn default() -> Self {
        Self {
            local_port: 11808,
            state_port: 11809,
            state_port2: 11810,
            log_directory: "logs".into(),
            bin_directory: "bin".into(),
            log_date: "2026-01-01".into(),
            speed_ping_test_url: Some("https://example.com/".into()),
        }
    }
}

impl CodegenOptions {
    fn settings(&self) -> CodegenSettings {
        CodegenSettings {
            inbound: config_codegen::input::InboundSettings {
                local_port: self.local_port,
                ..Default::default()
            },
            state_port: self.state_port,
            state_port2: self.state_port2,
            log_directory: self.log_directory.clone(),
            bin_directory: self.bin_directory.clone(),
            log_date: self.log_date.clone(),
            speed_ping_test_url: self.speed_ping_test_url.clone(),
            ..Default::default()
        }
    }
}

/// Map a domain `ConfigType` onto the generator token.
pub fn config_type(value: ConfigType) -> CodegenConfigType {
    match value {
        ConfigType::Vmess => CodegenConfigType::Vmess,
        ConfigType::Custom => CodegenConfigType::Custom,
        ConfigType::Shadowsocks => CodegenConfigType::Shadowsocks,
        ConfigType::Socks => CodegenConfigType::Socks,
        ConfigType::Vless => CodegenConfigType::Vless,
        ConfigType::Trojan => CodegenConfigType::Trojan,
        ConfigType::Hysteria2 => CodegenConfigType::Hysteria2,
        ConfigType::Tuic => CodegenConfigType::Tuic,
        ConfigType::WireGuard => CodegenConfigType::WireGuard,
        ConfigType::Http => CodegenConfigType::Http,
        ConfigType::Anytls => CodegenConfigType::Anytls,
        ConfigType::Naive => CodegenConfigType::Naive,
        ConfigType::Outbound => CodegenConfigType::Outbound,
        ConfigType::PolicyGroup => CodegenConfigType::PolicyGroup,
        ConfigType::ProxyChain => CodegenConfigType::ProxyChain,
    }
}

fn multiple_load(value: Option<MultipleLoad>) -> Option<CodegenMultipleLoad> {
    value.map(|v| match v {
        MultipleLoad::LeastPing => CodegenMultipleLoad::LeastPing,
        MultipleLoad::Fallback => CodegenMultipleLoad::Fallback,
        MultipleLoad::Random => CodegenMultipleLoad::Random,
        MultipleLoad::RoundRobin => CodegenMultipleLoad::RoundRobin,
        MultipleLoad::LeastLoad => CodegenMultipleLoad::LeastLoad,
    })
}

fn proto_extra(profile: &Profile) -> ProtocolExtra {
    let p = &profile.proto_extra;
    ProtocolExtra {
        uot: p.uot,
        congestion_control: p.congestion_control.clone(),
        http_headers: p.http_headers.clone(),
        alter_id: p.alter_id.clone(),
        vmess_security: p.vmess_security.clone(),
        flow: p.flow.clone(),
        vless_encryption: p.vless_encryption.clone(),
        ss_method: p.ss_method.clone(),
        wg_public_key: p.wg_public_key.clone(),
        wg_preshared_key: p.wg_preshared_key.clone(),
        wg_interface_address: p.wg_interface_address.clone(),
        wg_reserved: p.wg_reserved.clone(),
        wg_mtu: p.wg_mtu,
        wg_dns: p.wg_dns.clone(),
        salamander_pass: p.salamander_pass.clone(),
        up_mbps: p.up_mbps,
        down_mbps: p.down_mbps,
        ports: p.ports.clone(),
        hop_interval: p.hop_interval.clone(),
        hy2_realm_url: p.hy2_realm_url.clone(),
        gecko_min_packet_size: p.gecko_min_packet_size.clone(),
        gecko_max_packet_size: p.gecko_max_packet_size.clone(),
        insecure_concurrency: p.insecure_concurrency,
        naive_quic: p.naive_quic,
        group_type: p.group_type.clone(),
        child_items: p.child_items.clone(),
        sub_child_items: p.sub_child_items.clone(),
        filter: p.filter.clone(),
        multiple_load: multiple_load(p.multiple_load),
        is_singbox_endpoint: p.is_singbox_endpoint,
    }
}

fn transport_extra(profile: &Profile) -> TransportExtra {
    let t = &profile.transport_extra;
    TransportExtra {
        raw_header_type: t.raw_header_type.clone(),
        host: t.host.clone(),
        path: t.path.clone(),
        xhttp_mode: t.xhttp_mode.clone(),
        xhttp_extra: t.xhttp_extra.clone(),
        grpc_authority: t.grpc_authority.clone(),
        grpc_service_name: t.grpc_service_name.clone(),
        grpc_mode: t.grpc_mode.clone(),
        kcp_header_type: t.kcp_header_type.clone(),
        kcp_seed: t.kcp_seed.clone(),
        kcp_mtu: t.kcp_mtu,
    }
}

fn parse_finalmask(raw: Option<&String>) -> Option<serde_json::Value> {
    let raw = raw?;
    if raw.trim().is_empty() {
        return None;
    }
    serde_json::from_str(raw).ok()
}

/// Project one domain profile onto the generator model.
pub fn to_codegen_profile(profile: &Profile, custom_config: Option<String>) -> CodegenProfile {
    let security = &profile.security;
    CodegenProfile {
        index_id: profile.index_id.clone(),
        config_type: config_type(profile.config_type),
        remarks: profile.remarks.clone(),
        address: profile.address.clone(),
        port: profile.port,
        password: profile.password.clone(),
        username: profile.username.clone(),
        network: profile.network.clone(),
        stream_security: security.stream_security.clone().unwrap_or_default(),
        allow_insecure: matches!(
            security.allow_insecure.as_deref(),
            Some("true") | Some("1") | Some("True")
        ),
        sni: security.sni.clone().unwrap_or_default(),
        alpn: security.alpn.clone().unwrap_or_default(),
        fingerprint: security.fingerprint.clone().unwrap_or_default(),
        public_key: security.public_key.clone().unwrap_or_default(),
        short_id: security.short_id.clone().unwrap_or_default(),
        spider_x: security.spider_x.clone().unwrap_or_default(),
        mldsa65_verify: security.mldsa65_verify.clone().unwrap_or_default(),
        mux_enabled: profile.mux_enabled,
        cert: security.cert.clone().unwrap_or_default(),
        cert_sha: security.cert_sha.clone().unwrap_or_default(),
        ech_config_list: security.ech_config_list.clone().unwrap_or_default(),
        verify_peer_cert_by_name: security.verify_peer_cert_by_name.clone().unwrap_or_default(),
        finalmask: parse_finalmask(profile.finalmask.as_ref()),
        proto_extra: proto_extra(profile),
        transport_extra: transport_extra(profile),
        custom_config,
    }
}

/// Key under `proto_extra.extra` holding the pass-through custom/outbound JSON
/// text for `ConfigType::Custom` / `ConfigType::Outbound` profiles.
pub const CUSTOM_CONFIG_KEY: &str = "customConfigText";

/// Read the persisted custom/outbound config text, if any.
pub fn custom_config_text(profile: &Profile) -> Option<String> {
    profile
        .proto_extra
        .extra
        .get(CUSTOM_CONFIG_KEY)
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .filter(|text| !text.trim().is_empty())
}

/// Assemble the complete generation input for `active` against `all`.
pub fn build_input(
    active: &Profile,
    all: &[Profile],
    custom_config: Option<String>,
    outbound_contents: BTreeMap<String, String>,
    template: Option<CodegenTemplate>,
    opts: &CodegenOptions,
) -> CodegenInput {
    let mut profiles: BTreeMap<String, CodegenProfile> = BTreeMap::new();
    for profile in all {
        let custom = if matches!(profile.config_type, ConfigType::Custom | ConfigType::Outbound) {
            outbound_contents
                .get(&profile.index_id)
                .cloned()
                .or_else(|| custom_config_text(profile))
        } else {
            None
        };
        profiles.insert(
            profile.index_id.clone(),
            to_codegen_profile(profile, custom),
        );
    }
    let active_custom = if matches!(active.config_type, ConfigType::Custom | ConfigType::Outbound) {
        custom_config.or_else(|| custom_config_text(active))
    } else {
        None
    };
    CodegenInput {
        profile: to_codegen_profile(active, active_custom),
        profiles,
        custom_outbound_content: outbound_contents,
        settings: opts.settings(),
        routing: Some(CodegenRouting::default()),
        dns: Some(CodegenDns::default()),
        template,
        ..Default::default()
    }
}

/// Run the structured generator for `core`.
pub fn generate(core: CoreType, input: &CodegenInput) -> Result<GeneratedConfigs, CodegenError> {
    match core {
        CoreType::SingBox => generate_singbox(input),
        _ => generate_xray(input),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::MultipleLoad;

    fn group() -> Profile {
        let mut profile = Profile {
            index_id: "group-1".into(),
            config_type: ConfigType::PolicyGroup,
            is_sub: false,
            remarks: "group".into(),
            ..Default::default()
        };
        profile.proto_extra.child_items = Some("c1,c2".into());
        profile.proto_extra.multiple_load = Some(MultipleLoad::LeastLoad);
        profile
    }

    fn leaf(id: &str, address: &str) -> Profile {
        Profile {
            index_id: id.into(),
            config_type: ConfigType::Vless,
            remarks: id.into(),
            address: address.into(),
            port: 443,
            password: "11111111-2222-3333-4444-555555555555".into(),
            ..Default::default()
        }
    }

    #[test]
    fn engine_input_generates_xray_balancer() {
        let active = group();
        let all = vec![active.clone(), leaf("c1", "192.0.2.1"), leaf("c2", "192.0.2.2")];
        let input = build_input(&active, &all, None, BTreeMap::new(), None, &CodegenOptions::default());
        let generated = generate(CoreType::Xray, &input).unwrap();
        assert_eq!(
            generated.main["routing"]["balancers"][0]["tag"],
            serde_json::json!("proxy-balancer")
        );
    }

    #[test]
    fn engine_input_generates_singbox_selector() {
        let active = group();
        let all = vec![active.clone(), leaf("c1", "192.0.2.1"), leaf("c2", "192.0.2.2")];
        let input = build_input(&active, &all, None, BTreeMap::new(), None, &CodegenOptions::default());
        let generated = generate(CoreType::SingBox, &input).unwrap();
        let outbounds = generated.main["outbounds"].as_array().unwrap();
        assert!(outbounds.iter().any(|o| o["type"] == "selector"));
        assert!(outbounds.iter().any(|o| o["type"] == "urltest"));
    }
}
