//! Bridge from the persisted domain model to the pure `config_codegen` input
//! model, plus the grouped generate entry point (T10).
//!
//! The generators stay pure (no IO); this module performs the small assembly the
//! T07/T08 notes list as caller responsibility: profile-set projection, resolved
//! ports/paths and the custom-outbound content map read by the engine.

use std::collections::BTreeMap;

use config_codegen::input::{
    CodegenDns, CodegenInput, CodegenProfile, CodegenRouting, CodegenSettings, CodegenTemplate,
    ConfigType as CodegenConfigType, MultipleLoad as CodegenMultipleLoad, ProtocolExtra, SimpleDns,
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
        verify_peer_cert_by_name: security
            .verify_peer_cert_by_name
            .clone()
            .unwrap_or_default(),
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
        let custom = if matches!(
            profile.config_type,
            ConfigType::Custom | ConfigType::Outbound
        ) {
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
    // RT-08: a file-type Custom/Outbound active node has no inline
    // `customConfigText`; its verbatim payload is resolved from `Address` by
    // the engine and arrives here through `outbound_contents`. Consult that
    // map before falling back so the active node generates from the same
    // content as the rest of the graph.
    let active_custom = if matches!(
        active.config_type,
        ConfigType::Custom | ConfigType::Outbound
    ) {
        custom_config
            .or_else(|| outbound_contents.get(&active.index_id).cloned())
            .or_else(|| custom_config_text(active))
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

/// Generate a real pre-SOCKS sidecar config. Mirrors upstream
/// `ConfigHandler.GetPreSocksItem` (a synthesized `SOCKS` node with
/// `Address = Loopback`, `Port = main core proxy port`) plus
/// `CoreManager.CoreStartPreService` -> `CoreConfigHandler.GenerateClientConfig`
/// with the full pre-context. The frozen generator therefore emits:
///
/// - a user-facing inbound on the real local port (`opts.local_port`), and
/// - a `socks` outbound that dials `dial_address:dial_port` (the main core).
///
/// The sidecar must consume the real settings tree (`settings`), not
/// `CodegenInput::default`, so TUN / simple-DNS / routing-basic / statistics
/// choices reach its config; `routing`/`dns` carry the active profile exactly
/// as the main core generation does.
pub fn generate_pre_socks_config(
    core: CoreType,
    dial_address: &str,
    dial_port: u16,
    opts: &CodegenOptions,
    settings: &domain::AppSettings,
    routing: Option<CodegenRouting>,
    dns: Option<CodegenDns>,
) -> Result<GeneratedConfigs, CodegenError> {
    let profile = CodegenProfile {
        index_id: "pre-socks".to_string(),
        config_type: CodegenConfigType::Socks,
        remarks: "pre-socks".to_string(),
        address: dial_address.to_string(),
        port: dial_port as i32,
        ..Default::default()
    };
    let mut input = CodegenInput {
        settings: settings_from_app(settings, opts),
        routing,
        dns,
        ..Default::default()
    };
    input
        .profiles
        .insert(profile.index_id.clone(), profile.clone());
    input.profile = profile;
    generate(core, &input)
}

/// Map the persisted settings tree onto generator settings (T11).
///
/// Ports/paths come from `opts`; everything else is projected from
/// `AppSettings` so routing/DNS/TUN/fragment/mux choices really reach the
/// generated kernel config.
pub fn settings_from_app(settings: &domain::AppSettings, opts: &CodegenOptions) -> CodegenSettings {
    let mut base = opts.settings();
    let core = &settings.core_basic_item;
    base.core_basic.loglevel = core.loglevel.clone().unwrap_or_else(|| "warning".into());
    base.core_basic.log_enabled = core.log_enabled;
    base.core_basic.def_fingerprint = core.def_fingerprint.clone();
    base.core_basic.def_user_agent = core.def_user_agent.clone();
    base.core_basic.send_through = core.send_through.clone();
    base.core_basic.bind_interface = core.bind_interface.clone();
    base.core_basic.enable_fragment = core.enable_fragment;
    base.core_basic.enable_final_fragment = core.enable_final_fragment;
    base.core_basic.enable_cache_file4_sbox = core.enable_cache_file4_sbox;
    let mux_ray = &settings.mux4_ray_item;
    base.mux4_ray.concurrency = mux_ray.concurrency.unwrap_or(8);
    base.mux4_ray.xudp_concurrency = mux_ray.xudp_concurrency.unwrap_or(16);
    base.mux4_ray.xudp_proxy_udp443 = mux_ray
        .xudp_proxy_udp443
        .clone()
        .unwrap_or_else(|| "reject".into());
    let mux_sbox = &settings.mux4_sbox_item;
    base.mux4_sbox.protocol = mux_sbox.protocol.clone().unwrap_or_else(|| "h2mux".into());
    base.mux4_sbox.max_connections = mux_sbox.max_connections;
    base.mux4_sbox.padding = mux_sbox.padding;
    base.kcp.mtu = settings.kcp_item.mtu;
    base.kcp.tti = settings.kcp_item.tti;
    base.kcp.uplink_capacity = settings.kcp_item.uplink_capacity;
    base.kcp.downlink_capacity = settings.kcp_item.downlink_capacity;
    base.kcp.cwnd_multiplier = settings.kcp_item.cwnd_multiplier.max(1);
    base.kcp.max_sending_window = settings.kcp_item.max_sending_window;
    base.grpc.idle_timeout = settings.grpc_item.idle_timeout;
    base.grpc.health_check_timeout = settings.grpc_item.health_check_timeout;
    base.grpc.permit_without_stream = settings.grpc_item.permit_without_stream;
    base.grpc.initial_windows_size = settings.grpc_item.initial_windows_size;
    base.hysteria.up_mbps = Some(settings.hysteria_item.up_mbps);
    base.hysteria.down_mbps = Some(settings.hysteria_item.down_mbps);
    base.hysteria.hop_interval = settings.hysteria_item.hop_interval;
    base.happy_eyeballs4_ray.try_delay_ms = settings.happy_eyeballs4_ray_item.try_delay_ms;
    base.happy_eyeballs4_ray.prioritize_ipv6 = settings.happy_eyeballs4_ray_item.prioritize_ipv6;
    base.happy_eyeballs4_ray.interleave = settings.happy_eyeballs4_ray_item.interleave;
    base.happy_eyeballs4_ray.max_concurrent_try =
        settings.happy_eyeballs4_ray_item.max_concurrent_try;
    base.gui.enable_statistics = settings.gui_item.enable_statistics;
    base.gui.display_real_time_speed = settings.gui_item.display_real_time_speed;
    // Inbound: `opts.local_port` stays the resolved listener port (caller
    // responsibility, e.g. free test ports >= 11808); every other inbound
    // field is projected from the settings tree.
    if let Some(first) = settings.inbound.first() {
        base.inbound.second_local_port_enabled = first.second_local_port_enabled;
        base.inbound.allow_lan_conn = first.allow_lan_conn;
        base.inbound.new_port4_lan = first.new_port4_lan;
        base.inbound.user = first.user.clone();
        base.inbound.pass = first.pass.clone();
        base.inbound.udp_enabled = first.udp_enabled;
        base.inbound.sniffing_enabled = first.sniffing_enabled;
        base.inbound.dest_override = first.dest_override.clone().unwrap_or_default();
        base.inbound.route_only = first.route_only;
        base.inbound.protocol = inbound_protocol_token(first.protocol).to_string();
    }
    let tun = &settings.tun_mode_item;
    base.tun.enabled = tun.enable_tun;
    base.tun.mtu = tun.mtu;
    base.tun.ipv4_address = tun.ipv4_address.clone();
    base.tun.enable_ipv6_address = tun.enable_ipv6_address;
    base.tun.ipv6_address = tun.ipv6_address.clone();
    base.tun.route_exclude_address = tun.route_exclude_address.clone().unwrap_or_default();
    base.tun.auto_route = tun.auto_route;
    base.tun.strict_route = tun.strict_route;
    base.tun.stack = tun.stack.clone();
    base.tun.icmp_routing = tun.icmp_routing.clone();
    let routing_basic = &settings.routing_basic_item;
    base.routing_basic.domain_strategy = routing_basic
        .domain_strategy
        .clone()
        .unwrap_or_else(|| "AsIs".into());
    base.routing_basic.domain_strategy4_singbox = routing_basic.domain_strategy4_singbox.clone();
    base.speed_ping_test_url = settings.speed_test_item.speed_ping_test_url.clone();
    if let Some(fragment) = settings.fragment4_ray_item.as_ref() {
        base.fragment4_ray.packets = fragment.packets.clone();
        base.fragment4_ray.lengths = fragment.lengths.clone().unwrap_or_default();
        base.fragment4_ray.delays = fragment.delays.clone().unwrap_or_default();
        base.fragment4_ray.max_split = fragment.max_split.clone();
    }
    base
}

/// Kernel inbound protocol token for a stored `Inbound.Protocol` (FLD-CFG-036).
///
/// Upstream `V2rayInboundService.BuildInbound` marks the user inbound as
/// `mixed` for the `socks`/`socks2`/`socks3` identities (a mixed listener
/// serves SOCKS and HTTP on one port; the identity only picks the port
/// offset). The token is still *derived* here rather than hardcoded in the
/// generator so an explicit non-socks identity is honored instead of being
/// silently ignored.
pub fn inbound_protocol_token(protocol: domain::InboundProtocol) -> &'static str {
    use domain::InboundProtocol as P;
    match protocol {
        // SOCKS-family listeners are generated as `mixed` (upstream parity).
        P::Socks | P::Socks2 | P::Socks3 | P::Mixed => "mixed",
        // PAC/API/speedtest are internal endpoints; `GenInbounds` never emits
        // them as user listeners, so they fall back to the mixed listener.
        P::Pac | P::Api | P::Api2 | P::Speedtest => "mixed",
    }
}

/// Map a stored routing profile onto the generator model (T11).
///
/// `custom_ruleset` is the already-read file content (the caller does IO).
pub fn routing_to_codegen(
    profile: &domain::RoutingProfile,
    custom_ruleset: Option<serde_json::Value>,
) -> CodegenRouting {
    let rules = profile.rules().unwrap_or_default();
    CodegenRouting {
        rule_set: rules
            .into_iter()
            .map(|r| config_codegen::input::CodegenRule {
                enabled: r.enabled,
                rule_type: match r.rule_type {
                    Some(domain::RuleType::All) => config_codegen::input::RuleType::All,
                    Some(domain::RuleType::Dns) => config_codegen::input::RuleType::Dns,
                    _ => config_codegen::input::RuleType::Routing,
                },
                outbound_tag: r.outbound_tag.unwrap_or_default(),
                port: r.port,
                network: r.network,
                inbound_tag: r.inbound_tag,
                domain: r.domain,
                ip: r.ip,
                protocol: r.protocol,
                process: r.process,
                id: r.id,
            })
            .collect(),
        domain_strategy: unless_empty(&profile.domain_strategy),
        domain_strategy4_singbox: unless_empty(&profile.domain_strategy4_singbox),
        custom_ruleset,
    }
}

fn unless_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Map stored DNS state onto the generator model (T11).
pub fn dns_to_codegen(
    item: Option<&domain::DnsProfile>,
    simple: &domain::SimpleDnsItem,
    system_hosts: std::collections::BTreeMap<String, String>,
    protect_domain_list: Vec<String>,
) -> CodegenDns {
    CodegenDns {
        enabled: item.map(|d| d.enabled).unwrap_or(false),
        normal: item.and_then(|d| d.normal_dns.clone()),
        tun: item.and_then(|d| d.tun_dns.clone()),
        domain_strategy4_freedom: item.and_then(|d| d.domain_strategy4_freedom.clone()),
        domain_dns_address: item.and_then(|d| d.domain_dns_address.clone()),
        use_system_hosts: item.map(|d| d.use_system_hosts).unwrap_or(false),
        protect_domain_list,
        system_hosts,
        simple: SimpleDns {
            direct_dns: simple.direct_dns.clone(),
            remote_dns: simple.remote_dns.clone(),
            bootstrap_dns: simple.bootstrap_dns.clone(),
            direct_expected_ips: simple.direct_expected_ips.clone(),
            strategy4_freedom: simple.strategy4_freedom.clone(),
            strategy4_proxy: simple.strategy4_proxy.clone(),
            strategy4_proxy_dial: simple.strategy4_proxy_dial.clone(),
            serve_stale: simple.serve_stale.unwrap_or(false),
            parallel_query: simple.parallel_query.unwrap_or(false),
            block_aaaa_query: simple.block_aaaa_query.unwrap_or(false),
            block_binding_query: simple.block_binding_query.unwrap_or(false),
            add_common_hosts: simple.add_common_hosts.unwrap_or(false),
            use_system_hosts: simple.use_system_hosts.unwrap_or(false),
            hosts: simple.hosts.clone(),
            fake_ip: simple.fake_ip.unwrap_or(false),
            global_fake_ip: simple.global_fake_ip,
            fake_ip_range: simple.fake_ip_range.clone(),
            enable_happy_eyeballs: simple.enable_happy_eyeballs.unwrap_or(false),
        },
    }
}

/// Assemble generation input with real routing/DNS/settings (T11).
#[allow(clippy::too_many_arguments)]
pub fn build_input_full(
    active: &Profile,
    all: &[Profile],
    custom_config: Option<String>,
    outbound_contents: BTreeMap<String, String>,
    template: Option<CodegenTemplate>,
    opts: &CodegenOptions,
    settings: &domain::AppSettings,
    routing: Option<CodegenRouting>,
    dns: Option<CodegenDns>,
    rule_mode: Option<String>,
) -> CodegenInput {
    let mut input = build_input(
        active,
        all,
        custom_config,
        outbound_contents,
        template,
        opts,
    );
    input.settings = settings_from_app(settings, opts);
    input.routing = routing;
    input.dns = dns;
    input.rule_mode = rule_mode;
    input
}

/// Map a stored [`domain::FullConfigTemplate`] row onto the generator model.
///
/// A disabled row, or an enabled row with no content, yields `None` (the
/// generator then behaves as if no template was set).
pub fn template_for(item: &domain::FullConfigTemplate) -> Option<CodegenTemplate> {
    if !item.enabled {
        return None;
    }
    let has_config = item
        .config
        .as_deref()
        .is_some_and(|text| !text.trim().is_empty());
    let has_tun = item
        .tun_config
        .as_deref()
        .is_some_and(|text| !text.trim().is_empty());
    if !has_config && !has_tun {
        return None;
    }
    Some(CodegenTemplate {
        enabled: true,
        config: item.config.clone(),
        tun_config: item.tun_config.clone(),
        add_proxy_only: item.add_proxy_only.unwrap_or(false),
        proxy_detour: item.proxy_detour.clone(),
    })
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
        let all = vec![
            active.clone(),
            leaf("c1", "192.0.2.1"),
            leaf("c2", "192.0.2.2"),
        ];
        let input = build_input(
            &active,
            &all,
            None,
            BTreeMap::new(),
            None,
            &CodegenOptions::default(),
        );
        let generated = generate(CoreType::Xray, &input).unwrap();
        assert_eq!(
            generated.main["routing"]["balancers"][0]["tag"],
            serde_json::json!("proxy-balancer")
        );
    }

    #[test]
    fn active_file_type_custom_consumes_outbound_contents() {
        // FIX-04B: a Custom/Outbound active node whose payload is file-backed
        // has no inline `customConfigText`; the engine feeds the file text via
        // `outbound_contents`, which must reach `input.profile.custom_config`.
        let active = Profile {
            index_id: "custom-file".into(),
            config_type: ConfigType::Custom,
            core_type: Some(CoreType::Xray),
            remarks: "custom".into(),
            ..Default::default()
        };
        let raw = r#"{"log":{"loglevel":"warning"},"inbounds":[],"outbounds":[{"protocol":"freedom","tag":"direct"}],"fix04b_marker":true}"#;
        let mut contents = BTreeMap::new();
        contents.insert("custom-file".to_string(), raw.to_string());
        let input = build_input(
            &active,
            std::slice::from_ref(&active),
            None,
            contents,
            None,
            &CodegenOptions::default(),
        );
        assert_eq!(input.profile.custom_config.as_deref(), Some(raw));
        let generated = generate(CoreType::Xray, &input).unwrap();
        let text = serde_json::to_string(&generated.main).unwrap();
        assert!(text.contains("fix04b_marker"), "{text}");
    }

    #[test]
    fn engine_input_generates_singbox_selector() {
        let active = group();
        let all = vec![
            active.clone(),
            leaf("c1", "192.0.2.1"),
            leaf("c2", "192.0.2.2"),
        ];
        let input = build_input(
            &active,
            &all,
            None,
            BTreeMap::new(),
            None,
            &CodegenOptions::default(),
        );
        let generated = generate(CoreType::SingBox, &input).unwrap();
        let outbounds = generated.main["outbounds"].as_array().unwrap();
        assert!(outbounds.iter().any(|o| o["type"] == "selector"));
        assert!(outbounds.iter().any(|o| o["type"] == "urltest"));
    }

    fn chain() -> Profile {
        let mut profile = Profile {
            index_id: "chain-1".into(),
            config_type: ConfigType::ProxyChain,
            is_sub: false,
            remarks: "chain".into(),
            ..Default::default()
        };
        profile.proto_extra.child_items = Some("c1,c2,c3".into());
        profile
    }

    fn chain_graph() -> (Profile, Vec<Profile>) {
        let active = chain();
        let all = vec![
            active.clone(),
            leaf("c1", "192.0.2.11"),
            leaf("c2", "192.0.2.12"),
            leaf("c3", "192.0.2.13"),
        ];
        (active, all)
    }

    #[test]
    fn engine_input_generates_xray_dialer_proxy_chain() {
        let (active, all) = chain_graph();
        let input = build_input(
            &active,
            &all,
            None,
            BTreeMap::new(),
            None,
            &CodegenOptions::default(),
        );
        let generated = generate(CoreType::Xray, &input).unwrap();
        let outbounds = generated.main["outbounds"].as_array().unwrap();
        // Upstream `BuildChainOutboundsList` reverses `ChildItems`: the entry
        // (`proxy`) is the last child and dials the next hop on the wire.
        assert_eq!(outbounds[0]["tag"], serde_json::json!("proxy"));
        assert_eq!(
            outbounds[0]["settings"]["address"],
            serde_json::json!("192.0.2.13")
        );
        assert_eq!(
            outbounds[0]["streamSettings"]["sockopt"]["dialerProxy"],
            serde_json::json!("chain-proxy-1-c2")
        );
        assert_eq!(outbounds[1]["tag"], serde_json::json!("chain-proxy-1-c2"));
        assert_eq!(
            outbounds[1]["streamSettings"]["sockopt"]["dialerProxy"],
            serde_json::json!("chain-proxy-2-c1")
        );
        assert_eq!(outbounds[2]["tag"], serde_json::json!("chain-proxy-2-c1"));
        assert!(outbounds[2]["streamSettings"]["sockopt"]
            .get("dialerProxy")
            .is_none());
        // A chain is sequential, not balanced: no observatory/balancer.
        assert!(generated.main.get("observatory").is_none());
        assert!(generated.main.get("burstObservatory").is_none());
        assert!(generated.main["routing"].get("balancers").is_none());
    }

    #[test]
    fn engine_input_generates_singbox_detour_chain() {
        let (active, all) = chain_graph();
        let input = build_input(
            &active,
            &all,
            None,
            BTreeMap::new(),
            None,
            &CodegenOptions::default(),
        );
        let generated = generate(CoreType::SingBox, &input).unwrap();
        let outbounds = generated.main["outbounds"].as_array().unwrap();
        assert_eq!(outbounds[0]["tag"], serde_json::json!("proxy"));
        assert_eq!(outbounds[0]["server"], serde_json::json!("192.0.2.13"));
        assert_eq!(
            outbounds[0]["detour"],
            serde_json::json!("chain-proxy-1-c2")
        );
        assert_eq!(outbounds[1]["tag"], serde_json::json!("chain-proxy-1-c2"));
        assert_eq!(
            outbounds[1]["detour"],
            serde_json::json!("chain-proxy-2-c1")
        );
        assert_eq!(outbounds[2]["tag"], serde_json::json!("chain-proxy-2-c1"));
        assert!(outbounds[2].get("detour").is_none());
    }

    #[test]
    fn client_config_export_text_contains_node_and_outbounds() {
        // RE-PROF-08: the full client-config export serializes the same
        // `generate` output the runtime plan uses. The saved/clipboard text
        // must carry the node address and an outbounds section.
        let active = leaf("export-leaf", "192.0.2.55");
        let all = vec![active.clone()];
        let input = build_input(
            &active,
            &all,
            None,
            BTreeMap::new(),
            None,
            &CodegenOptions::default(),
        );
        let generated = generate(CoreType::Xray, &input).unwrap();
        let text = serde_json::to_string_pretty(&generated.main).unwrap();
        assert!(text.contains("192.0.2.55"), "{text}");
        assert!(text.contains("\"outbounds\""), "{text}");
    }
}
