//! Application settings tree (`guiNConfig.json`).
//!
//! Full `Config` root plus its 25 `ConfigItems` sub-classes. Persistence uses
//! the exact upstream JSON shape: PascalCase keys (with the upstream acronym
//! spellings such as `IPv4Address`/`FakeIP`), enums as integers, and `null`
//! written for absent references. Unknown keys survive at every level through
//! `#[serde(flatten)]`, and "explicit null" / "empty string" stay
//! distinguishable because nullable strings are modelled as `Option<String>`.
//!
//! Defaults follow the three upstream layers (`ConfigHandler.LoadConfig`,
//! `compat/fields.settings.yaml`):
//!   1. C# property initializers (`#[serde(default = ...)]` on the field);
//!   2. `LoadConfig` object-level synthesis + corrections ([`AppSettings::apply_load_defaults`]);
//!   3. runtime/generator fallbacks (e.g. empty `Stack` -> `gvisor`), which are
//!      *not* materialised into storage and therefore stay out of this module.
//!
//! Each nested field carries its own serde default (the upstream property
//! initializer or the CLR type default). A field that is merely absent from a
//! *present* object therefore stays null/zero/empty, exactly like
//! `System.Text.Json` over a constructed CLR object. The `LoadConfig`
//! object-level values are supplied by each group's `Default` impl, which the
//! root uses when the whole group is missing.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::entities::{
    ColumnDefinition, CoreTypeBinding, GlobalHotkey, InboundListener, WindowState,
};
use crate::enums::{ConfigType, CoreType, GirdOrientation, SysProxyType};
use crate::error::{codes, DomainError};
use crate::profile::ExtraMap;

/// Change-propagation class of a persisted setting (upstream `apply_timing`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyTiming {
    /// The UI applies the change immediately.
    Immediate,
    /// Storage only; picked up on the next relevant operation.
    Save,
    /// Requires a kernel reload.
    RestartCore,
    /// Requires an application restart.
    RestartApp,
    /// Read on the next launch.
    NextLaunch,
}

impl ApplyTiming {
    pub const fn as_str(self) -> &'static str {
        match self {
            ApplyTiming::Immediate => "immediate",
            ApplyTiming::Save => "save",
            ApplyTiming::RestartCore => "restart_core",
            ApplyTiming::RestartApp => "restart_app",
            ApplyTiming::NextLaunch => "next_launch",
        }
    }

    pub const fn needs_core_restart(self) -> bool {
        matches!(self, ApplyTiming::RestartCore)
    }

    pub const fn needs_app_restart(self) -> bool {
        matches!(self, ApplyTiming::RestartApp)
    }
}

/// Toplevel settings field groups that can be saved independently.
pub const SETTINGS_GROUPS: &[&str] = &[
    "IndexId",
    "SubIndexId",
    "CoreBasicItem",
    "TunModeItem",
    "KcpItem",
    "GrpcItem",
    "RoutingBasicItem",
    "GuiItem",
    "MsgUIItem",
    "UiItem",
    "ConstItem",
    "SpeedTestItem",
    "Mux4RayItem",
    "Mux4SboxItem",
    "HysteriaItem",
    "ClashUIItem",
    "SystemProxyItem",
    "WebDavItem",
    "CheckUpdateItem",
    "Fragment4RayItem",
    "Inbound",
    "GlobalHotkeys",
    "CoreTypeItem",
    "SimpleDNSItem",
    "HappyEyeballs4RayItem",
];

pub fn is_settings_group(key: &str) -> bool {
    SETTINGS_GROUPS.contains(&key)
}

/// The eight `ConfigType`s that have a core-selection control upstream
/// (`OptionSettingViewModel.CoreType1..7` + `CoreType9`).
pub const CORE_TYPE_CONTROLS: [ConfigType; 8] = [
    ConfigType::Vmess,
    ConfigType::Custom,
    ConfigType::Shadowsocks,
    ConfigType::Socks,
    ConfigType::Vless,
    ConfigType::Trojan,
    ConfigType::Hysteria2,
    ConfigType::WireGuard,
];

const DEFAULT_LANGUAGE: &str = "zh-Hans";
const ROUTING_DOMAIN_STRATEGY: &str = "AsIs";
const ROOT_CERT_PROVIDER: &str = "system";
const SINGBOX_MUX: &str = "h2mux";
const SPEED_TEST_URL: &str = "https://cachefly.cachefly.net/50mb.test";
const SPEED_PING_TEST_URL: &str = "https://www.google.com/generate_204";
const UDP_TEST_TARGET: &str = "ntp:pool.ntp.org";
const DIRECT_DNS: &str = "119.29.29.29";
const REMOTE_DNS: &str = "https://cloudflare-dns.com/dns-query";
const FAKE_IP_RANGE: &str = "198.18.0.0/15";
const TUN_ICMP_ROUTING: &str = "rule";
const SYSTEM_PROXY_EXCEPTIONS_WINDOWS: &str = "localhost;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.20.*;172.21.*;172.22.*;172.23.*;172.24.*;172.25.*;172.26.*;172.27.*;172.28.*;172.29.*;172.30.*;172.31.*;192.168.*";
/// Upstream `Global.SystemProxyExceptionsLinux` (used on non-Windows hosts).
const SYSTEM_PROXY_EXCEPTIONS_LINUX: &str = "localhost,127.0.0.0/8,::1";

/// Platform default for `SystemProxyItem.SystemProxyExceptions`, mirroring
/// upstream (Windows vs Linux strings in `Global.cs`).
pub fn default_system_proxy_exceptions() -> String {
    if cfg!(windows) {
        SYSTEM_PROXY_EXCEPTIONS_WINDOWS.to_string()
    } else {
        SYSTEM_PROXY_EXCEPTIONS_LINUX.to_string()
    }
}

fn default_true() -> bool {
    true
}

fn default_tray_limit() -> i32 {
    20
}

fn default_hop_interval() -> i32 {
    30
}

fn default_two() -> i32 {
    2
}

fn default_inbound_list() -> Vec<InboundListener> {
    vec![default_inbound()]
}

fn default_inbound() -> InboundListener {
    InboundListener {
        local_port: 10808,
        protocol: crate::enums::InboundProtocol::Socks,
        udp_enabled: true,
        sniffing_enabled: true,
        dest_override: Some(vec!["http".to_string(), "tls".to_string()]),
        route_only: false,
        allow_lan_conn: false,
        new_port4_lan: false,
        user: String::new(),
        pass: String::new(),
        second_local_port_enabled: false,
        extra: ExtraMap::new(),
    }
}

/// Treat an explicit `null` exactly like a missing value for object groups,
/// matching `LoadConfig`'s `config.X ??= new ...`.
fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

fn non_empty(value: &Option<String>) -> bool {
    value.as_deref().is_some_and(|v| !v.is_empty())
}

/// Redact double-quoted scalars from a serde error message so a structured
/// corruption diagnostic never echoes raw config values (e.g. a mistyped
/// secret) back to logs or the UI.
fn sanitize_serde_detail(error: &serde_json::Error) -> String {
    let text = error.to_string();
    let mut out = String::with_capacity(text.len());
    let mut in_quotes = false;
    for ch in text.chars() {
        if ch == '"' {
            if in_quotes {
                out.push('?');
            }
            out.push(ch);
            in_quotes = !in_quotes;
        } else if in_quotes {
            // Skip the quoted scalar itself.
        } else {
            out.push(ch);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// ConfigItems
// ---------------------------------------------------------------------------

/// `CoreBasicItem` (9 properties).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CoreBasicItem {
    #[serde(default)]
    pub log_enabled: bool,
    #[serde(default)]
    pub loglevel: Option<String>,
    #[serde(default)]
    pub def_fingerprint: Option<String>,
    #[serde(default)]
    pub def_user_agent: Option<String>,
    #[serde(default)]
    pub send_through: Option<String>,
    #[serde(default)]
    pub bind_interface: Option<String>,
    #[serde(default)]
    pub enable_fragment: bool,
    #[serde(default)]
    pub enable_final_fragment: bool,
    #[serde(default = "default_true")]
    pub enable_cache_file4_sbox: bool,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

impl Default for CoreBasicItem {
    /// The object `LoadConfig` creates when `CoreBasicItem` is absent.
    fn default() -> Self {
        Self {
            log_enabled: false,
            loglevel: Some("warning".to_string()),
            def_fingerprint: None,
            def_user_agent: None,
            send_through: None,
            bind_interface: None,
            enable_fragment: false,
            enable_final_fragment: false,
            enable_cache_file4_sbox: true,
            extra: ExtraMap::new(),
        }
    }
}

/// `TunModeItem` (11 properties).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TunModeItem {
    #[serde(default)]
    pub enable_tun: bool,
    #[serde(default = "default_true")]
    pub auto_route: bool,
    #[serde(default = "default_true")]
    pub strict_route: bool,
    #[serde(default)]
    pub stack: Option<String>,
    #[serde(default)]
    pub mtu: i32,
    #[serde(rename = "EnableIPv6Address", default)]
    pub enable_ipv6_address: bool,
    #[serde(default)]
    pub icmp_routing: Option<String>,
    #[serde(default = "default_true")]
    pub enable_legacy_protect: bool,
    #[serde(default)]
    pub route_exclude_address: Option<Vec<String>>,
    #[serde(rename = "IPv4Address", default)]
    pub ipv4_address: Option<String>,
    #[serde(rename = "IPv6Address", default)]
    pub ipv6_address: Option<String>,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

impl Default for TunModeItem {
    fn default() -> Self {
        Self {
            enable_tun: false,
            auto_route: true,
            strict_route: true,
            stack: None,
            mtu: 9000,
            enable_ipv6_address: false,
            icmp_routing: Some(TUN_ICMP_ROUTING.to_string()),
            enable_legacy_protect: true,
            route_exclude_address: None,
            ipv4_address: None,
            ipv6_address: None,
            extra: ExtraMap::new(),
        }
    }
}

/// `KcpItem` (6 properties; the UI tab is commented out upstream).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct KcpItem {
    #[serde(default)]
    pub mtu: i32,
    #[serde(default)]
    pub tti: i32,
    #[serde(default)]
    pub uplink_capacity: i32,
    #[serde(default)]
    pub downlink_capacity: i32,
    #[serde(default)]
    pub cwnd_multiplier: i32,
    #[serde(default)]
    pub max_sending_window: i32,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

impl Default for KcpItem {
    fn default() -> Self {
        Self {
            mtu: 1350,
            tti: 50,
            uplink_capacity: 12,
            downlink_capacity: 100,
            cwnd_multiplier: 1,
            max_sending_window: 2 * 1024 * 1024,
            extra: ExtraMap::new(),
        }
    }
}

/// `GrpcItem` (4 properties); no UI control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GrpcItem {
    #[serde(default)]
    pub idle_timeout: Option<i32>,
    #[serde(default)]
    pub health_check_timeout: Option<i32>,
    #[serde(default)]
    pub permit_without_stream: Option<bool>,
    #[serde(default)]
    pub initial_windows_size: Option<i32>,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

impl Default for GrpcItem {
    fn default() -> Self {
        Self {
            idle_timeout: Some(60),
            health_check_timeout: Some(20),
            permit_without_stream: Some(false),
            initial_windows_size: Some(0),
            extra: ExtraMap::new(),
        }
    }
}

/// `RoutingBasicItem` (3 properties); edited in the routing window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RoutingBasicItem {
    #[serde(default)]
    pub domain_strategy: Option<String>,
    #[serde(rename = "DomainStrategy4Singbox", default)]
    pub domain_strategy4_singbox: Option<String>,
    #[serde(default)]
    pub routing_index_id: Option<String>,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

impl Default for RoutingBasicItem {
    fn default() -> Self {
        Self {
            domain_strategy: Some(ROUTING_DOMAIN_STRATEGY.to_string()),
            domain_strategy4_singbox: None,
            routing_index_id: None,
            extra: ExtraMap::new(),
        }
    }
}

/// `GUIItem` (9 properties). Serialized under the `GuiItem` key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GuiItem {
    #[serde(default)]
    pub auto_run: bool,
    #[serde(default)]
    pub enable_statistics: bool,
    #[serde(default)]
    pub display_real_time_speed: bool,
    #[serde(default)]
    pub keep_older_dedupl: bool,
    #[serde(default)]
    pub auto_update_interval: i32,
    #[serde(default = "default_tray_limit")]
    pub tray_menu_servers_limit: i32,
    #[serde(rename = "EnableHWA", default)]
    pub enable_hwa: bool,
    #[serde(default = "default_true")]
    pub enable_log: bool,
    #[serde(default)]
    pub root_cert_provider: Option<String>,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

impl Default for GuiItem {
    fn default() -> Self {
        Self {
            auto_run: false,
            enable_statistics: false,
            display_real_time_speed: false,
            keep_older_dedupl: false,
            auto_update_interval: 0,
            tray_menu_servers_limit: 20,
            enable_hwa: false,
            enable_log: true,
            root_cert_provider: Some(ROOT_CERT_PROVIDER.to_string()),
            extra: ExtraMap::new(),
        }
    }
}

/// `MsgUIItem` (2 properties); message-window preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct MsgUiItem {
    #[serde(default)]
    pub main_msg_filter: Option<String>,
    #[serde(default)]
    pub auto_refresh: Option<bool>,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

/// `UIItem` (17 properties). Serialized under the `UiItem` key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UiItem {
    #[serde(default)]
    pub enable_auto_adjust_main_lv_col_width: bool,
    #[serde(default)]
    pub main_gird_height1: i32,
    #[serde(default)]
    pub main_gird_height2: i32,
    #[serde(default)]
    pub main_gird_orientation: GirdOrientation,
    #[serde(default)]
    pub color_primary_name: Option<String>,
    #[serde(default)]
    pub current_theme: Option<String>,
    #[serde(default)]
    pub current_language: Option<String>,
    #[serde(default)]
    pub current_font_family: Option<String>,
    #[serde(default)]
    pub current_font_size: i32,
    #[serde(default)]
    pub enable_drag_drop_sort: bool,
    #[serde(default)]
    pub double_click2_activate: bool,
    #[serde(default)]
    pub auto_hide_startup: bool,
    #[serde(default)]
    pub hide2_tray_when_close: bool,
    #[serde(rename = "MacOSShowInDock", default)]
    pub macos_show_in_dock: bool,
    #[serde(default)]
    pub main_column_item: Vec<ColumnDefinition>,
    #[serde(default)]
    pub window_size_item: Vec<WindowState>,
    #[serde(default)]
    pub hide_column_ip_info: bool,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

impl Default for UiItem {
    fn default() -> Self {
        Self {
            enable_auto_adjust_main_lv_col_width: false,
            main_gird_height1: 0,
            main_gird_height2: 0,
            main_gird_orientation: GirdOrientation::Vertical,
            color_primary_name: None,
            current_theme: None,
            current_language: Some(DEFAULT_LANGUAGE.to_string()),
            current_font_family: None,
            current_font_size: 0,
            enable_drag_drop_sort: false,
            double_click2_activate: false,
            auto_hide_startup: false,
            hide2_tray_when_close: false,
            macos_show_in_dock: false,
            main_column_item: Vec::new(),
            window_size_item: Vec::new(),
            hide_column_ip_info: false,
            extra: ExtraMap::new(),
        }
    }
}

/// `ConstItem` (4 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct ConstItem {
    #[serde(default)]
    pub sub_convert_url: Option<String>,
    #[serde(default)]
    pub geo_source_url: Option<String>,
    #[serde(default)]
    pub srs_source_url: Option<String>,
    #[serde(default)]
    pub route_rules_template_source_url: Option<String>,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

/// `SpeedTestItem` (8 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SpeedTestItem {
    #[serde(default)]
    pub speed_test_timeout: i32,
    #[serde(default)]
    pub speed_test_url: Option<String>,
    #[serde(default)]
    pub speed_ping_test_url: Option<String>,
    #[serde(default)]
    pub mixed_concurrency_count: i32,
    #[serde(rename = "IPAPIUrl", default)]
    pub ipapi_url: Option<String>,
    #[serde(default)]
    pub udp_test_target: Option<String>,
    #[serde(default)]
    pub speed_test_page_size: Option<i32>,
    #[serde(default)]
    pub speed_test_delay_interval: Option<i32>,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

impl Default for SpeedTestItem {
    fn default() -> Self {
        Self {
            speed_test_timeout: 10,
            speed_test_url: Some(SPEED_TEST_URL.to_string()),
            speed_ping_test_url: Some(SPEED_PING_TEST_URL.to_string()),
            mixed_concurrency_count: 10,
            ipapi_url: None,
            udp_test_target: Some(UDP_TEST_TARGET.to_string()),
            speed_test_page_size: None,
            speed_test_delay_interval: None,
            extra: ExtraMap::new(),
        }
    }
}

/// `Mux4RayItem` (3 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Mux4RayItem {
    #[serde(default)]
    pub concurrency: Option<i32>,
    #[serde(default)]
    pub xudp_concurrency: Option<i32>,
    #[serde(rename = "XudpProxyUDP443", default)]
    pub xudp_proxy_udp443: Option<String>,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

impl Default for Mux4RayItem {
    fn default() -> Self {
        Self {
            concurrency: Some(8),
            xudp_concurrency: Some(16),
            xudp_proxy_udp443: Some("reject".to_string()),
            extra: ExtraMap::new(),
        }
    }
}

/// `Mux4SboxItem` (3 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Mux4SboxItem {
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub max_connections: i32,
    #[serde(default)]
    pub padding: Option<bool>,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

impl Default for Mux4SboxItem {
    fn default() -> Self {
        Self {
            protocol: Some(SINGBOX_MUX.to_string()),
            max_connections: 8,
            padding: None,
            extra: ExtraMap::new(),
        }
    }
}

/// `HysteriaItem` (3 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct HysteriaItem {
    #[serde(default)]
    pub up_mbps: i32,
    #[serde(default)]
    pub down_mbps: i32,
    #[serde(default = "default_hop_interval")]
    pub hop_interval: i32,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

impl Default for HysteriaItem {
    fn default() -> Self {
        Self {
            up_mbps: 100,
            down_mbps: 100,
            hop_interval: 30,
            extra: ExtraMap::new(),
        }
    }
}

/// `ClashUIItem` (8 properties). Serialized under the `ClashUIItem` key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ClashUiItem {
    #[serde(rename = "EnableIPv6", default)]
    pub enable_ipv6: bool,
    #[serde(default)]
    pub enable_mixin_content: bool,
    #[serde(default)]
    pub proxies_sorting: i32,
    #[serde(default)]
    pub proxies_auto_refresh: bool,
    #[serde(default = "default_two")]
    pub proxies_refresh_interval: i32,
    #[serde(default)]
    pub connections_auto_refresh: bool,
    #[serde(default = "default_two")]
    pub connections_refresh_interval: i32,
    #[serde(default)]
    pub connections_column_item: Vec<ColumnDefinition>,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

impl Default for ClashUiItem {
    fn default() -> Self {
        Self {
            enable_ipv6: false,
            enable_mixin_content: false,
            proxies_sorting: 0,
            proxies_auto_refresh: false,
            proxies_refresh_interval: 2,
            connections_auto_refresh: false,
            connections_refresh_interval: 2,
            connections_column_item: Vec::new(),
            extra: ExtraMap::new(),
        }
    }
}

/// `SystemProxyItem` (6 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SystemProxyItem {
    #[serde(default)]
    pub sys_proxy_type: SysProxyType,
    #[serde(default)]
    pub system_proxy_exceptions: Option<String>,
    #[serde(default = "default_true")]
    pub not_proxy_local_address: bool,
    #[serde(default)]
    pub system_proxy_advanced_protocol: Option<String>,
    #[serde(default)]
    pub custom_system_proxy_pac_path: Option<String>,
    #[serde(default)]
    pub custom_system_proxy_script_path: Option<String>,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

impl Default for SystemProxyItem {
    fn default() -> Self {
        Self {
            sys_proxy_type: SysProxyType::ForcedClear,
            system_proxy_exceptions: Some(default_system_proxy_exceptions()),
            not_proxy_local_address: true,
            system_proxy_advanced_protocol: None,
            custom_system_proxy_pac_path: None,
            custom_system_proxy_script_path: None,
            extra: ExtraMap::new(),
        }
    }
}

/// `WebDavItem` (4 properties). Values are secrets; never log them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct WebDavItem {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub user_name: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub dir_name: Option<String>,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

/// `CheckUpdateItem` (3 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CheckUpdateItem {
    #[serde(default)]
    pub check_pre_release_update: bool,
    #[serde(default = "default_true")]
    pub update_via_proxy: bool,
    #[serde(default)]
    pub selected_core_types: Option<Vec<String>>,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

impl Default for CheckUpdateItem {
    fn default() -> Self {
        Self {
            check_pre_release_update: false,
            update_via_proxy: true,
            selected_core_types: None,
            extra: ExtraMap::new(),
        }
    }
}

/// `Fragment4RayItem` (6 properties incl. 2 legacy migration fields).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Fragment4RayItem {
    #[serde(default)]
    pub packets: Option<String>,
    #[serde(default)]
    pub lengths: Option<Vec<String>>,
    #[serde(default)]
    pub delays: Option<Vec<String>>,
    #[serde(default)]
    pub max_split: Option<String>,
    /// Legacy migration field; preserved, used only as a `Lengths` fallback.
    #[serde(default)]
    pub length: Option<String>,
    /// Legacy migration field; preserved, used only as a `Delays` fallback.
    #[serde(default)]
    pub interval: Option<String>,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

impl Default for Fragment4RayItem {
    fn default() -> Self {
        Self {
            packets: Some("tlshello".to_string()),
            lengths: None,
            delays: None,
            max_split: Some("0".to_string()),
            length: None,
            interval: None,
            extra: ExtraMap::new(),
        }
    }
}

/// `SimpleDNSItem` (18 properties); edited in the DNS window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct SimpleDnsItem {
    #[serde(default)]
    pub use_system_hosts: Option<bool>,
    #[serde(default)]
    pub add_common_hosts: Option<bool>,
    #[serde(rename = "FakeIP", default)]
    pub fake_ip: Option<bool>,
    #[serde(default)]
    pub global_fake_ip: Option<bool>,
    #[serde(rename = "FakeIPRange", default)]
    pub fake_ip_range: Option<String>,
    #[serde(default)]
    pub block_binding_query: Option<bool>,
    #[serde(rename = "BlockAAAAQuery", default)]
    pub block_aaaa_query: Option<bool>,
    #[serde(rename = "DirectDNS", default)]
    pub direct_dns: Option<String>,
    #[serde(rename = "RemoteDNS", default)]
    pub remote_dns: Option<String>,
    #[serde(rename = "BootstrapDNS", default)]
    pub bootstrap_dns: Option<String>,
    #[serde(default)]
    pub strategy4_freedom: Option<String>,
    #[serde(default)]
    pub strategy4_proxy: Option<String>,
    #[serde(default)]
    pub strategy4_proxy_dial: Option<String>,
    #[serde(default)]
    pub serve_stale: Option<bool>,
    #[serde(default)]
    pub parallel_query: Option<bool>,
    #[serde(default)]
    pub hosts: Option<String>,
    #[serde(rename = "DirectExpectedIPs", default)]
    pub direct_expected_ips: Option<String>,
    #[serde(default)]
    pub enable_happy_eyeballs: Option<bool>,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

impl SimpleDnsItem {
    /// The object `ConfigHandler.InitBuiltinSimpleDNS` returns.
    pub fn builtin() -> Self {
        Self {
            use_system_hosts: Some(false),
            add_common_hosts: Some(true),
            fake_ip: Some(false),
            global_fake_ip: Some(true),
            fake_ip_range: None,
            block_binding_query: Some(true),
            block_aaaa_query: None,
            direct_dns: Some(DIRECT_DNS.to_string()),
            remote_dns: Some(REMOTE_DNS.to_string()),
            bootstrap_dns: Some(DIRECT_DNS.to_string()),
            strategy4_freedom: None,
            strategy4_proxy: None,
            strategy4_proxy_dial: None,
            serve_stale: None,
            parallel_query: None,
            hosts: None,
            direct_expected_ips: None,
            enable_happy_eyeballs: None,
            extra: ExtraMap::new(),
        }
    }
}

/// `HappyEyeballs4RayItem` (4 properties); edited in the DNS window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct HappyEyeballs4RayItem {
    #[serde(default)]
    pub try_delay_ms: Option<i32>,
    #[serde(rename = "PrioritizeIPv6", default)]
    pub prioritize_ipv6: Option<bool>,
    #[serde(default)]
    pub interleave: Option<i32>,
    #[serde(default)]
    pub max_concurrent_try: Option<i32>,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

impl Default for HappyEyeballs4RayItem {
    fn default() -> Self {
        Self {
            try_delay_ms: Some(250),
            prioritize_ipv6: Some(false),
            interleave: Some(1),
            max_concurrent_try: Some(4),
            extra: ExtraMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Config root
// ---------------------------------------------------------------------------

/// The `guiNConfig.json` root (`Config`: 2 ids + 23 items).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AppSettings {
    #[serde(default)]
    pub index_id: Option<String>,
    #[serde(default)]
    pub sub_index_id: Option<String>,

    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub core_basic_item: CoreBasicItem,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub tun_mode_item: TunModeItem,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub kcp_item: KcpItem,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub grpc_item: GrpcItem,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub routing_basic_item: RoutingBasicItem,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub gui_item: GuiItem,
    #[serde(
        rename = "MsgUIItem",
        default,
        deserialize_with = "deserialize_null_default"
    )]
    pub msg_ui_item: MsgUiItem,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub ui_item: UiItem,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub const_item: ConstItem,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub speed_test_item: SpeedTestItem,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub mux4_ray_item: Mux4RayItem,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub mux4_sbox_item: Mux4SboxItem,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub hysteria_item: HysteriaItem,
    #[serde(
        rename = "ClashUIItem",
        default,
        deserialize_with = "deserialize_null_default"
    )]
    pub clash_ui_item: ClashUiItem,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub system_proxy_item: SystemProxyItem,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub web_dav_item: WebDavItem,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub check_update_item: CheckUpdateItem,
    #[serde(default)]
    pub fragment4_ray_item: Option<Fragment4RayItem>,
    #[serde(
        default = "default_inbound_list",
        deserialize_with = "deserialize_null_default"
    )]
    pub inbound: Vec<InboundListener>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub global_hotkeys: Vec<GlobalHotkey>,
    #[serde(default)]
    pub core_type_item: Option<Vec<CoreTypeBinding>>,
    #[serde(
        rename = "SimpleDNSItem",
        default,
        deserialize_with = "deserialize_null_default"
    )]
    pub simple_dns_item: SimpleDnsItem,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub happy_eyeballs4_ray_item: HappyEyeballs4RayItem,
    #[serde(flatten, default)]
    pub extra: ExtraMap,
}

impl Default for AppSettings {
    fn default() -> Self {
        let mut settings = Self {
            index_id: None,
            sub_index_id: None,
            core_basic_item: CoreBasicItem::default(),
            tun_mode_item: TunModeItem::default(),
            kcp_item: KcpItem::default(),
            grpc_item: GrpcItem::default(),
            routing_basic_item: RoutingBasicItem::default(),
            gui_item: GuiItem::default(),
            msg_ui_item: MsgUiItem::default(),
            ui_item: UiItem::default(),
            const_item: ConstItem::default(),
            speed_test_item: SpeedTestItem::default(),
            mux4_ray_item: Mux4RayItem::default(),
            mux4_sbox_item: Mux4SboxItem::default(),
            hysteria_item: HysteriaItem::default(),
            clash_ui_item: ClashUiItem::default(),
            system_proxy_item: SystemProxyItem::default(),
            web_dav_item: WebDavItem::default(),
            check_update_item: CheckUpdateItem::default(),
            fragment4_ray_item: Some(Fragment4RayItem::default()),
            inbound: default_inbound_list(),
            global_hotkeys: Vec::new(),
            core_type_item: None,
            simple_dns_item: SimpleDnsItem::builtin(),
            happy_eyeballs4_ray_item: HappyEyeballs4RayItem::default(),
            extra: ExtraMap::new(),
        };
        settings.apply_load_defaults();
        settings
    }
}

impl AppSettings {
    /// Strictly parse a raw `guiNConfig.json` tree (engine meta keys already
    /// stripped). A known-field type error is returned as a structured
    /// `error.config_corrupt` failure instead of defaulting the whole tree
    /// (SP-01/CP-06). Missing groups still get their `LoadConfig` defaults via
    /// [`AppSettings::apply_load_defaults`]; explicit `null` groups behave
    /// like missing ones (upstream `??=`); unknown keys survive in `extra`.
    pub fn parse_strict(value: &Value) -> Result<Self, DomainError> {
        serde_json::from_value(value.clone()).map_err(|error| {
            DomainError::new(codes::FIELD_FORMAT, "error.config_corrupt")
                .with_detail(sanitize_serde_detail(&error))
                .retryable()
        })
    }

    /// Apply the object-level `ConfigHandler.LoadConfig` synthesis and
    /// corrections that cannot be expressed as serde field defaults.
    pub fn apply_load_defaults(&mut self) {
        // Inbound: a missing/empty list gets the default socks listener and the
        // first listener is always forced to `socks`.
        if self.inbound.is_empty() {
            self.inbound.push(default_inbound());
        }
        if let Some(first) = self.inbound.first_mut() {
            first.protocol = crate::enums::InboundProtocol::Socks;
        }

        if !non_empty(&self.routing_basic_item.domain_strategy) {
            self.routing_basic_item.domain_strategy = Some(ROUTING_DOMAIN_STRATEGY.to_string());
        }

        if self.kcp_item.cwnd_multiplier <= 0 {
            self.kcp_item.cwnd_multiplier = 1;
        }
        if self.kcp_item.max_sending_window <= 0 {
            self.kcp_item.max_sending_window = 2 * 1024 * 1024;
        }

        if !matches!(
            self.gui_item.root_cert_provider.as_deref(),
            Some("system") | Some("chrome") | Some("mozilla")
        ) {
            self.gui_item.root_cert_provider = Some(ROOT_CERT_PROVIDER.to_string());
        }

        if !non_empty(&self.ui_item.current_language) {
            self.ui_item.current_language = Some(DEFAULT_LANGUAGE.to_string());
        }

        // SimpleDNSItem: only the `??=` fills apply to a present object; the
        // `InitBuiltinSimpleDNS` values are supplied via `Default` when the
        // whole object is absent.
        if self.simple_dns_item.block_aaaa_query.is_none() {
            self.simple_dns_item.block_aaaa_query = Some(false);
        }
        if self.simple_dns_item.fake_ip_range.is_none() {
            self.simple_dns_item.fake_ip_range = Some(FAKE_IP_RANGE.to_string());
        }
        if self.simple_dns_item.global_fake_ip.is_none() {
            self.simple_dns_item.global_fake_ip = Some(true);
        }
        if self.simple_dns_item.bootstrap_dns.is_none() {
            self.simple_dns_item.bootstrap_dns = Some(DIRECT_DNS.to_string());
        }
        if self.simple_dns_item.serve_stale.is_none() {
            self.simple_dns_item.serve_stale = Some(false);
        }
        if self.simple_dns_item.parallel_query.is_none() {
            self.simple_dns_item.parallel_query = Some(false);
        }
        if self.simple_dns_item.enable_happy_eyeballs.is_none() {
            self.simple_dns_item.enable_happy_eyeballs = Some(false);
        }

        // FLD-CFG-106: enforced by correction-on-normalize (>=10), never by
        // rejecting the save; `validate_settings` therefore only rejects <0.
        if self.speed_test_item.speed_test_timeout < 10 {
            self.speed_test_item.speed_test_timeout = 10;
        }
        if !non_empty(&self.speed_test_item.speed_test_url) {
            self.speed_test_item.speed_test_url = Some(SPEED_TEST_URL.to_string());
        }
        if !non_empty(&self.speed_test_item.speed_ping_test_url) {
            self.speed_test_item.speed_ping_test_url = Some(SPEED_PING_TEST_URL.to_string());
        }
        // Same correction-not-rejection contract as SpeedTestTimeout above.
        if self.speed_test_item.mixed_concurrency_count < 10 {
            self.speed_test_item.mixed_concurrency_count = 10;
        }
        if !non_empty(&self.speed_test_item.udp_test_target) {
            self.speed_test_item.udp_test_target = Some(UDP_TEST_TARGET.to_string());
        }

        if !non_empty(&self.system_proxy_item.system_proxy_exceptions) {
            self.system_proxy_item.system_proxy_exceptions =
                Some(default_system_proxy_exceptions());
        }

        // Fragment4RayItem is nullable upstream: a missing object is created,
        // then `MaxSplit`/`Lengths`/`Delays` are filled (using the legacy
        // `Length`/`Interval` fallbacks).
        let fragment = self
            .fragment4_ray_item
            .get_or_insert_with(Fragment4RayItem::default);
        if fragment.max_split.is_none() {
            fragment.max_split = Some("0".to_string());
        }
        if !fragment.lengths.as_ref().is_some_and(|v| !v.is_empty()) {
            fragment.lengths = Some(vec![fragment
                .length
                .clone()
                .unwrap_or_else(|| "50-100".to_string())]);
        }
        if !fragment.delays.as_ref().is_some_and(|v| !v.is_empty()) {
            fragment.delays = Some(vec![fragment
                .interval
                .clone()
                .unwrap_or_else(|| "10-20".to_string())]);
        }
    }

    /// Initialise `CoreTypeItem` the way `OptionSettingViewModel.InitCoreType`
    /// does when the settings window first opens: every `ConfigType` gets a row
    /// defaulting to Xray.
    pub fn init_core_type_items(&mut self) {
        let items = self.core_type_item.get_or_insert_with(Vec::new);
        for config_type in ConfigType::ALL {
            if !items.iter().any(|b| b.config_type == config_type) {
                items.push(CoreTypeBinding::new(config_type, CoreType::Xray));
            }
        }
    }

    /// The core bound to `config_type` (falls back to Xray, upstream default).
    pub fn core_for(&self, config_type: ConfigType) -> CoreType {
        self.core_type_item
            .as_ref()
            .and_then(|items| {
                items
                    .iter()
                    .find(|b| b.config_type == config_type)
                    .map(|b| b.core_type)
            })
            .unwrap_or(CoreType::Xray)
    }

    /// Classify every field that differs between `self` (old) and `new`.
    pub fn classified_changes(&self, new: &AppSettings) -> Vec<SettingsChange> {
        let old_value = serde_json::to_value(self).unwrap_or(serde_json::Value::Null);
        let new_value = serde_json::to_value(new).unwrap_or(serde_json::Value::Null);
        let mut changes = Vec::new();
        if let (serde_json::Value::Object(old), serde_json::Value::Object(new)) =
            (&old_value, &new_value)
        {
            let mut keys: Vec<&String> = old.keys().chain(new.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                if key == "IndexId" || key == "SubIndexId" {
                    if old.get(key) != new.get(key) {
                        changes.push(SettingsChange::leaf("Config", key));
                    }
                    continue;
                }
                match (old.get(key), new.get(key)) {
                    (Some(a), Some(b)) if a == b => {}
                    (Some(serde_json::Value::Array(a)), Some(serde_json::Value::Array(b)))
                        if is_list_group(key) =>
                    {
                        diff_list(key, a, b, &mut changes);
                    }
                    (Some(serde_json::Value::Object(a)), Some(serde_json::Value::Object(b))) => {
                        diff_object(key, a, b, &mut changes);
                    }
                    _ => changes.push(SettingsChange::whole(key)),
                }
            }
        }
        changes
    }
}

fn is_list_group(key: &str) -> bool {
    matches!(key, "Inbound" | "GlobalHotkeys" | "CoreTypeItem")
}

fn diff_object(
    group: &str,
    a: &serde_json::Map<String, serde_json::Value>,
    b: &serde_json::Map<String, serde_json::Value>,
    out: &mut Vec<SettingsChange>,
) {
    let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
    keys.sort();
    keys.dedup();
    for field in keys {
        if a.get(field) != b.get(field) {
            out.push(SettingsChange::leaf(group, field));
        }
    }
}

fn diff_list(
    group: &str,
    a: &[serde_json::Value],
    b: &[serde_json::Value],
    out: &mut Vec<SettingsChange>,
) {
    if a.len() != b.len() {
        out.push(SettingsChange::whole(group));
        return;
    }
    for (left, right) in a.iter().zip(b.iter()) {
        match (left, right) {
            (serde_json::Value::Object(la), serde_json::Value::Object(rb)) => {
                let mut keys: Vec<&String> = la.keys().chain(rb.keys()).collect();
                keys.sort();
                keys.dedup();
                for field in keys {
                    if la.get(field) != rb.get(field) {
                        out.push(SettingsChange::leaf(group, field));
                    }
                }
            }
            _ if left == right => {}
            _ => out.push(SettingsChange::whole(group)),
        }
    }
}

/// One classified settings change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsChange {
    pub group: String,
    pub field: String,
    pub timing: ApplyTiming,
}

impl SettingsChange {
    pub fn leaf(group: &str, field: &str) -> Self {
        Self {
            group: group.to_string(),
            field: field.to_string(),
            timing: crate::settings_timing::classify_leaf(group, field),
        }
    }

    pub fn whole(group: &str) -> Self {
        Self {
            group: group.to_string(),
            field: "*".to_string(),
            timing: crate::settings_timing::classify_leaf(group, "*"),
        }
    }

    pub fn path(&self) -> String {
        if self.field == "*" {
            self.group.clone()
        } else {
            format!("{}.{}", self.group, self.field)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_raw() -> &'static str {
        r#"{
            "IndexId": "node-1",
            "SubIndexId": "",
            "CoreBasicItem": {"LogEnabled": true, "Loglevel": null, "FutureToggle": 42},
            "GuiItem": {"EnableLog": false},
            "UiItem": {"CurrentLanguage": "", "MainGirdOrientation": 0},
            "Inbound": [{"LocalPort": 12345, "Protocol": 0, "UdpEnabled": true}],
            "Fragment4RayItem": {"Packets": "1-2", "Length": "7-9"},
            "SimpleDNSItem": {"FakeIP": null, "RemoteDNS": ""},
            "UnknownRoot": {"x": [1, 2, 3]}
        }"#
    }

    #[test]
    fn parses_pascal_case_and_preserves_unknown() {
        let s: AppSettings = serde_json::from_str(sample_raw()).unwrap();
        assert_eq!(s.index_id.as_deref(), Some("node-1"));
        assert_eq!(s.sub_index_id.as_deref(), Some(""));
        assert!(s.core_basic_item.log_enabled);
        assert!(s.core_basic_item.extra.contains_key("FutureToggle"));
        assert!(s.extra.contains_key("UnknownRoot"));
        assert_eq!(s.inbound[0].local_port, 12345);
    }

    #[test]
    fn null_loglevel_stays_none_but_missing_object_gets_warning() {
        let s: AppSettings = serde_json::from_str(sample_raw()).unwrap();
        // Present object with an explicit null loglevel stays null.
        assert_eq!(s.core_basic_item.loglevel, None);
        // A missing object uses the LoadConfig default.
        let empty: AppSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(empty.core_basic_item.loglevel.as_deref(), Some("warning"));
    }

    #[test]
    fn present_object_missing_field_stays_type_default() {
        // A present `CoreBasicItem` without `Loglevel` keeps null (CLR default).
        let s: AppSettings = serde_json::from_str(r#"{"CoreBasicItem":{}}"#).unwrap();
        assert_eq!(s.core_basic_item.loglevel, None);
    }

    #[test]
    fn empty_string_and_null_are_distinct() {
        let s: AppSettings = serde_json::from_str(sample_raw()).unwrap();
        assert_eq!(s.sub_index_id.as_deref(), Some(""));
        assert_eq!(s.simple_dns_item.remote_dns.as_deref(), Some(""));
        assert_eq!(s.simple_dns_item.fake_ip, None);
    }

    #[test]
    fn apply_load_defaults_fills_object_level_defaults() {
        let mut s: AppSettings = serde_json::from_str("{}").unwrap();
        s.apply_load_defaults();
        assert_eq!(s.inbound[0].local_port, 10808);
        assert_eq!(s.inbound[0].protocol, crate::enums::InboundProtocol::Socks);
        assert_eq!(
            s.routing_basic_item.domain_strategy.as_deref(),
            Some("AsIs")
        );
        assert_eq!(s.ui_item.current_language.as_deref(), Some("zh-Hans"));
        assert_eq!(s.speed_test_item.speed_test_timeout, 10);
        assert_eq!(s.gui_item.root_cert_provider.as_deref(), Some("system"));
        assert!(s.system_proxy_item.system_proxy_exceptions.is_some());
        assert_eq!(
            s.fragment4_ray_item.as_ref().unwrap().max_split.as_deref(),
            Some("0")
        );
    }

    #[test]
    fn legacy_fragment_lengths_become_delays_fallback() {
        let mut s: AppSettings = serde_json::from_str(sample_raw()).unwrap();
        s.apply_load_defaults();
        let fragment = s.fragment4_ray_item.unwrap();
        assert_eq!(fragment.lengths.unwrap(), vec!["7-9".to_string()]);
        assert_eq!(fragment.delays.unwrap(), vec!["10-20".to_string()]);
    }

    #[test]
    fn explicit_null_group_becomes_load_default() {
        let raw = r#"{"CoreBasicItem": null, "KcpItem": null, "Inbound": null}"#;
        let mut s: AppSettings = serde_json::from_str(raw).unwrap();
        s.apply_load_defaults();
        assert_eq!(s.core_basic_item.loglevel.as_deref(), Some("warning"));
        assert_eq!(s.kcp_item.mtu, 1350);
        assert_eq!(s.inbound.len(), 1);
    }

    #[test]
    fn parse_strict_rejects_bad_field_type_without_defaulting() {
        // SP-01: a single mistyped known field must not silently reset the
        // rest of the document to defaults.
        let value: Value = serde_json::from_str(
            r#"{"GuiItem": {"TrayMenuServersLimit": "oops-not-a-number"},
                "UiItem": {"CurrentLanguage": "en"}}"#,
        )
        .unwrap();
        let err = AppSettings::parse_strict(&value).expect_err("bad type must fail");
        assert_eq!(err.code, codes::FIELD_FORMAT);
        assert_eq!(err.message_key, "error.config_corrupt");
        assert!(!err
            .detail
            .as_deref()
            .unwrap_or("")
            .contains("oops-not-a-number"));
    }

    #[test]
    fn parse_strict_accepts_missing_null_and_unknown() {
        let value: Value = serde_json::from_str(
            r#"{"GuiItem": null, "UiItem": {"CurrentLanguage": "en"},
                "FutureRoot": {"x": 1}}"#,
        )
        .unwrap();
        let mut parsed = AppSettings::parse_strict(&value).expect("valid doc");
        parsed.apply_load_defaults();
        assert_eq!(parsed.gui_item.tray_menu_servers_limit, 20);
        assert_eq!(parsed.ui_item.current_language.as_deref(), Some("en"));
        assert!(parsed.extra.contains_key("FutureRoot"));
    }

    #[test]
    fn enums_round_trip_as_integers() {
        let s: AppSettings = serde_json::from_str(sample_raw()).unwrap();
        let value = serde_json::to_value(&s).unwrap();
        assert_eq!(value["UiItem"]["MainGirdOrientation"], 0);
        assert_eq!(value["SystemProxyItem"]["SysProxyType"], 0);
    }

    #[test]
    fn round_trip_is_field_equivalent_including_unknown() {
        let mut first: AppSettings = serde_json::from_str(sample_raw()).unwrap();
        first.apply_load_defaults();
        let text = serde_json::to_string(&first).unwrap();
        let second: AppSettings = serde_json::from_str(&text).unwrap();
        assert_eq!(first, second);
        // Unknown keys survive a typed round trip.
        assert!(second.core_basic_item.extra.contains_key("FutureToggle"));
        assert!(second.extra.contains_key("UnknownRoot"));
    }

    #[test]
    fn defaults_match_upstream_initializers() {
        let s = AppSettings::default();
        assert!(s.gui_item.enable_log);
        assert_eq!(s.gui_item.tray_menu_servers_limit, 20);
        assert!(s.core_basic_item.enable_cache_file4_sbox);
        assert!(s.tun_mode_item.auto_route);
        assert!(s.tun_mode_item.strict_route);
        assert!(s.tun_mode_item.enable_legacy_protect);
        assert_eq!(s.hysteria_item.hop_interval, 30);
        assert_eq!(s.clash_ui_item.proxies_refresh_interval, 2);
        assert!(s.system_proxy_item.not_proxy_local_address);
        assert!(s.check_update_item.update_via_proxy);
        assert_eq!(s.ui_item.main_gird_orientation, GirdOrientation::Vertical);
        assert_eq!(s.simple_dns_item.use_system_hosts, Some(false));
        assert_eq!(s.simple_dns_item.add_common_hosts, Some(true));
        assert_eq!(s.happy_eyeballs4_ray_item.try_delay_ms, Some(250));
    }

    #[test]
    fn classify_restart_core_and_restart_app() {
        let old = AppSettings::default();
        let mut new = old.clone();
        new.core_basic_item.loglevel = Some("debug".to_string());
        new.gui_item.enable_statistics = true;
        new.ui_item.current_theme = Some("Dark".to_string());
        let changes = old.classified_changes(&new);
        let by_path: std::collections::HashMap<_, _> =
            changes.iter().map(|c| (c.path(), c.timing)).collect();
        assert_eq!(
            by_path.get("CoreBasicItem.Loglevel"),
            Some(&ApplyTiming::RestartCore)
        );
        assert_eq!(
            by_path.get("GuiItem.EnableStatistics"),
            Some(&ApplyTiming::RestartApp)
        );
        assert_eq!(
            by_path.get("UiItem.CurrentTheme"),
            Some(&ApplyTiming::Immediate)
        );
    }

    #[test]
    fn classify_list_and_hotkey_groups() {
        let old = AppSettings::default();
        let mut new = old.clone();
        new.inbound[0].local_port = 12345;
        new.global_hotkeys.push(GlobalHotkey {
            action: 0,
            control: true,
            key_code: Some(65),
            ..Default::default()
        });
        let changes = old.classified_changes(&new);
        let by_path: std::collections::HashMap<_, _> =
            changes.iter().map(|c| (c.path(), c.timing)).collect();
        assert_eq!(
            by_path.get("Inbound.LocalPort"),
            Some(&ApplyTiming::RestartCore)
        );
        assert_eq!(by_path.get("GlobalHotkeys"), Some(&ApplyTiming::NextLaunch));
    }

    #[test]
    fn classify_whole_group_add_remove() {
        let old = AppSettings::default();
        let mut new = old.clone();
        new.fragment4_ray_item = None;
        let changes = old.classified_changes(&new);
        assert!(changes
            .iter()
            .any(|c| c.group == "Fragment4RayItem" && c.timing == ApplyTiming::RestartCore));
    }

    #[test]
    fn core_type_helpers() {
        let mut s = AppSettings::default();
        assert_eq!(s.core_for(ConfigType::Vless), CoreType::Xray);
        s.init_core_type_items();
        assert_eq!(
            s.core_type_item.as_ref().unwrap().len(),
            ConfigType::ALL.len()
        );
        assert_eq!(s.core_for(ConfigType::Vmess), CoreType::Xray);
    }

    #[test]
    fn group_keys_are_recognised() {
        for key in SETTINGS_GROUPS {
            assert!(is_settings_group(key), "{key}");
        }
        assert!(!is_settings_group("NotAGroup"));
    }

    #[test]
    fn timing_table_covers_all_ledger_entries() {
        let table = crate::settings_timing::FIELD_TIMING;
        assert_eq!(table.len(), 180, "one entry per ledger field");
        let mut unique = std::collections::HashSet::new();
        for (group, field, _) in table {
            assert!(
                unique.insert((*group, *field)),
                "duplicate timing entry {group}.{field}"
            );
        }
    }

    #[test]
    fn timing_groups_are_known() {
        for (group, field, _) in crate::settings_timing::FIELD_TIMING {
            if *group == "Config" {
                // Whole-group entries reference a top-level key.
                assert!(!field.is_empty());
            } else {
                assert!(
                    is_settings_group(group) || matches!(*group, "ColumnItem" | "WindowSizeItem"),
                    "unknown timing group {group}"
                );
            }
        }
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn all_groups_round_trip_with_unknown_keys() {
        use crate::entities::CoreTypeBinding;
        let mut settings = AppSettings::default();
        settings.index_id = Some("id".to_string());
        settings.sub_index_id = Some(String::new());
        settings.core_basic_item.loglevel = Some("debug".to_string());
        settings.tun_mode_item.stack = Some("mixed".to_string());
        settings.kcp_item.mtu = 1400;
        settings.grpc_item.idle_timeout = Some(30);
        settings.routing_basic_item.routing_index_id = Some("r1".to_string());
        settings.gui_item.enable_statistics = true;
        settings.msg_ui_item.auto_refresh = Some(true);
        settings.ui_item.current_theme = Some("Dark".to_string());
        settings.const_item.geo_source_url = Some("url".to_string());
        settings.speed_test_item.speed_test_timeout = 20;
        settings.mux4_ray_item.concurrency = Some(4);
        settings.mux4_sbox_item.protocol = Some("smux".to_string());
        settings.hysteria_item.up_mbps = 50;
        settings.clash_ui_item.enable_ipv6 = true;
        settings.system_proxy_item.sys_proxy_type = crate::enums::SysProxyType::Pac;
        settings.web_dav_item.url = Some("https://example.invalid".to_string());
        settings.check_update_item.check_pre_release_update = true;
        settings.fragment4_ray_item.as_mut().unwrap().packets = Some("1-2".to_string());
        settings.inbound[0].user = "u".to_string();
        settings.global_hotkeys.push(GlobalHotkey {
            action: 1,
            control: true,
            key_code: Some(66),
            ..Default::default()
        });
        settings.core_type_item = Some(vec![CoreTypeBinding::new(
            ConfigType::Vless,
            CoreType::SingBox,
        )]);
        settings.simple_dns_item.fake_ip = Some(true);
        settings.happy_eyeballs4_ray_item.try_delay_ms = Some(100);
        settings
            .extra
            .insert("CustomRoot".to_string(), serde_json::json!(1));

        let text = serde_json::to_string(&settings).unwrap();
        let mut restored: AppSettings = serde_json::from_str(&text).unwrap();
        restored.apply_load_defaults();
        assert_eq!(settings, restored);
        assert!(restored.extra.contains_key("CustomRoot"));
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn core_type_binding_field_timings_differ() {
        use crate::entities::CoreTypeBinding;
        let mut old = AppSettings::default();
        old.core_type_item = Some(vec![CoreTypeBinding::new(
            ConfigType::Vless,
            CoreType::Xray,
        )]);

        let mut core_changed = old.clone();
        core_changed.core_type_item = Some(vec![CoreTypeBinding::new(
            ConfigType::Vless,
            CoreType::SingBox,
        )]);
        let changes = old.classified_changes(&core_changed);
        assert!(changes
            .iter()
            .any(|c| c.path() == "CoreTypeItem.CoreType" && c.timing == ApplyTiming::RestartCore));

        let mut type_changed = old.clone();
        type_changed.core_type_item = Some(vec![CoreTypeBinding::new(
            ConfigType::Trojan,
            CoreType::Xray,
        )]);
        let changes = old.classified_changes(&type_changed);
        assert!(changes
            .iter()
            .any(|c| c.path() == "CoreTypeItem.ConfigType" && c.timing == ApplyTiming::Save));
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn hotkey_edits_are_next_launch() {
        let mut old = AppSettings::default();
        old.global_hotkeys = vec![GlobalHotkey {
            action: 0,
            ..Default::default()
        }];
        let mut new = old.clone();
        new.global_hotkeys[0].control = true;
        new.global_hotkeys[0].key_code = Some(65);
        let changes = old.classified_changes(&new);
        assert!(changes
            .iter()
            .any(|c| c.group == "GlobalHotkeys" && c.timing == ApplyTiming::NextLaunch));
    }
}
