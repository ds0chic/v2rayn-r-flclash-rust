//! Bridge from the persisted domain model to the pure `config_codegen` input
//! model, plus the grouped generate entry point (T10).
//!
//! The generators stay pure (no IO); this module performs the small assembly the
//! T07/T08 notes list as caller responsibility: profile-set projection, resolved
//! ports/paths and the custom-outbound content map read by the engine.

use std::collections::{BTreeMap, BTreeSet, HashMap};

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
    /// Caller-supplied snapshot of the `bin/srss/<name>.srs` files present on
    /// disk (pure input, no IO). Entries switch the sing-box generator to
    /// `local` rule_sets; the production snapshot filler lives with the
    /// update/engine side (G-07, locked out of this card).
    pub local_srs_files: BTreeSet<String>,
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
            local_srs_files: BTreeSet::new(),
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
    let mut all_map: HashMap<String, Profile> = all
        .iter()
        .cloned()
        .map(|p| (p.index_id.clone(), p))
        .collect();
    all_map
        .entry(active.index_id.clone())
        .or_insert_with(|| active.clone());
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
        let mut projected = to_codegen_profile(profile, custom);
        apply_resolved_children(&mut projected, profile, &all_map);
        profiles.insert(profile.index_id.clone(), projected);
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
    let mut active_projected = to_codegen_profile(active, active_custom);
    apply_resolved_children(&mut active_projected, active, &all_map);
    CodegenInput {
        profile: active_projected,
        profiles,
        custom_outbound_content: outbound_contents,
        settings: opts.settings(),
        routing: Some(CodegenRouting::default()),
        dns: Some(CodegenDns::default()),
        template,
        ..Default::default()
    }
}

/// Fill a group's effective generation `ChildItems`: subscription children
/// (`SubChildItems` + `Filter`, upstream `GetSubChildProfileItems`) first, then
/// the explicit ordered `ChildItems` (upstream `GetSelectedChildProfileItems`).
///
/// The generator itself only understands explicit child ids, so this bridge
/// resolves the subscription source with the same `application::groups`
/// semantics used for validation/preview. Resolved subscription children are
/// always existing ids; an explicit id that no longer resolves is kept so the
/// generator still reports a readable dangling-reference error instead of
/// silently dropping the reference.
fn apply_resolved_children(
    projected: &mut CodegenProfile,
    profile: &Profile,
    all: &HashMap<String, Profile>,
) {
    if !crate::groups::is_group(profile.config_type) {
        return;
    }
    let mut ids: Vec<String> = crate::groups::resolve_sub_children(profile, all)
        .into_iter()
        .map(|child| child.index_id.clone())
        .collect();
    for id in crate::groups::child_index_ids(profile) {
        if !ids.iter().any(|existing| existing == &id) {
            ids.push(id);
        }
    }
    projected.proto_extra.child_items = if ids.is_empty() {
        None
    } else {
        Some(ids.join(","))
    };
    projected.proto_extra.sub_child_items = None;
    projected.proto_extra.filter = None;
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
    mut input: CodegenInput,
) -> Result<GeneratedConfigs, CodegenError> {
    let profile = CodegenProfile {
        index_id: "pre-socks".to_string(),
        config_type: CodegenConfigType::Socks,
        remarks: "pre-socks".to_string(),
        address: dial_address.to_string(),
        port: dial_port as i32,
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
    // SRS source template (`ConstItem.SrsSourceUrl`, else the upstream
    // built-in): the sing-box generator emits it as `route.rule_set[].url`.
    base.ruleset_url = Some(crate::dns::effective_srs_source(&settings.const_item));
    // Caller-supplied local snapshot; the generator picks `local` rule_sets
    // for these names without touching the network.
    base.local_srs_files = opts.local_srs_files.clone();
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
    // The generated core config and the net-host deferred discovery must agree
    // on one adapter name; otherwise the adapter is created under the core's
    // default name and discovery can never match it (TUN would time out).
    base.tun.name = Some(crate::tun_plan::DEFAULT_TUN_ADAPTER.to_string());
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

/// Map the persisted tree onto the mihomo merge options
/// (`CoreConfigClashService.GenerateClientCustomConfig`).
///
/// This is the consumer for `ClashUIItem.EnableIPv6` (FLD-CFG-129) and
/// `ClashUIItem.EnableMixinContent` (FLD-CFG-130), which gate the `ipv6`
/// rewrite and the user-Mixin merge respectively. The remaining `ClashUIItem`
/// fields (`ProxiesSorting`/`ProxiesAutoRefresh`/`*RefreshInterval`/
/// `ConnectionsColumnItem`) are UI refresh/column preferences consumed by the
/// Clash proxies/connections views, not by config generation.
pub fn mixin_options_from_app(
    settings: &domain::AppSettings,
    opts: &CodegenOptions,
) -> crate::mixin::MixinOptions {
    let inbound = settings.inbound.first();
    crate::mixin::MixinOptions {
        local_port: opts.local_port,
        state_port2: opts.state_port2,
        log_level: settings
            .core_basic_item
            .loglevel
            .clone()
            .unwrap_or_else(|| "warning".into()),
        allow_lan: inbound.map(|item| item.allow_lan_conn).unwrap_or(false),
        ipv6: settings.clash_ui_item.enable_ipv6,
        tun_enabled: settings.tun_mode_item.enable_tun,
        mixin_enabled: settings.clash_ui_item.enable_mixin_content,
    }
}

/// Mihomo native-plan merge entry (SP-24 G-06, generator side).
///
/// Pure: `raw_base_yaml` is the custom YAML text, `mixin_yaml`/`tun_yaml`
/// are caller-read file texts (IO stays with the engine). Returns the merged
/// YAML; a bad YAML is `FIELD_FORMAT` and the caller must keep the old plan
/// instead of persisting the output. Unknown keys are retained verbatim
/// (upstream `CoreConfigClashService` merge order).
///
/// Exact engine wiring (locked `engine.rs` native_custom branch): resolve the
/// raw text as today, read the mixin/tun texts, then
/// `mihomo_body_for_plan(&raw, mixin_text.as_deref(), tun_text.as_deref(),
/// &settings, opts)?`.
pub fn mihomo_body_for_plan(
    raw_base_yaml: &str,
    mixin_yaml: Option<&str>,
    tun_yaml: Option<&str>,
    settings: &domain::AppSettings,
    opts: &CodegenOptions,
) -> Result<String, domain::DomainError> {
    let mixin_opts = mixin_options_from_app(settings, opts);
    crate::mixin::generate_mihomo(raw_base_yaml, mixin_yaml, tun_yaml, &mixin_opts)
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
                // Upstream `RulesItem.RuleType` is nullable and a null rule
                // applies to both routing and DNS (the built-in templates carry
                // no `ruleType`), so only an explicit value narrows it.
                rule_type: match r.rule_type {
                    Some(domain::RuleType::Routing) => config_codegen::input::RuleType::Routing,
                    Some(domain::RuleType::Dns) => config_codegen::input::RuleType::Dns,
                    Some(domain::RuleType::All) | None => config_codegen::input::RuleType::All,
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
    fn r4_20_sub_child_policy_group_resolves_subscription_children() {
        // R4-20: an auto/region policy group stores its source in
        // `SubChildItems` + `Filter`, not in `ChildItems`. Generation must
        // resolve the subscription children (upstream
        // `GetSubChildProfileItemsByProtocolExtra`) or the group emits no
        // balancer at all.
        let mut group = crate::groups::new_group_all("sub-1", "All nodes".into());
        group.index_id = "group-all".into();
        let mut hk = leaf("h1", "192.0.2.31");
        hk.remarks = "HK-1".into();
        hk.subid = "sub-1".into();
        let mut us = leaf("u1", "192.0.2.32");
        us.remarks = "US-1".into();
        us.subid = "sub-1".into();
        let mut other = leaf("o1", "192.0.2.33");
        other.subid = "sub-2".into();
        let all = vec![group.clone(), hk, us, other];
        let input = build_input(
            &group,
            &all,
            None,
            BTreeMap::new(),
            None,
            &CodegenOptions::default(),
        );
        assert_eq!(
            input.profile.proto_extra.child_items.as_deref(),
            Some("h1,u1"),
            "subscription children resolve in index-id order"
        );
        let generated = generate(CoreType::Xray, &input).unwrap();
        let text = serde_json::to_string(&generated.main).unwrap();
        assert!(text.contains("192.0.2.31"), "{text}");
        assert!(text.contains("192.0.2.32"), "{text}");
        assert!(
            !text.contains("192.0.2.33"),
            "another subscription's node must be dropped: {text}"
        );
        assert_eq!(
            generated.main["routing"]["balancers"][0]["tag"],
            serde_json::json!("proxy-balancer")
        );
    }

    #[test]
    fn r4_20_sub_child_proxy_chain_generates_detour_order() {
        let mut chain = crate::groups::new_group(
            ConfigType::ProxyChain,
            "chain".into(),
            MultipleLoad::LeastPing,
        );
        chain.index_id = "chain-1".into();
        chain.subid = "sub-1".into();
        chain.proto_extra.sub_child_items = Some("sub-1".into());
        let mut c1 = leaf("c1", "192.0.2.11");
        c1.remarks = "c1".into();
        c1.subid = "sub-1".into();
        let mut c2 = leaf("c2", "192.0.2.12");
        c2.remarks = "c2".into();
        c2.subid = "sub-1".into();
        let all = vec![chain.clone(), c1, c2];
        let input = build_input(
            &chain,
            &all,
            None,
            BTreeMap::new(),
            None,
            &CodegenOptions::default(),
        );
        let generated = generate(CoreType::Xray, &input).unwrap();
        let outbounds = generated.main["outbounds"].as_array().unwrap();
        assert_eq!(outbounds[0]["tag"], serde_json::json!("proxy"));
        assert_eq!(
            outbounds[0]["settings"]["address"],
            serde_json::json!("192.0.2.12")
        );
        assert_eq!(
            outbounds[0]["streamSettings"]["sockopt"]["dialerProxy"],
            serde_json::json!("chain-proxy-1-c1")
        );
        assert_eq!(outbounds[1]["tag"], serde_json::json!("chain-proxy-1-c1"));
    }

    #[test]
    fn r4_20_missing_explicit_child_reports_readable_dangling() {
        let mut group =
            crate::groups::new_group(ConfigType::PolicyGroup, "g".into(), MultipleLoad::LeastPing);
        group.index_id = "g".into();
        group.proto_extra.child_items = Some("missing".into());
        let all = vec![group.clone()];
        let input = build_input(
            &group,
            &all,
            None,
            BTreeMap::new(),
            None,
            &CodegenOptions::default(),
        );
        let err = generate(CoreType::Xray, &input).unwrap_err();
        assert_eq!(err.code, "dangling_reference");
        assert!(err.message.contains("missing"), "{}", err.message);
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

    #[test]
    fn core_basic_item_reaches_generated_config() {
        // R4-13.S05: every CoreBasicItem field is projected onto the generator
        // settings and consumed by the real xray/sing-box output, so a saved
        // change is not "save only".
        let mut settings = domain::AppSettings::default();
        settings.core_basic_item.loglevel = Some("debug".into());
        settings.core_basic_item.log_enabled = true;
        settings.core_basic_item.enable_fragment = true;
        settings.core_basic_item.enable_final_fragment = true;
        settings.core_basic_item.enable_cache_file4_sbox = true;

        let codegen_settings = settings_from_app(&settings, &CodegenOptions::default());
        assert_eq!(codegen_settings.core_basic.loglevel, "debug");
        assert!(codegen_settings.core_basic.log_enabled);
        assert!(codegen_settings.core_basic.enable_fragment);
        assert!(codegen_settings.core_basic.enable_final_fragment);
        assert!(codegen_settings.core_basic.enable_cache_file4_sbox);

        let active = leaf("core-basic", "192.0.2.9");
        let mut input = build_input(
            &active,
            std::slice::from_ref(&active),
            None,
            BTreeMap::new(),
            None,
            &CodegenOptions::default(),
        );
        input.settings = codegen_settings;

        let xray = generate(CoreType::Xray, &input).unwrap();
        assert_eq!(xray.main["log"]["loglevel"], serde_json::json!("debug"));
        assert!(xray.main["log"]["error"].is_string());

        let singbox = generate(CoreType::SingBox, &input).unwrap();
        assert_eq!(singbox.main["log"]["level"], serde_json::json!("debug"));
        assert_eq!(
            singbox.main["experimental"]["cache_file"]["enabled"],
            serde_json::json!(true)
        );
    }

    #[test]
    fn r4_13_s22_tun_fields_reach_generated_config() {
        // R4-13.S22: every `TunModeItem` routing/KCP field is projected onto the
        // generator settings and consumed by the real sing-box / xray TUN
        // inbound, so a saved change is not "save only".
        let mut settings = domain::AppSettings::default();
        settings.tun_mode_item.enable_tun = true;
        settings.tun_mode_item.auto_route = false;
        settings.tun_mode_item.strict_route = true;
        settings.tun_mode_item.stack = Some("system".into());
        settings.tun_mode_item.icmp_routing = Some("direct".into());
        settings.tun_mode_item.mtu = 1400;
        settings.tun_mode_item.ipv4_address = Some("172.18.0.1/30".into());
        settings.tun_mode_item.enable_ipv6_address = true;
        settings.tun_mode_item.ipv6_address = Some("fd00::1/64".into());
        settings.tun_mode_item.route_exclude_address = Some(vec!["10.0.0.0/8".into()]);

        let codegen_settings = settings_from_app(&settings, &CodegenOptions::default());
        assert!(codegen_settings.tun.enabled);
        assert_eq!(codegen_settings.tun.mtu, 1400);
        assert!(!codegen_settings.tun.auto_route);
        assert!(codegen_settings.tun.strict_route);
        assert_eq!(codegen_settings.tun.stack.as_deref(), Some("system"));
        assert_eq!(codegen_settings.tun.icmp_routing.as_deref(), Some("direct"));
        assert_eq!(
            codegen_settings.tun.route_exclude_address,
            vec!["10.0.0.0/8".to_string()]
        );
        assert!(codegen_settings.tun.enable_ipv6_address);

        let active = leaf("tun-fields", "192.0.2.10");
        let mut input = build_input(
            &active,
            std::slice::from_ref(&active),
            None,
            BTreeMap::new(),
            None,
            &CodegenOptions::default(),
        );
        input.settings = codegen_settings;

        let singbox = generate(CoreType::SingBox, &input).unwrap();
        let inbounds = singbox.main["inbounds"].as_array().expect("inbounds array");
        let tun = inbounds
            .iter()
            .find(|i| i["type"] == serde_json::json!("tun"))
            .expect("sing-box tun inbound");
        assert_eq!(tun["mtu"], serde_json::json!(1400));
        assert_eq!(tun["auto_route"], serde_json::json!(false));
        assert_eq!(tun["strict_route"], serde_json::json!(true));
        assert_eq!(tun["stack"], serde_json::json!("system"));
        // The adapter name is shared with net-host deferred discovery; a
        // divergent name makes TUN discovery time out.
        assert_eq!(tun["interface_name"], serde_json::json!("v2rayn-tun"));

        let xray = generate(CoreType::Xray, &input).unwrap();
        let xray_inbounds = xray.main["inbounds"].as_array().expect("inbounds array");
        let xray_tun = xray_inbounds
            .iter()
            .find(|i| i["protocol"] == serde_json::json!("tun"))
            .expect("xray tun inbound");
        assert_eq!(
            xray_tun["settings"]["name"],
            serde_json::json!("v2rayn-tun")
        );
    }

    // R4-13.S02: ClashUIItem.EnableIPv6 / EnableMixinContent reach the mihomo
    // merge options and really change the merged YAML.
    #[test]
    fn r4_13_s02_clash_ui_item_reaches_mihomo_merge() {
        let mut settings = domain::AppSettings::default();
        settings.clash_ui_item.enable_ipv6 = true;
        settings.clash_ui_item.enable_mixin_content = true;
        settings.inbound[0].allow_lan_conn = true;
        let opts = mixin_options_from_app(&settings, &CodegenOptions::default());
        assert!(opts.ipv6, "EnableIPv6 must reach the mihomo merge");
        assert!(
            opts.mixin_enabled,
            "EnableMixinContent gates the Mixin merge"
        );
        assert!(opts.allow_lan, "Inbound.AllowLANConn reaches allow-lan");
        assert_eq!(opts.local_port, 11808, "never the live 10808 port");

        let base = "port: 7890\nmode: rule\n";
        let mixin = "rules:\n  - MATCH,DIRECT\n";
        let on = crate::mixin::generate_mihomo(base, Some(mixin), None, &opts).unwrap();
        assert!(on.contains("ipv6: true"), "{on}");
        assert!(on.contains("MATCH,DIRECT"), "{on}");

        settings.clash_ui_item.enable_mixin_content = false;
        let off = mixin_options_from_app(&settings, &CodegenOptions::default());
        let off_yaml = crate::mixin::generate_mihomo(base, Some(mixin), None, &off).unwrap();
        assert!(
            !off_yaml.contains("MATCH,DIRECT"),
            "a saved EnableMixinContent=false must skip the Mixin merge: {off_yaml}"
        );
    }

    // R4-13.S06: CoreTypeItem rows reach the per-config-type core selection.
    #[test]
    fn r4_13_s06_core_type_item_reaches_core_selection() {
        let mut settings = domain::AppSettings::default();
        assert_eq!(settings.core_for(ConfigType::Vless), CoreType::Xray);
        settings.init_core_type_items();
        if let Some(items) = settings.core_type_item.as_mut() {
            if let Some(row) = items
                .iter_mut()
                .find(|b| b.config_type == ConfigType::Vless)
            {
                row.core_type = CoreType::SingBox;
            }
        }
        assert_eq!(settings.core_for(ConfigType::Vless), CoreType::SingBox);
        assert_eq!(settings.core_for(ConfigType::Vmess), CoreType::Xray);
    }

    // R4-13.S09: GrpcItem reaches the xray/sing-box gRPC transport settings.
    #[test]
    fn r4_13_s09_grpc_item_reaches_codegen() {
        let mut settings = domain::AppSettings::default();
        settings.grpc_item.idle_timeout = Some(11);
        settings.grpc_item.health_check_timeout = Some(7);
        settings.grpc_item.permit_without_stream = Some(true);
        settings.grpc_item.initial_windows_size = Some(65535);
        let cs = settings_from_app(&settings, &CodegenOptions::default());
        assert_eq!(cs.grpc.idle_timeout, Some(11));
        assert_eq!(cs.grpc.health_check_timeout, Some(7));
        assert_eq!(cs.grpc.permit_without_stream, Some(true));
        assert_eq!(cs.grpc.initial_windows_size, Some(65535));
    }

    // R4-13.S10: GuiItem statistics/speed reach the generator's experimental
    // statistics consumer.
    #[test]
    fn r4_13_s10_gui_item_reaches_codegen() {
        let mut settings = domain::AppSettings::default();
        settings.gui_item.enable_statistics = true;
        settings.gui_item.display_real_time_speed = true;
        let cs = settings_from_app(&settings, &CodegenOptions::default());
        assert!(cs.gui.enable_statistics);
        assert!(cs.gui.display_real_time_speed);
    }

    // R4-13.S11: HappyEyeballs4RayItem reaches the xray DNS happy-eyeballs block.
    #[test]
    fn r4_13_s11_happy_eyeballs_reaches_codegen() {
        let mut settings = domain::AppSettings::default();
        settings.happy_eyeballs4_ray_item.try_delay_ms = Some(300);
        settings.happy_eyeballs4_ray_item.prioritize_ipv6 = Some(true);
        settings.happy_eyeballs4_ray_item.interleave = Some(2);
        settings.happy_eyeballs4_ray_item.max_concurrent_try = Some(3);
        let cs = settings_from_app(&settings, &CodegenOptions::default());
        assert_eq!(cs.happy_eyeballs4_ray.try_delay_ms, Some(300));
        assert_eq!(cs.happy_eyeballs4_ray.prioritize_ipv6, Some(true));
        assert_eq!(cs.happy_eyeballs4_ray.interleave, Some(2));
        assert_eq!(cs.happy_eyeballs4_ray.max_concurrent_try, Some(3));
    }

    // R4-13.S12: HysteriaItem reaches the hysteria2 up/down/hop parameters.
    #[test]
    fn r4_13_s12_hysteria_reaches_codegen() {
        let mut settings = domain::AppSettings::default();
        settings.hysteria_item.up_mbps = 55;
        settings.hysteria_item.down_mbps = 66;
        settings.hysteria_item.hop_interval = 42;
        let cs = settings_from_app(&settings, &CodegenOptions::default());
        assert_eq!(cs.hysteria.up_mbps, Some(55));
        assert_eq!(cs.hysteria.down_mbps, Some(66));
        assert_eq!(cs.hysteria.hop_interval, 42);
    }

    // R4-13.S13: every Inbound listener field except the caller-owned local
    // port reaches the generated inbound settings.
    #[test]
    fn r4_13_s13_inbound_reaches_codegen() {
        let mut settings = domain::AppSettings::default();
        let inbound = &mut settings.inbound[0];
        inbound.second_local_port_enabled = true;
        inbound.allow_lan_conn = true;
        inbound.new_port4_lan = true;
        inbound.user = "u".into();
        inbound.pass = "p".into();
        inbound.udp_enabled = false;
        inbound.sniffing_enabled = false;
        inbound.dest_override = Some(vec!["tls".into()]);
        inbound.route_only = true;
        inbound.protocol = domain::InboundProtocol::Mixed;
        let cs = settings_from_app(&settings, &CodegenOptions::default());
        assert!(cs.inbound.second_local_port_enabled);
        assert!(cs.inbound.allow_lan_conn);
        assert!(cs.inbound.new_port4_lan);
        assert_eq!(cs.inbound.user, "u");
        assert_eq!(cs.inbound.pass, "p");
        assert!(!cs.inbound.udp_enabled);
        assert!(!cs.inbound.sniffing_enabled);
        assert_eq!(cs.inbound.dest_override, vec!["tls".to_string()]);
        assert!(cs.inbound.route_only);
        assert_eq!(cs.inbound.protocol, "mixed");
    }

    // R4-13.S14: KcpItem reaches the xray kcp transport and clamps cwnd.
    #[test]
    fn r4_13_s14_kcp_item_reaches_codegen() {
        let mut settings = domain::AppSettings::default();
        settings.kcp_item.mtu = 1400;
        settings.kcp_item.tti = 40;
        settings.kcp_item.uplink_capacity = 15;
        settings.kcp_item.downlink_capacity = 90;
        settings.kcp_item.cwnd_multiplier = 3;
        settings.kcp_item.max_sending_window = 1024;
        let cs = settings_from_app(&settings, &CodegenOptions::default());
        assert_eq!(cs.kcp.mtu, 1400);
        assert_eq!(cs.kcp.tti, 40);
        assert_eq!(cs.kcp.uplink_capacity, 15);
        assert_eq!(cs.kcp.downlink_capacity, 90);
        assert_eq!(cs.kcp.cwnd_multiplier, 3);
        assert_eq!(cs.kcp.max_sending_window, 1024);

        settings.kcp_item.cwnd_multiplier = 0;
        let clamped = settings_from_app(&settings, &CodegenOptions::default());
        assert_eq!(clamped.kcp.cwnd_multiplier, 1, "cwnd >= 1 correction");
    }

    // R4-13.S16: Mux4RayItem reaches the xray mux, with frozen null fallbacks.
    #[test]
    fn r4_13_s16_mux4_ray_reaches_codegen() {
        let mut settings = domain::AppSettings::default();
        settings.mux4_ray_item.concurrency = Some(4);
        settings.mux4_ray_item.xudp_concurrency = Some(9);
        settings.mux4_ray_item.xudp_proxy_udp443 = Some("skip".into());
        let cs = settings_from_app(&settings, &CodegenOptions::default());
        assert_eq!(cs.mux4_ray.concurrency, 4);
        assert_eq!(cs.mux4_ray.xudp_concurrency, 9);
        assert_eq!(cs.mux4_ray.xudp_proxy_udp443, "skip");

        settings.mux4_ray_item.concurrency = None;
        settings.mux4_ray_item.xudp_concurrency = None;
        settings.mux4_ray_item.xudp_proxy_udp443 = None;
        let fallback = settings_from_app(&settings, &CodegenOptions::default());
        assert_eq!(fallback.mux4_ray.concurrency, 8);
        assert_eq!(fallback.mux4_ray.xudp_concurrency, 16);
        assert_eq!(fallback.mux4_ray.xudp_proxy_udp443, "reject");
    }

    // R4-13.S17: Mux4SboxItem reaches the sing-box mux.
    #[test]
    fn r4_13_s17_mux4_sbox_reaches_codegen() {
        let mut settings = domain::AppSettings::default();
        settings.mux4_sbox_item.protocol = Some("smux".into());
        settings.mux4_sbox_item.max_connections = 12;
        settings.mux4_sbox_item.padding = Some(true);
        let cs = settings_from_app(&settings, &CodegenOptions::default());
        assert_eq!(cs.mux4_sbox.protocol, "smux");
        assert_eq!(cs.mux4_sbox.max_connections, 12);
        assert_eq!(cs.mux4_sbox.padding, Some(true));

        settings.mux4_sbox_item.protocol = None;
        let fallback = settings_from_app(&settings, &CodegenOptions::default());
        assert_eq!(fallback.mux4_sbox.protocol, "h2mux");
    }

    // R4-13.S18: RoutingBasicItem reaches the xray/sing-box routing strategy.
    #[test]
    fn r4_13_s18_routing_basic_reaches_codegen() {
        let mut settings = domain::AppSettings::default();
        settings.routing_basic_item.domain_strategy = Some("IPIfNonMatch".into());
        settings.routing_basic_item.domain_strategy4_singbox = Some("prefer_ipv4".into());
        let cs = settings_from_app(&settings, &CodegenOptions::default());
        assert_eq!(cs.routing_basic.domain_strategy, "IPIfNonMatch");
        assert_eq!(
            cs.routing_basic.domain_strategy4_singbox.as_deref(),
            Some("prefer_ipv4")
        );

        settings.routing_basic_item.domain_strategy = None;
        let fallback = settings_from_app(&settings, &CodegenOptions::default());
        assert_eq!(fallback.routing_basic.domain_strategy, "AsIs");
    }

    // R4-13.S19: SimpleDNSItem reaches the generator's SimpleDns block.
    #[test]
    fn r4_13_s19_simple_dns_reaches_codegen() {
        let mut settings = domain::AppSettings::default();
        let simple = &mut settings.simple_dns_item;
        simple.direct_dns = Some("1.1.1.1".into());
        simple.remote_dns = Some("https://dns.example/dns-query".into());
        simple.bootstrap_dns = Some("8.8.8.8".into());
        simple.fake_ip = Some(true);
        simple.global_fake_ip = Some(false);
        simple.fake_ip_range = Some("198.18.0.0/15".into());
        simple.serve_stale = Some(true);
        simple.parallel_query = Some(true);
        simple.enable_happy_eyeballs = Some(true);
        let dns = dns_to_codegen(None, &settings.simple_dns_item, BTreeMap::new(), Vec::new());
        assert_eq!(dns.simple.direct_dns.as_deref(), Some("1.1.1.1"));
        assert_eq!(
            dns.simple.remote_dns.as_deref(),
            Some("https://dns.example/dns-query")
        );
        assert_eq!(dns.simple.bootstrap_dns.as_deref(), Some("8.8.8.8"));
        assert!(dns.simple.fake_ip);
        assert_eq!(dns.simple.global_fake_ip, Some(false));
        assert_eq!(dns.simple.fake_ip_range.as_deref(), Some("198.18.0.0/15"));
        assert!(dns.simple.serve_stale);
        assert!(dns.simple.parallel_query);
        assert!(dns.simple.enable_happy_eyeballs);
    }

    // R4-13.S20: SpeedTestItem reaches the speedtest engine parameters with the
    // upstream corrections (<10 timeout/concurrency and blank URL fallbacks).
    #[test]
    fn r4_13_s20_speed_test_reaches_engine() {
        let item = domain::SpeedTestItem {
            speed_test_timeout: 3,
            mixed_concurrency_count: 2,
            speed_test_url: Some("   ".into()),
            speed_ping_test_url: Some("  ".into()),
            udp_test_target: Some("ntp:pool.ntp.org".into()),
            speed_test_page_size: Some(5),
            speed_test_delay_interval: Some(2),
            ..Default::default()
        };
        let st = crate::speedtest::SpeedTestSettings::from_item(&item);
        assert_eq!(st.timeout.as_secs(), 10, "timeout correction >= 10");
        assert_eq!(st.mixed_concurrency, 10, "concurrency correction >= 10");
        assert!(
            !st.speed_test_url.trim().is_empty(),
            "blank URL falls back to the frozen default"
        );
        assert_eq!(st.page_size, 5);
        assert_eq!(st.delay_interval.as_secs(), 2);
        assert_eq!(st.udp_test_target.as_deref(), Some("ntp:pool.ntp.org"));
    }

    // SP-24 G-06: the mihomo merge is consumable from the generator side.
    // The engine's native_custom branch (locked) must call
    // `mihomo_body_for_plan`; a bad YAML is a hard error so the caller keeps
    // the old plan instead of persisting a corrupt config.
    #[test]
    fn sp24_mihomo_body_for_plan_merges_and_rejects() {
        let mut settings = domain::AppSettings::default();
        settings.clash_ui_item.enable_ipv6 = true;
        settings.clash_ui_item.enable_mixin_content = true;
        let opts = CodegenOptions::default();
        let base = "port: 7890\nmode: rule\n";
        let mixin = "rules:\n  - MATCH,DIRECT\nunknown-kept: 42\n";
        let merged = mihomo_body_for_plan(base, Some(mixin), None, &settings, &opts).unwrap();
        assert!(merged.contains("ipv6: true"), "{merged}");
        assert!(merged.contains("MATCH,DIRECT"), "{merged}");
        assert!(merged.contains("unknown-kept"), "{merged}");
        assert!(!merged.contains("10808"), "{merged}");

        let bad_base = mihomo_body_for_plan("- just\n- a\n- list\n", None, None, &settings, &opts)
            .unwrap_err();
        assert_eq!(bad_base.code, domain::codes::FIELD_FORMAT);
        let bad_mixin = mihomo_body_for_plan(
            base,
            Some("rules:\n\t- tab-indent\n"),
            None,
            &settings,
            &opts,
        )
        .unwrap_err();
        assert_eq!(bad_mixin.code, domain::codes::FIELD_FORMAT);

        settings.clash_ui_item.enable_mixin_content = false;
        let off = mihomo_body_for_plan(base, Some(mixin), None, &settings, &opts).unwrap();
        assert!(!off.contains("MATCH,DIRECT"), "{off}");
    }

    // SP-24 G-07 (generator side): the stored SRS source template reaches the
    // sing-box rule_set URL; unset falls back to the upstream built-in.
    #[test]
    fn sp24_srs_source_reaches_codegen_settings() {
        let mut settings = domain::AppSettings::default();
        settings.const_item.srs_source_url = Some("https://mirror.example/srs-{0}/{1}.srs".into());
        let cs = settings_from_app(&settings, &CodegenOptions::default());
        assert_eq!(
            cs.ruleset_url.as_deref(),
            Some("https://mirror.example/srs-{0}/{1}.srs")
        );

        let fallback =
            settings_from_app(&domain::AppSettings::default(), &CodegenOptions::default());
        assert_eq!(
            fallback.ruleset_url.as_deref(),
            Some(crate::dns::BUILTIN_SRS_URL)
        );
    }

    // SP-24 G-02 end to end (generator side): a saved range string passes
    // validation and reaches the wire as the first int (raw `1-3` kept).
    #[test]
    fn sp24_saved_range_max_split_reaches_wire_first_int() {
        let mut settings = domain::AppSettings::default();
        settings.core_basic_item.enable_final_fragment = true;
        settings.fragment4_ray_item.as_mut().unwrap().max_split = Some("1-3".into());
        assert!(crate::settings::validate_settings(&settings).is_ok());
        let active = leaf("frag-leaf", "192.0.2.72");
        let mut input = build_input(
            &active,
            std::slice::from_ref(&active),
            None,
            BTreeMap::new(),
            None,
            &CodegenOptions::default(),
        );
        input.settings = settings_from_app(&settings, &CodegenOptions::default());
        assert_eq!(
            input.settings.fragment4_ray.max_split.as_deref(),
            Some("1-3"),
            "raw range string is preserved, not collapsed to 1"
        );
        let generated = generate(CoreType::Xray, &input).unwrap();
        assert_eq!(
            generated.main["outbounds"][0]["streamSettings"]["finalmask"]["tcp"][0]["settings"]
                ["maxSplit"],
            serde_json::json!(1)
        );
    }

    // SP-24 G-07 (generator side): a caller-supplied local snapshot selects
    // `local` rule_sets. The production snapshot filler is still missing
    // (update/network + engine are locked); this proves the consumer end.
    #[test]
    fn sp24_local_srs_snapshot_selects_local_ruleset() {
        let mut opts = CodegenOptions::default();
        opts.local_srs_files.insert("geosite-google".into());
        let active = leaf("srs-leaf", "192.0.2.71");
        let mut input = build_input(
            &active,
            std::slice::from_ref(&active),
            None,
            BTreeMap::new(),
            None,
            &opts,
        );
        input.settings = settings_from_app(&domain::AppSettings::default(), &opts);
        assert!(input.settings.local_srs_files.contains("geosite-google"));
        input.routing = Some(CodegenRouting {
            rule_set: vec![config_codegen::input::CodegenRule {
                enabled: true,
                rule_type: config_codegen::input::RuleType::Routing,
                outbound_tag: "proxy".into(),
                domain: Some(vec!["geosite:google".into()]),
                ..Default::default()
            }],
            ..Default::default()
        });
        let generated = generate(CoreType::SingBox, &input).unwrap();
        let main = &generated.main;
        assert_eq!(
            main["route"]["rule_set"][0]["type"],
            serde_json::json!("local")
        );
        assert_eq!(
            main["route"]["rule_set"][0]["path"],
            serde_json::json!("bin/srss/geosite-google.srs")
        );
    }

    #[test]
    fn rules_without_rule_type_apply_to_routing_and_dns() {
        let profile = domain::RoutingProfile {
            rule_set: r#"[
                {"outbound_tag":"direct","domain":["geosite:cn"],"enabled":true},
                {"outbound_tag":"proxy","domain":["a.example"],"enabled":true,"rule_type":1},
                {"outbound_tag":"proxy","domain":["b.example"],"enabled":true,"rule_type":2}
            ]"#
            .into(),
            ..Default::default()
        };
        let rules = routing_to_codegen(&profile, None).rule_set;
        let kinds: Vec<_> = rules.iter().map(|r| r.rule_type).collect();
        assert_eq!(
            kinds,
            [
                config_codegen::input::RuleType::All,
                config_codegen::input::RuleType::Routing,
                config_codegen::input::RuleType::Dns,
            ]
        );
        assert!(!rules[0].is_routing() && !rules[0].is_dns());
    }
}
