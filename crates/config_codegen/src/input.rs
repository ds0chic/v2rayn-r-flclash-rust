//! Pure input model for the Xray / sing-box generators.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `EConfigType` of the upstream model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigType {
    #[default]
    #[serde(rename = "VMess", alias = "vmess")]
    Vmess,
    #[serde(rename = "Shadowsocks", alias = "shadowsocks", alias = "ss")]
    Shadowsocks,
    #[serde(rename = "SOCKS", alias = "socks")]
    Socks,
    #[serde(rename = "HTTP", alias = "http")]
    Http,
    #[serde(rename = "VLESS", alias = "vless")]
    Vless,
    #[serde(rename = "Trojan", alias = "trojan")]
    Trojan,
    #[serde(rename = "Hysteria2", alias = "hysteria2", alias = "hy2")]
    Hysteria2,
    #[serde(rename = "TUIC", alias = "tuic")]
    Tuic,
    #[serde(rename = "WireGuard", alias = "wireguard", alias = "wg")]
    WireGuard,
    #[serde(rename = "Anytls", alias = "anytls")]
    Anytls,
    #[serde(rename = "Naive", alias = "naive")]
    Naive,
    #[serde(rename = "Outbound", alias = "outbound")]
    Outbound,
    #[serde(rename = "PolicyGroup", alias = "policy_group")]
    PolicyGroup,
    #[serde(rename = "ProxyChain", alias = "proxy_chain")]
    ProxyChain,
    #[serde(rename = "Custom", alias = "custom")]
    Custom,
}

impl ConfigType {
    /// Upstream `ConfigType.IsGroupType()`.
    pub fn is_group(self) -> bool {
        matches!(self, ConfigType::PolicyGroup | ConfigType::ProxyChain)
    }

    /// Upstream `ConfigType.IsComplexType()`.
    pub fn is_complex(self) -> bool {
        matches!(
            self,
            ConfigType::PolicyGroup | ConfigType::ProxyChain | ConfigType::Custom
        )
    }

    /// `Global.ProtocolTypes` member, `None` for group/pseudo types.
    pub fn protocol_type(self) -> Option<&'static str> {
        Some(match self {
            ConfigType::Vmess => "vmess",
            ConfigType::Shadowsocks => "shadowsocks",
            ConfigType::Socks => "socks",
            ConfigType::Http => "http",
            ConfigType::Vless => "vless",
            ConfigType::Trojan => "trojan",
            ConfigType::Hysteria2 => "hysteria2",
            ConfigType::Tuic => "tuic",
            ConfigType::WireGuard => "wireguard",
            ConfigType::Anytls => "anytls",
            ConfigType::Naive => "naive",
            ConfigType::Outbound
            | ConfigType::PolicyGroup
            | ConfigType::ProxyChain
            | ConfigType::Custom => return None,
        })
    }
}

/// `ProtocolExtraItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProtocolExtra {
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
    pub multiple_load: Option<MultipleLoad>,
    pub is_singbox_endpoint: Option<bool>,
}

/// `EMultipleLoad`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MultipleLoad {
    #[serde(rename = "LeastPing", alias = "least_ping")]
    LeastPing,
    #[serde(rename = "Fallback", alias = "fallback")]
    Fallback,
    #[serde(rename = "Random", alias = "random")]
    Random,
    #[serde(rename = "RoundRobin", alias = "round_robin")]
    RoundRobin,
    #[serde(rename = "LeastLoad", alias = "least_load")]
    LeastLoad,
}

/// `TransportExtraItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
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
}

/// One node (`ProfileItem`). String fields that are empty behave like upstream
/// `null`/empty values: they are only written when the upstream source writes
/// them unconditionally.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CodegenProfile {
    pub index_id: String,
    pub config_type: ConfigType,
    pub remarks: String,
    pub address: String,
    pub port: i32,
    pub password: String,
    pub username: String,
    pub network: String,
    pub stream_security: String,
    pub allow_insecure: bool,
    pub sni: String,
    pub alpn: String,
    pub fingerprint: String,
    pub public_key: String,
    pub short_id: String,
    pub spider_x: String,
    pub mldsa65_verify: String,
    pub mux_enabled: Option<bool>,
    pub cert: String,
    pub cert_sha: String,
    pub ech_config_list: String,
    pub verify_peer_cert_by_name: String,
    pub finalmask: Option<Value>,
    pub proto_extra: ProtocolExtra,
    pub transport_extra: TransportExtra,
    /// Raw JSON text used when `config_type == Custom` (pass-through).
    pub custom_config: Option<String>,
}

/// `CoreBasicItem`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CoreBasic {
    pub loglevel: String,
    pub log_enabled: bool,
    pub def_fingerprint: Option<String>,
    pub def_user_agent: Option<String>,
    pub send_through: Option<String>,
    pub bind_interface: Option<String>,
    pub enable_fragment: bool,
    pub enable_final_fragment: bool,
    pub enable_cache_file4_sbox: bool,
}

impl Default for CoreBasic {
    fn default() -> Self {
        Self {
            loglevel: "warning".into(),
            log_enabled: false,
            def_fingerprint: None,
            def_user_agent: None,
            send_through: None,
            bind_interface: None,
            enable_fragment: false,
            enable_final_fragment: false,
            enable_cache_file4_sbox: false,
        }
    }
}

/// `Fragment4RayItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Fragment4Ray {
    pub packets: Option<String>,
    pub lengths: Vec<String>,
    pub delays: Vec<String>,
    pub max_split: Option<String>,
}

/// `Mux4RayItem`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Mux4Ray {
    pub concurrency: i32,
    pub xudp_concurrency: i32,
    pub xudp_proxy_udp443: String,
}

impl Default for Mux4Ray {
    fn default() -> Self {
        Self {
            concurrency: 8,
            xudp_concurrency: 8,
            xudp_proxy_udp443: "reject".into(),
        }
    }
}

/// `Mux4SboxItem`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Mux4Sbox {
    pub protocol: String,
    pub max_connections: i32,
    pub padding: Option<bool>,
}

impl Default for Mux4Sbox {
    fn default() -> Self {
        Self {
            protocol: String::new(),
            max_connections: 8,
            padding: None,
        }
    }
}

/// `KcpItem`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Kcp {
    pub mtu: i32,
    pub tti: i32,
    pub uplink_capacity: i32,
    pub downlink_capacity: i32,
    pub cwnd_multiplier: i32,
    pub max_sending_window: i32,
}

impl Default for Kcp {
    fn default() -> Self {
        Self {
            mtu: 1350,
            tti: 20,
            uplink_capacity: 5,
            downlink_capacity: 20,
            cwnd_multiplier: 1,
            max_sending_window: 0,
        }
    }
}

/// `GrpcItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Grpc {
    pub idle_timeout: Option<i32>,
    pub health_check_timeout: Option<i32>,
    pub permit_without_stream: Option<bool>,
    pub initial_windows_size: Option<i32>,
}

/// `HysteriaItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Hysteria {
    pub up_mbps: Option<i32>,
    pub down_mbps: Option<i32>,
    pub hop_interval: i32,
}

/// `HappyEyeballs4RayItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HappyEyeballs4Ray {
    pub try_delay_ms: Option<i32>,
    #[serde(rename = "prioritizeIPv6")]
    pub prioritize_ipv6: Option<bool>,
    pub interleave: Option<i32>,
    pub max_concurrent_try: Option<i32>,
}

/// `GuiItem` subset needed by the generators.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Gui {
    pub enable_statistics: bool,
    pub display_real_time_speed: bool,
}

/// `Inbound[0]` (`InItem`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct InboundSettings {
    pub local_port: i32,
    pub second_local_port_enabled: bool,
    pub allow_lan_conn: bool,
    pub new_port4_lan: bool,
    pub user: String,
    pub pass: String,
    pub udp_enabled: bool,
    pub sniffing_enabled: bool,
    pub dest_override: Vec<String>,
    pub route_only: bool,
}

impl Default for InboundSettings {
    fn default() -> Self {
        Self {
            // The upstream default is 10808; tests must always override this with
            // a free port >= 11808. Keep the test-friendly default to avoid ever
            // emitting the user's live proxy port from an unset fixture.
            local_port: 11808,
            second_local_port_enabled: false,
            allow_lan_conn: false,
            new_port4_lan: true,
            user: String::new(),
            pass: String::new(),
            udp_enabled: true,
            sniffing_enabled: true,
            dest_override: vec!["http".into(), "tls".into()],
            route_only: false,
        }
    }
}

/// `TunModeItem`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TunSettings {
    pub enabled: bool,
    pub mtu: i32,
    pub ipv4_address: Option<String>,
    pub enable_ipv6_address: bool,
    pub ipv6_address: Option<String>,
    pub route_exclude_address: Vec<String>,
    pub auto_route: bool,
    pub strict_route: bool,
    pub stack: Option<String>,
    pub icmp_routing: Option<String>,
    /// Deterministic override for the platform specific interface name.
    pub name: Option<String>,
}

impl Default for TunSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            mtu: 0,
            ipv4_address: None,
            enable_ipv6_address: false,
            ipv6_address: None,
            route_exclude_address: vec![],
            auto_route: true,
            strict_route: false,
            stack: None,
            icmp_routing: None,
            name: None,
        }
    }
}

/// `RoutingBasicItem`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RoutingBasic {
    pub domain_strategy: String,
    pub domain_strategy4_singbox: Option<String>,
}

impl Default for RoutingBasic {
    fn default() -> Self {
        Self {
            domain_strategy: "IPIfNonMatch".into(),
            domain_strategy4_singbox: None,
        }
    }
}

/// Host platform, used only for deterministic interface/process naming.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    #[default]
    Windows,
    MacOS,
    Linux,
}

/// Global settings that are not part of the node.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CodegenSettings {
    pub core_basic: CoreBasic,
    pub fragment4_ray: Fragment4Ray,
    pub mux4_ray: Mux4Ray,
    pub mux4_sbox: Mux4Sbox,
    pub kcp: Kcp,
    pub grpc: Grpc,
    pub hysteria: Hysteria,
    pub happy_eyeballs4_ray: HappyEyeballs4Ray,
    pub gui: Gui,
    pub inbound: InboundSettings,
    pub tun: TunSettings,
    pub routing_basic: RoutingBasic,
    pub platform: Platform,
    /// Resolved `AppManager.StatePort` for Xray (runtime context input).
    pub state_port: i32,
    /// Resolved `AppManager.StatePort2` for sing-box (runtime context input).
    pub state_port2: i32,
    /// Directory used to resolve log file names.
    pub log_directory: String,
    /// Directory used to resolve `cache.db` and `srss/`.
    pub bin_directory: String,
    /// Synthetic `yyyy-MM-dd` used for log file names.
    pub log_date: String,
    /// Speed test URL used as observatory probe URL.
    pub speed_ping_test_url: Option<String>,
    /// Already resolved executable identifiers for TUN direct-out rules.
    pub protect_core_executables: Vec<String>,
    pub has_global_ipv6_address: bool,
    /// Override for `Global.SingboxRulesetUrl`.
    pub ruleset_url: Option<String>,
    /// Local `bin/srss/<name>.srs` files that exist (pure input, no IO).
    pub local_srs_files: BTreeSet<String>,
}

/// One routing rule (`RulesItem`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CodegenRule {
    pub enabled: bool,
    pub rule_type: RuleType,
    pub outbound_tag: String,
    pub port: Option<String>,
    pub network: Option<String>,
    pub inbound_tag: Option<Vec<String>>,
    pub domain: Option<Vec<String>>,
    pub ip: Option<Vec<String>>,
    pub protocol: Option<Vec<String>>,
    pub process: Option<Vec<String>>,
    /// Stable id used to build deterministic DNS evaluate tags.
    pub id: String,
}

impl CodegenRule {
    /// `ERuleType`.
    pub fn is_dns(&self) -> bool {
        self.rule_type == RuleType::Dns
    }

    pub fn is_routing(&self) -> bool {
        self.rule_type == RuleType::Routing
    }
}

/// `ERuleType`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleType {
    #[serde(rename = "ALL", alias = "all")]
    All,
    #[default]
    #[serde(rename = "Routing", alias = "routing")]
    Routing,
    #[serde(rename = "DNS", alias = "dns")]
    Dns,
}

/// Active routing profile (`RoutingItem`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CodegenRouting {
    pub rule_set: Vec<CodegenRule>,
    pub domain_strategy: Option<String>,
    pub domain_strategy4_singbox: Option<String>,
    /// Parsed `CustomRulesetPath4Singbox` content. Reading the file is an IO
    /// concern of the caller, the generator only sees the parsed value.
    pub custom_ruleset: Option<Value>,
}

/// Raw DNS item (`RawDnsItem`), `enabled` selects the raw/custom path.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CodegenDns {
    pub enabled: bool,
    pub normal: Option<String>,
    pub tun: Option<String>,
    pub domain_strategy4_freedom: Option<String>,
    pub domain_dns_address: Option<String>,
    pub use_system_hosts: bool,
    pub protect_domain_list: Vec<String>,
    /// Hosts read from the OS, provided by the caller as pure data.
    pub system_hosts: BTreeMap<String, String>,
    pub simple: SimpleDns,
}

/// `SimpleDNSItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SimpleDns {
    pub direct_dns: Option<String>,
    pub remote_dns: Option<String>,
    pub bootstrap_dns: Option<String>,
    pub direct_expected_ips: Option<String>,
    pub strategy4_freedom: Option<String>,
    pub strategy4_proxy: Option<String>,
    pub strategy4_proxy_dial: Option<String>,
    pub serve_stale: bool,
    pub parallel_query: bool,
    pub block_aaaa_query: bool,
    pub block_binding_query: bool,
    pub add_common_hosts: bool,
    pub use_system_hosts: bool,
    pub hosts: Option<String>,
    pub fake_ip: bool,
    pub global_fake_ip: Option<bool>,
    pub fake_ip_range: Option<String>,
    pub enable_happy_eyeballs: bool,
}

/// `FullConfigTemplateItem` for one core.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CodegenTemplate {
    pub enabled: bool,
    pub config: Option<String>,
    pub tun_config: Option<String>,
    pub add_proxy_only: bool,
    pub proxy_detour: Option<String>,
}

/// Complete input of one generation run. Pure data; no IO is performed by the
/// crate itself apart from parsing the provided JSON text.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CodegenInput {
    /// Active profile.
    pub profile: CodegenProfile,
    /// All profiles by `IndexId`, including the active one (for group children
    /// and routing `remark:` resolution).
    pub profiles: BTreeMap<String, CodegenProfile>,
    /// Custom outbound contents by `IndexId`.
    pub custom_outbound_content: BTreeMap<String, String>,
    /// Extra synthetic fixtures used by tests for template samples.
    pub tun_rules: Option<Value>,
    pub tun_singbox_rules: Option<Value>,
    pub singbox_fakeip_filter: Option<Value>,
    pub dns_v2ray_normal: Option<String>,
    pub dns_singbox_normal: Option<String>,
    pub tun_singbox_dns: Option<String>,
    pub settings: CodegenSettings,
    pub routing: Option<CodegenRouting>,
    pub dns: Option<CodegenDns>,
    pub template: Option<CodegenTemplate>,
    /// `ERuleMode` name (`Rule` / `Global` / `Direct`; `None` means `Rule`).
    /// `Global` routes everything through the proxy, `Direct` through direct.
    pub rule_mode: Option<String>,
}
