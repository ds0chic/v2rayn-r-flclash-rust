//! Application settings tree.
//!
//! Mirrors the upstream `Config` root (guiNConfig.json) and its 25
//! `ConfigItems` sub-classes. This is the type + default-semantics skeleton
//! required by T02; full UI wiring and per-field validation is a later task
//! (`T12a`). Unknown keys are preserved at every level.
//!
//! Numeric/id semantics match `work/research-v2rayn/.../ConfigItems.cs` and
//! `Config.cs`; defaults follow the property initializers there.

use serde::{Deserialize, Serialize};

use crate::entities::{
    ColumnDefinition, CoreTypeBinding, GlobalHotkey, InboundListener, WindowState,
};
use crate::enums::{GirdOrientation, SysProxyType};
use crate::profile::ExtraMap;

/// `CoreBasicItem` (9 properties).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CoreBasicItem {
    pub log_enabled: bool,
    pub loglevel: String,
    pub def_fingerprint: String,
    pub def_user_agent: String,
    pub send_through: Option<String>,
    pub bind_interface: Option<String>,
    pub enable_fragment: bool,
    pub enable_final_fragment: bool,
    pub enable_cache_file4_sbox: bool,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

impl Default for CoreBasicItem {
    fn default() -> Self {
        Self {
            log_enabled: false,
            loglevel: String::new(),
            def_fingerprint: String::new(),
            def_user_agent: String::new(),
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
#[serde(default)]
pub struct TunModeItem {
    pub enable_tun: bool,
    pub auto_route: bool,
    pub strict_route: bool,
    pub stack: String,
    pub mtu: i32,
    pub enable_ipv6_address: bool,
    pub icmp_routing: String,
    pub enable_legacy_protect: bool,
    pub route_exclude_address: Option<Vec<String>>,
    pub ipv4_address: String,
    pub ipv6_address: String,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

impl Default for TunModeItem {
    fn default() -> Self {
        Self {
            enable_tun: false,
            auto_route: true,
            strict_route: true,
            stack: String::new(),
            mtu: 0,
            enable_ipv6_address: false,
            icmp_routing: String::new(),
            enable_legacy_protect: true,
            route_exclude_address: None,
            ipv4_address: String::new(),
            ipv6_address: String::new(),
            extra: ExtraMap::new(),
        }
    }
}

/// `KcpItem` (6 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KcpItem {
    pub mtu: i32,
    pub tti: i32,
    pub uplink_capacity: i32,
    pub downlink_capacity: i32,
    pub cwnd_multiplier: i32,
    pub max_sending_window: i32,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

/// `GrpcItem` (4 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct GrpcItem {
    pub idle_timeout: Option<i32>,
    pub health_check_timeout: Option<i32>,
    pub permit_without_stream: Option<bool>,
    pub initial_windows_size: Option<i32>,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

/// `RoutingBasicItem` (3 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RoutingBasicItem {
    pub domain_strategy: String,
    pub domain_strategy4_singbox: String,
    pub routing_index_id: String,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

/// `GUIItem` (9 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GuiItem {
    pub auto_run: bool,
    pub enable_statistics: bool,
    pub display_real_time_speed: bool,
    pub keep_older_dedupl: bool,
    pub auto_update_interval: i32,
    pub tray_menu_servers_limit: i32,
    pub enable_hwa: bool,
    pub enable_log: bool,
    pub root_cert_provider: Option<String>,
    #[serde(flatten)]
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
            root_cert_provider: None,
            extra: ExtraMap::new(),
        }
    }
}

/// `MsgUIItem` (2 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MsgUiItem {
    pub main_msg_filter: Option<String>,
    pub auto_refresh: Option<bool>,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

/// `UIItem` (17 properties).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiItem {
    pub enable_auto_adjust_main_lv_col_width: bool,
    pub main_gird_height1: i32,
    pub main_gird_height2: i32,
    pub main_gird_orientation: GirdOrientation,
    pub color_primary_name: Option<String>,
    pub current_theme: Option<String>,
    pub current_language: String,
    pub current_font_family: String,
    pub current_font_size: i32,
    pub enable_drag_drop_sort: bool,
    pub double_click2_activate: bool,
    pub auto_hide_startup: bool,
    pub hide2_tray_when_close: bool,
    pub macos_show_in_dock: bool,
    pub main_column_item: Vec<ColumnDefinition>,
    pub window_size_item: Vec<WindowState>,
    pub hide_column_ip_info: bool,
    #[serde(flatten)]
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
            current_language: String::new(),
            current_font_family: String::new(),
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
#[serde(default)]
pub struct ConstItem {
    pub sub_convert_url: Option<String>,
    pub geo_source_url: Option<String>,
    pub srs_source_url: Option<String>,
    pub route_rules_template_source_url: Option<String>,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

/// `SpeedTestItem` (8 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SpeedTestItem {
    pub speed_test_timeout: i32,
    pub speed_test_url: String,
    pub speed_ping_test_url: String,
    pub mixed_concurrency_count: i32,
    pub ipapi_url: String,
    pub udp_test_target: String,
    pub speed_test_page_size: Option<i32>,
    pub speed_test_delay_interval: Option<i32>,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

/// `Mux4RayItem` (3 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Mux4RayItem {
    pub concurrency: Option<i32>,
    pub xudp_concurrency: Option<i32>,
    pub xudp_proxy_udp443: Option<String>,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

/// `Mux4SboxItem` (3 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Mux4SboxItem {
    pub protocol: String,
    pub max_connections: i32,
    pub padding: Option<bool>,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

/// `HysteriaItem` (3 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HysteriaItem {
    pub up_mbps: i32,
    pub down_mbps: i32,
    /// Upstream default `Hysteria2DefaultHopInt = 30`.
    pub hop_interval: i32,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

impl Default for HysteriaItem {
    fn default() -> Self {
        Self {
            up_mbps: 0,
            down_mbps: 0,
            hop_interval: 30,
            extra: ExtraMap::new(),
        }
    }
}

/// `ClashUIItem` (8 properties).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ClashUiItem {
    pub enable_ipv6: bool,
    pub enable_mixin_content: bool,
    pub proxies_sorting: i32,
    pub proxies_auto_refresh: bool,
    pub proxies_refresh_interval: i32,
    pub connections_auto_refresh: bool,
    pub connections_refresh_interval: i32,
    pub connections_column_item: Vec<ColumnDefinition>,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

/// `SystemProxyItem` (6 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SystemProxyItem {
    pub sys_proxy_type: SysProxyType,
    pub system_proxy_exceptions: String,
    pub not_proxy_local_address: bool,
    pub system_proxy_advanced_protocol: String,
    pub custom_system_proxy_pac_path: Option<String>,
    pub custom_system_proxy_script_path: Option<String>,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

impl Default for SystemProxyItem {
    fn default() -> Self {
        Self {
            sys_proxy_type: SysProxyType::ForcedClear,
            system_proxy_exceptions: String::new(),
            not_proxy_local_address: true,
            system_proxy_advanced_protocol: String::new(),
            custom_system_proxy_pac_path: None,
            custom_system_proxy_script_path: None,
            extra: ExtraMap::new(),
        }
    }
}

/// `WebDavItem` (4 properties). Values are secrets; never log them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct WebDavItem {
    pub url: Option<String>,
    pub user_name: Option<String>,
    pub password: Option<String>,
    pub dir_name: Option<String>,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

/// `CheckUpdateItem` (3 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CheckUpdateItem {
    pub check_pre_release_update: bool,
    pub update_via_proxy: bool,
    pub selected_core_types: Option<Vec<i32>>,
    #[serde(flatten)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Fragment4RayItem {
    pub packets: Option<String>,
    pub lengths: Option<Vec<String>>,
    pub delays: Option<Vec<String>>,
    pub max_split: Option<String>,
    /// Legacy migration field; preserved, not used for generation.
    pub length: Option<String>,
    /// Legacy migration field; preserved, not used for generation.
    pub interval: Option<String>,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

/// `SimpleDNSItem` (18 properties).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SimpleDnsItem {
    pub use_system_hosts: Option<bool>,
    pub add_common_hosts: Option<bool>,
    pub fake_ip: Option<bool>,
    pub global_fake_ip: Option<bool>,
    pub fake_ip_range: Option<String>,
    pub block_binding_query: Option<bool>,
    pub block_aaaa_query: Option<bool>,
    pub direct_dns: Option<String>,
    pub remote_dns: Option<String>,
    pub bootstrap_dns: Option<String>,
    pub strategy4_freedom: Option<String>,
    pub strategy4_proxy: Option<String>,
    pub strategy4_proxy_dial: Option<String>,
    pub serve_stale: Option<bool>,
    pub parallel_query: Option<bool>,
    pub hosts: Option<String>,
    pub direct_expected_ips: Option<String>,
    pub enable_happy_eyeballs: Option<bool>,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

/// `HappyEyeballs4RayItem` (4 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HappyEyeballs4RayItem {
    pub try_delay_ms: Option<i32>,
    pub prioritize_ipv6: Option<bool>,
    pub interleave: Option<i32>,
    pub max_concurrent_try: Option<i32>,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

/// The guiNConfig.json root (`Config`, 25 properties: 2 ids + 23 items).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppSettings {
    pub index_id: String,
    pub sub_index_id: String,

    pub core_basic_item: CoreBasicItem,
    pub tun_mode_item: TunModeItem,
    pub kcp_item: KcpItem,
    pub grpc_item: GrpcItem,
    pub routing_basic_item: RoutingBasicItem,
    pub gui_item: GuiItem,
    pub msg_ui_item: MsgUiItem,
    pub ui_item: UiItem,
    pub const_item: ConstItem,
    pub speed_test_item: SpeedTestItem,
    pub mux4_ray_item: Mux4RayItem,
    pub mux4_sbox_item: Mux4SboxItem,
    pub hysteria_item: HysteriaItem,
    pub clash_ui_item: ClashUiItem,
    pub system_proxy_item: SystemProxyItem,
    pub web_dav_item: WebDavItem,
    pub check_update_item: CheckUpdateItem,
    pub fragment4_ray_item: Option<Fragment4RayItem>,
    pub inbound: Vec<InboundListener>,
    pub global_hotkeys: Vec<GlobalHotkey>,
    pub core_type_item: Vec<CoreTypeBinding>,
    pub simple_dns_item: SimpleDnsItem,
    pub happy_eyeballs4_ray_item: HappyEyeballs4RayItem,

    #[serde(flatten)]
    pub extra: ExtraMap,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hysteria_default_hop_interval() {
        assert_eq!(HysteriaItem::default().hop_interval, 30);
    }

    #[test]
    fn settings_defaults_match_upstream() {
        let s = AppSettings::default();
        assert!(s.gui_item.enable_log);
        assert_eq!(s.gui_item.tray_menu_servers_limit, 20);
        assert!(s.core_basic_item.enable_cache_file4_sbox);
        assert!(s.tun_mode_item.auto_route);
        assert_eq!(s.ui_item.main_gird_orientation, GirdOrientation::Vertical);
    }

    #[test]
    fn unknown_settings_key_preserved() {
        let raw = r#"{"gui_item":{"enable_log":true,"future_toggle":42}}"#;
        let s: AppSettings = serde_json::from_str(raw).unwrap();
        assert!(s.gui_item.enable_log);
        assert!(s.gui_item.extra.contains_key("future_toggle"));

        let out = serde_json::to_value(&s).unwrap();
        assert_eq!(out["gui_item"]["future_toggle"], 42);
    }
}
