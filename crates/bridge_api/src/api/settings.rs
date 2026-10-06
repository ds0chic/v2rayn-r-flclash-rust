//! FRB settings surface: strongly typed DTOs for the whole `guiNConfig.json`
//! tree, plus `get_settings` / `save_settings` / `save_settings_group`.
//!
//! Every group carries an `extra_json` field so unknown keys loaded from disk
//! survive a UI round trip. The save result reports the fields that changed
//! together with their `apply_timing` (and the restart-core / restart-app /
//! next-launch subsets) exactly as `compat/fields.settings.yaml` classifies
//! them.

use application::{normalize_for_save, validate_settings};
use domain::entities::{
    ColumnDefinition, CoreTypeBinding as DomainCoreTypeBinding, GlobalHotkey, InboundListener,
    WindowState,
};
use domain::{
    AppSettings, CheckUpdateItem, ClashUiItem, ConstItem, CoreBasicItem, Fragment4RayItem,
    GrpcItem, GuiItem, HappyEyeballs4RayItem, HysteriaItem, KcpItem, MsgUiItem, Mux4RayItem,
    Mux4SboxItem, RoutingBasicItem, SimpleDnsItem, SpeedTestItem, SystemProxyItem, TunModeItem,
    UiItem, WebDavItem,
};
use domain::{ConfigType, CoreType, GirdOrientation, InboundProtocol, SysProxyType};
use flutter_rust_bridge::frb;
use serde_json::Value;

use crate::api::contract::ErrorDto;
use crate::api::engine::{engine, error_dto};

fn extra_to_json(extra: &domain::ExtraMap) -> String {
    if extra.is_empty() {
        "{}".to_string()
    } else {
        serde_json::to_string(extra).unwrap_or_else(|_| "{}".to_string())
    }
}

fn json_to_extra(raw: &str) -> domain::ExtraMap {
    if raw.trim().is_empty() || raw.trim() == "{}" {
        return domain::ExtraMap::new();
    }
    serde_json::from_str(raw).unwrap_or_default()
}

fn group_revisions_to_json(revisions: &std::collections::BTreeMap<String, u64>) -> String {
    serde_json::to_string(revisions).unwrap_or_else(|_| "{}".to_string())
}

// ---------------------------------------------------------------------------
// Group DTOs
// ---------------------------------------------------------------------------

/// `CoreBasicItem` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct CoreBasicItemDto {
    pub log_enabled: bool,
    pub loglevel: Option<String>,
    pub def_fingerprint: Option<String>,
    pub def_user_agent: Option<String>,
    pub send_through: Option<String>,
    pub bind_interface: Option<String>,
    pub enable_fragment: bool,
    pub enable_final_fragment: bool,
    pub enable_cache_file4_sbox: bool,
    pub extra_json: String,
}

impl From<&CoreBasicItem> for CoreBasicItemDto {
    fn from(v: &CoreBasicItem) -> Self {
        Self {
            log_enabled: v.log_enabled,
            loglevel: v.loglevel.clone(),
            def_fingerprint: v.def_fingerprint.clone(),
            def_user_agent: v.def_user_agent.clone(),
            send_through: v.send_through.clone(),
            bind_interface: v.bind_interface.clone(),
            enable_fragment: v.enable_fragment,
            enable_final_fragment: v.enable_final_fragment,
            enable_cache_file4_sbox: v.enable_cache_file4_sbox,
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<CoreBasicItemDto> for CoreBasicItem {
    fn from(d: CoreBasicItemDto) -> Self {
        Self {
            log_enabled: d.log_enabled,
            loglevel: d.loglevel,
            def_fingerprint: d.def_fingerprint,
            def_user_agent: d.def_user_agent,
            send_through: d.send_through,
            bind_interface: d.bind_interface,
            enable_fragment: d.enable_fragment,
            enable_final_fragment: d.enable_final_fragment,
            enable_cache_file4_sbox: d.enable_cache_file4_sbox,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `TunModeItem` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct TunModeItemDto {
    pub enable_tun: bool,
    pub auto_route: bool,
    pub strict_route: bool,
    pub stack: Option<String>,
    pub mtu: i32,
    pub enable_ipv6_address: bool,
    pub icmp_routing: Option<String>,
    pub enable_legacy_protect: bool,
    pub route_exclude_address: Option<Vec<String>>,
    pub ipv4_address: Option<String>,
    pub ipv6_address: Option<String>,
    pub extra_json: String,
}

impl From<&TunModeItem> for TunModeItemDto {
    fn from(v: &TunModeItem) -> Self {
        Self {
            enable_tun: v.enable_tun,
            auto_route: v.auto_route,
            strict_route: v.strict_route,
            stack: v.stack.clone(),
            mtu: v.mtu,
            enable_ipv6_address: v.enable_ipv6_address,
            icmp_routing: v.icmp_routing.clone(),
            enable_legacy_protect: v.enable_legacy_protect,
            route_exclude_address: v.route_exclude_address.clone(),
            ipv4_address: v.ipv4_address.clone(),
            ipv6_address: v.ipv6_address.clone(),
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<TunModeItemDto> for TunModeItem {
    fn from(d: TunModeItemDto) -> Self {
        Self {
            enable_tun: d.enable_tun,
            auto_route: d.auto_route,
            strict_route: d.strict_route,
            stack: d.stack,
            mtu: d.mtu,
            enable_ipv6_address: d.enable_ipv6_address,
            icmp_routing: d.icmp_routing,
            enable_legacy_protect: d.enable_legacy_protect,
            route_exclude_address: d.route_exclude_address,
            ipv4_address: d.ipv4_address,
            ipv6_address: d.ipv6_address,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `KcpItem` DTO (UI tab commented out upstream).
#[derive(Debug, Clone, PartialEq)]
pub struct KcpItemDto {
    pub mtu: i32,
    pub tti: i32,
    pub uplink_capacity: i32,
    pub downlink_capacity: i32,
    pub cwnd_multiplier: i32,
    pub max_sending_window: i32,
    pub extra_json: String,
}

impl From<&KcpItem> for KcpItemDto {
    fn from(v: &KcpItem) -> Self {
        Self {
            mtu: v.mtu,
            tti: v.tti,
            uplink_capacity: v.uplink_capacity,
            downlink_capacity: v.downlink_capacity,
            cwnd_multiplier: v.cwnd_multiplier,
            max_sending_window: v.max_sending_window,
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<KcpItemDto> for KcpItem {
    fn from(d: KcpItemDto) -> Self {
        Self {
            mtu: d.mtu,
            tti: d.tti,
            uplink_capacity: d.uplink_capacity,
            downlink_capacity: d.downlink_capacity,
            cwnd_multiplier: d.cwnd_multiplier,
            max_sending_window: d.max_sending_window,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `GrpcItem` DTO (no UI control).
#[derive(Debug, Clone, PartialEq)]
pub struct GrpcItemDto {
    pub idle_timeout: Option<i32>,
    pub health_check_timeout: Option<i32>,
    pub permit_without_stream: Option<bool>,
    pub initial_windows_size: Option<i32>,
    pub extra_json: String,
}

impl From<&GrpcItem> for GrpcItemDto {
    fn from(v: &GrpcItem) -> Self {
        Self {
            idle_timeout: v.idle_timeout,
            health_check_timeout: v.health_check_timeout,
            permit_without_stream: v.permit_without_stream,
            initial_windows_size: v.initial_windows_size,
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<GrpcItemDto> for GrpcItem {
    fn from(d: GrpcItemDto) -> Self {
        Self {
            idle_timeout: d.idle_timeout,
            health_check_timeout: d.health_check_timeout,
            permit_without_stream: d.permit_without_stream,
            initial_windows_size: d.initial_windows_size,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `RoutingBasicItem` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct RoutingBasicItemDto {
    pub domain_strategy: Option<String>,
    pub domain_strategy4_singbox: Option<String>,
    pub routing_index_id: Option<String>,
    pub extra_json: String,
}

impl From<&RoutingBasicItem> for RoutingBasicItemDto {
    fn from(v: &RoutingBasicItem) -> Self {
        Self {
            domain_strategy: v.domain_strategy.clone(),
            domain_strategy4_singbox: v.domain_strategy4_singbox.clone(),
            routing_index_id: v.routing_index_id.clone(),
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<RoutingBasicItemDto> for RoutingBasicItem {
    fn from(d: RoutingBasicItemDto) -> Self {
        Self {
            domain_strategy: d.domain_strategy,
            domain_strategy4_singbox: d.domain_strategy4_singbox,
            routing_index_id: d.routing_index_id,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `GUIItem` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct GuiItemDto {
    pub auto_run: bool,
    pub enable_statistics: bool,
    pub display_real_time_speed: bool,
    pub keep_older_dedupl: bool,
    pub auto_update_interval: i32,
    pub tray_menu_servers_limit: i32,
    pub enable_hwa: bool,
    pub enable_log: bool,
    pub root_cert_provider: Option<String>,
    pub extra_json: String,
}

impl From<&GuiItem> for GuiItemDto {
    fn from(v: &GuiItem) -> Self {
        Self {
            auto_run: v.auto_run,
            enable_statistics: v.enable_statistics,
            display_real_time_speed: v.display_real_time_speed,
            keep_older_dedupl: v.keep_older_dedupl,
            auto_update_interval: v.auto_update_interval,
            tray_menu_servers_limit: v.tray_menu_servers_limit,
            enable_hwa: v.enable_hwa,
            enable_log: v.enable_log,
            root_cert_provider: v.root_cert_provider.clone(),
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<GuiItemDto> for GuiItem {
    fn from(d: GuiItemDto) -> Self {
        Self {
            auto_run: d.auto_run,
            enable_statistics: d.enable_statistics,
            display_real_time_speed: d.display_real_time_speed,
            keep_older_dedupl: d.keep_older_dedupl,
            auto_update_interval: d.auto_update_interval,
            tray_menu_servers_limit: d.tray_menu_servers_limit,
            enable_hwa: d.enable_hwa,
            enable_log: d.enable_log,
            root_cert_provider: d.root_cert_provider,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `MsgUIItem` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct MsgUiItemDto {
    pub main_msg_filter: Option<String>,
    pub auto_refresh: Option<bool>,
    pub extra_json: String,
}

impl From<&MsgUiItem> for MsgUiItemDto {
    fn from(v: &MsgUiItem) -> Self {
        Self {
            main_msg_filter: v.main_msg_filter.clone(),
            auto_refresh: v.auto_refresh,
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<MsgUiItemDto> for MsgUiItem {
    fn from(d: MsgUiItemDto) -> Self {
        Self {
            main_msg_filter: d.main_msg_filter,
            auto_refresh: d.auto_refresh,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// A main-table or Clash connection column (`ColumnItem`).
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnItemDto {
    pub name: String,
    pub width: i32,
    pub index: i32,
    pub extra_json: String,
}

impl From<&ColumnDefinition> for ColumnItemDto {
    fn from(v: &ColumnDefinition) -> Self {
        Self {
            name: v.name.clone(),
            width: v.width,
            index: v.index,
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<ColumnItemDto> for ColumnDefinition {
    fn from(d: ColumnItemDto) -> Self {
        Self {
            name: d.name,
            width: d.width,
            index: d.index,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// A persisted window geometry row (`WindowSizeItem`).
#[derive(Debug, Clone, PartialEq)]
pub struct WindowSizeItemDto {
    pub type_name: String,
    pub width: i32,
    pub height: i32,
    pub main_grid_height1: i32,
    pub main_grid_height2: i32,
    /// `EGirdOrientation` numeric value (0/1/2).
    pub orientation: i32,
    pub extra_json: String,
}

impl From<&WindowState> for WindowSizeItemDto {
    fn from(v: &WindowState) -> Self {
        Self {
            type_name: v.type_name.clone(),
            width: v.width,
            height: v.height,
            main_grid_height1: v.main_grid_height1,
            main_grid_height2: v.main_grid_height2,
            orientation: v.orientation.value(),
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<WindowSizeItemDto> for WindowState {
    fn from(d: WindowSizeItemDto) -> Self {
        Self {
            type_name: d.type_name,
            width: d.width,
            height: d.height,
            main_grid_height1: d.main_grid_height1,
            main_grid_height2: d.main_grid_height2,
            orientation: GirdOrientation::from_value(d.orientation).unwrap_or_default(),
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `UIItem` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct UiItemDto {
    pub enable_auto_adjust_main_lv_col_width: bool,
    pub main_gird_height1: i32,
    pub main_gird_height2: i32,
    /// `EGirdOrientation` numeric value (0/1/2).
    pub main_gird_orientation: i32,
    pub color_primary_name: Option<String>,
    pub current_theme: Option<String>,
    pub current_language: Option<String>,
    pub current_font_family: Option<String>,
    pub current_font_size: i32,
    pub enable_drag_drop_sort: bool,
    pub double_click2_activate: bool,
    pub auto_hide_startup: bool,
    pub hide2_tray_when_close: bool,
    pub macos_show_in_dock: bool,
    pub main_column_item: Vec<ColumnItemDto>,
    pub window_size_item: Vec<WindowSizeItemDto>,
    pub hide_column_ip_info: bool,
    pub extra_json: String,
}

impl From<&UiItem> for UiItemDto {
    fn from(v: &UiItem) -> Self {
        Self {
            enable_auto_adjust_main_lv_col_width: v.enable_auto_adjust_main_lv_col_width,
            main_gird_height1: v.main_gird_height1,
            main_gird_height2: v.main_gird_height2,
            main_gird_orientation: v.main_gird_orientation.value(),
            color_primary_name: v.color_primary_name.clone(),
            current_theme: v.current_theme.clone(),
            current_language: v.current_language.clone(),
            current_font_family: v.current_font_family.clone(),
            current_font_size: v.current_font_size,
            enable_drag_drop_sort: v.enable_drag_drop_sort,
            double_click2_activate: v.double_click2_activate,
            auto_hide_startup: v.auto_hide_startup,
            hide2_tray_when_close: v.hide2_tray_when_close,
            macos_show_in_dock: v.macos_show_in_dock,
            main_column_item: v.main_column_item.iter().map(ColumnItemDto::from).collect(),
            window_size_item: v
                .window_size_item
                .iter()
                .map(WindowSizeItemDto::from)
                .collect(),
            hide_column_ip_info: v.hide_column_ip_info,
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<UiItemDto> for UiItem {
    fn from(d: UiItemDto) -> Self {
        Self {
            enable_auto_adjust_main_lv_col_width: d.enable_auto_adjust_main_lv_col_width,
            main_gird_height1: d.main_gird_height1,
            main_gird_height2: d.main_gird_height2,
            main_gird_orientation: GirdOrientation::from_value(d.main_gird_orientation)
                .unwrap_or_default(),
            color_primary_name: d.color_primary_name,
            current_theme: d.current_theme,
            current_language: d.current_language,
            current_font_family: d.current_font_family,
            current_font_size: d.current_font_size,
            enable_drag_drop_sort: d.enable_drag_drop_sort,
            double_click2_activate: d.double_click2_activate,
            auto_hide_startup: d.auto_hide_startup,
            hide2_tray_when_close: d.hide2_tray_when_close,
            macos_show_in_dock: d.macos_show_in_dock,
            main_column_item: d
                .main_column_item
                .into_iter()
                .map(ColumnDefinition::from)
                .collect(),
            window_size_item: d
                .window_size_item
                .into_iter()
                .map(WindowState::from)
                .collect(),
            hide_column_ip_info: d.hide_column_ip_info,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `ConstItem` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstItemDto {
    pub sub_convert_url: Option<String>,
    pub geo_source_url: Option<String>,
    pub srs_source_url: Option<String>,
    pub route_rules_template_source_url: Option<String>,
    pub extra_json: String,
}

impl From<&ConstItem> for ConstItemDto {
    fn from(v: &ConstItem) -> Self {
        Self {
            sub_convert_url: v.sub_convert_url.clone(),
            geo_source_url: v.geo_source_url.clone(),
            srs_source_url: v.srs_source_url.clone(),
            route_rules_template_source_url: v.route_rules_template_source_url.clone(),
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<ConstItemDto> for ConstItem {
    fn from(d: ConstItemDto) -> Self {
        Self {
            sub_convert_url: d.sub_convert_url,
            geo_source_url: d.geo_source_url,
            srs_source_url: d.srs_source_url,
            route_rules_template_source_url: d.route_rules_template_source_url,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `SpeedTestItem` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeedTestItemDto {
    pub speed_test_timeout: i32,
    pub speed_test_url: Option<String>,
    pub speed_ping_test_url: Option<String>,
    pub mixed_concurrency_count: i32,
    pub ipapi_url: Option<String>,
    pub udp_test_target: Option<String>,
    pub speed_test_page_size: Option<i32>,
    pub speed_test_delay_interval: Option<i32>,
    pub extra_json: String,
}

impl From<&SpeedTestItem> for SpeedTestItemDto {
    fn from(v: &SpeedTestItem) -> Self {
        Self {
            speed_test_timeout: v.speed_test_timeout,
            speed_test_url: v.speed_test_url.clone(),
            speed_ping_test_url: v.speed_ping_test_url.clone(),
            mixed_concurrency_count: v.mixed_concurrency_count,
            ipapi_url: v.ipapi_url.clone(),
            udp_test_target: v.udp_test_target.clone(),
            speed_test_page_size: v.speed_test_page_size,
            speed_test_delay_interval: v.speed_test_delay_interval,
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<SpeedTestItemDto> for SpeedTestItem {
    fn from(d: SpeedTestItemDto) -> Self {
        Self {
            speed_test_timeout: d.speed_test_timeout,
            speed_test_url: d.speed_test_url,
            speed_ping_test_url: d.speed_ping_test_url,
            mixed_concurrency_count: d.mixed_concurrency_count,
            ipapi_url: d.ipapi_url,
            udp_test_target: d.udp_test_target,
            speed_test_page_size: d.speed_test_page_size,
            speed_test_delay_interval: d.speed_test_delay_interval,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `Mux4RayItem` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct Mux4RayItemDto {
    pub concurrency: Option<i32>,
    pub xudp_concurrency: Option<i32>,
    pub xudp_proxy_udp443: Option<String>,
    pub extra_json: String,
}

impl From<&Mux4RayItem> for Mux4RayItemDto {
    fn from(v: &Mux4RayItem) -> Self {
        Self {
            concurrency: v.concurrency,
            xudp_concurrency: v.xudp_concurrency,
            xudp_proxy_udp443: v.xudp_proxy_udp443.clone(),
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<Mux4RayItemDto> for Mux4RayItem {
    fn from(d: Mux4RayItemDto) -> Self {
        Self {
            concurrency: d.concurrency,
            xudp_concurrency: d.xudp_concurrency,
            xudp_proxy_udp443: d.xudp_proxy_udp443,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `Mux4SboxItem` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct Mux4SboxItemDto {
    pub protocol: Option<String>,
    pub max_connections: i32,
    pub padding: Option<bool>,
    pub extra_json: String,
}

impl From<&Mux4SboxItem> for Mux4SboxItemDto {
    fn from(v: &Mux4SboxItem) -> Self {
        Self {
            protocol: v.protocol.clone(),
            max_connections: v.max_connections,
            padding: v.padding,
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<Mux4SboxItemDto> for Mux4SboxItem {
    fn from(d: Mux4SboxItemDto) -> Self {
        Self {
            protocol: d.protocol,
            max_connections: d.max_connections,
            padding: d.padding,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `HysteriaItem` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct HysteriaItemDto {
    pub up_mbps: i32,
    pub down_mbps: i32,
    pub hop_interval: i32,
    pub extra_json: String,
}

impl From<&HysteriaItem> for HysteriaItemDto {
    fn from(v: &HysteriaItem) -> Self {
        Self {
            up_mbps: v.up_mbps,
            down_mbps: v.down_mbps,
            hop_interval: v.hop_interval,
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<HysteriaItemDto> for HysteriaItem {
    fn from(d: HysteriaItemDto) -> Self {
        Self {
            up_mbps: d.up_mbps,
            down_mbps: d.down_mbps,
            hop_interval: d.hop_interval,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `ClashUIItem` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct ClashUiItemDto {
    pub enable_ipv6: bool,
    pub enable_mixin_content: bool,
    pub proxies_sorting: i32,
    pub proxies_auto_refresh: bool,
    pub proxies_refresh_interval: i32,
    pub connections_auto_refresh: bool,
    pub connections_refresh_interval: i32,
    pub connections_column_item: Vec<ColumnItemDto>,
    pub extra_json: String,
}

impl From<&ClashUiItem> for ClashUiItemDto {
    fn from(v: &ClashUiItem) -> Self {
        Self {
            enable_ipv6: v.enable_ipv6,
            enable_mixin_content: v.enable_mixin_content,
            proxies_sorting: v.proxies_sorting,
            proxies_auto_refresh: v.proxies_auto_refresh,
            proxies_refresh_interval: v.proxies_refresh_interval,
            connections_auto_refresh: v.connections_auto_refresh,
            connections_refresh_interval: v.connections_refresh_interval,
            connections_column_item: v
                .connections_column_item
                .iter()
                .map(ColumnItemDto::from)
                .collect(),
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<ClashUiItemDto> for ClashUiItem {
    fn from(d: ClashUiItemDto) -> Self {
        Self {
            enable_ipv6: d.enable_ipv6,
            enable_mixin_content: d.enable_mixin_content,
            proxies_sorting: d.proxies_sorting,
            proxies_auto_refresh: d.proxies_auto_refresh,
            proxies_refresh_interval: d.proxies_refresh_interval,
            connections_auto_refresh: d.connections_auto_refresh,
            connections_refresh_interval: d.connections_refresh_interval,
            connections_column_item: d
                .connections_column_item
                .into_iter()
                .map(ColumnDefinition::from)
                .collect(),
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `SystemProxyItem` DTO. Stored only in this task; the real WinINET work is T13.
#[derive(Debug, Clone, PartialEq)]
pub struct SystemProxyItemDto {
    /// `ESysProxyType` numeric value (0..3).
    pub sys_proxy_type: i32,
    pub system_proxy_exceptions: Option<String>,
    pub not_proxy_local_address: bool,
    pub system_proxy_advanced_protocol: Option<String>,
    pub custom_system_proxy_pac_path: Option<String>,
    pub custom_system_proxy_script_path: Option<String>,
    pub extra_json: String,
}

impl From<&SystemProxyItem> for SystemProxyItemDto {
    fn from(v: &SystemProxyItem) -> Self {
        Self {
            sys_proxy_type: v.sys_proxy_type.value(),
            system_proxy_exceptions: v.system_proxy_exceptions.clone(),
            not_proxy_local_address: v.not_proxy_local_address,
            system_proxy_advanced_protocol: v.system_proxy_advanced_protocol.clone(),
            custom_system_proxy_pac_path: v.custom_system_proxy_pac_path.clone(),
            custom_system_proxy_script_path: v.custom_system_proxy_script_path.clone(),
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<SystemProxyItemDto> for SystemProxyItem {
    fn from(d: SystemProxyItemDto) -> Self {
        Self {
            sys_proxy_type: SysProxyType::from_value(d.sys_proxy_type).unwrap_or_default(),
            system_proxy_exceptions: d.system_proxy_exceptions,
            not_proxy_local_address: d.not_proxy_local_address,
            system_proxy_advanced_protocol: d.system_proxy_advanced_protocol,
            custom_system_proxy_pac_path: d.custom_system_proxy_pac_path,
            custom_system_proxy_script_path: d.custom_system_proxy_script_path,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `WebDavItem` DTO. Values are secrets; the UI must not log them.
#[derive(Debug, Clone, PartialEq)]
pub struct WebDavItemDto {
    pub url: Option<String>,
    pub user_name: Option<String>,
    pub password: Option<String>,
    pub dir_name: Option<String>,
    pub extra_json: String,
}

impl From<&WebDavItem> for WebDavItemDto {
    fn from(v: &WebDavItem) -> Self {
        Self {
            url: v.url.clone(),
            user_name: v.user_name.clone(),
            password: v.password.clone(),
            dir_name: v.dir_name.clone(),
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<WebDavItemDto> for WebDavItem {
    fn from(d: WebDavItemDto) -> Self {
        Self {
            url: d.url,
            user_name: d.user_name,
            password: d.password,
            dir_name: d.dir_name,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `CheckUpdateItem` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckUpdateItemDto {
    pub check_pre_release_update: bool,
    pub update_via_proxy: bool,
    pub selected_core_types: Option<Vec<String>>,
    pub extra_json: String,
}

impl From<&CheckUpdateItem> for CheckUpdateItemDto {
    fn from(v: &CheckUpdateItem) -> Self {
        Self {
            check_pre_release_update: v.check_pre_release_update,
            update_via_proxy: v.update_via_proxy,
            selected_core_types: v.selected_core_types.clone(),
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<CheckUpdateItemDto> for CheckUpdateItem {
    fn from(d: CheckUpdateItemDto) -> Self {
        Self {
            check_pre_release_update: d.check_pre_release_update,
            update_via_proxy: d.update_via_proxy,
            selected_core_types: d.selected_core_types,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `Fragment4RayItem` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct Fragment4RayItemDto {
    pub packets: Option<String>,
    pub lengths: Option<Vec<String>>,
    pub delays: Option<Vec<String>>,
    pub max_split: Option<String>,
    pub length: Option<String>,
    pub interval: Option<String>,
    pub extra_json: String,
}

impl From<&Fragment4RayItem> for Fragment4RayItemDto {
    fn from(v: &Fragment4RayItem) -> Self {
        Self {
            packets: v.packets.clone(),
            lengths: v.lengths.clone(),
            delays: v.delays.clone(),
            max_split: v.max_split.clone(),
            length: v.length.clone(),
            interval: v.interval.clone(),
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<Fragment4RayItemDto> for Fragment4RayItem {
    fn from(d: Fragment4RayItemDto) -> Self {
        Self {
            packets: d.packets,
            lengths: d.lengths,
            delays: d.delays,
            max_split: d.max_split,
            length: d.length,
            interval: d.interval,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `InItem` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct InboundListenerDto {
    pub local_port: i32,
    /// `EInboundProtocol` numeric value.
    pub protocol: i32,
    pub udp_enabled: bool,
    pub sniffing_enabled: bool,
    pub dest_override: Option<Vec<String>>,
    pub route_only: bool,
    pub allow_lan_conn: bool,
    pub new_port4_lan: bool,
    pub user: String,
    pub pass: String,
    pub second_local_port_enabled: bool,
    pub extra_json: String,
}

impl From<&InboundListener> for InboundListenerDto {
    fn from(v: &InboundListener) -> Self {
        Self {
            local_port: v.local_port,
            protocol: v.protocol.value(),
            udp_enabled: v.udp_enabled,
            sniffing_enabled: v.sniffing_enabled,
            dest_override: v.dest_override.clone(),
            route_only: v.route_only,
            allow_lan_conn: v.allow_lan_conn,
            new_port4_lan: v.new_port4_lan,
            user: v.user.clone(),
            pass: v.pass.clone(),
            second_local_port_enabled: v.second_local_port_enabled,
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<InboundListenerDto> for InboundListener {
    fn from(d: InboundListenerDto) -> Self {
        Self {
            local_port: d.local_port,
            protocol: InboundProtocol::from_value(d.protocol).unwrap_or_default(),
            udp_enabled: d.udp_enabled,
            sniffing_enabled: d.sniffing_enabled,
            dest_override: d.dest_override,
            route_only: d.route_only,
            allow_lan_conn: d.allow_lan_conn,
            new_port4_lan: d.new_port4_lan,
            user: d.user,
            pass: d.pass,
            second_local_port_enabled: d.second_local_port_enabled,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `KeyEventItem` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct GlobalHotkeyDto {
    pub action: i32,
    pub alt: bool,
    pub control: bool,
    pub shift: bool,
    pub key_code: Option<i32>,
    pub extra_json: String,
}

impl From<&GlobalHotkey> for GlobalHotkeyDto {
    fn from(v: &GlobalHotkey) -> Self {
        Self {
            action: v.action,
            alt: v.alt,
            control: v.control,
            shift: v.shift,
            key_code: v.key_code,
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<GlobalHotkeyDto> for GlobalHotkey {
    fn from(d: GlobalHotkeyDto) -> Self {
        Self {
            action: d.action,
            alt: d.alt,
            control: d.control,
            shift: d.shift,
            key_code: d.key_code,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `CoreTypeItem` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct CoreTypeBindingDto {
    pub config_type: ConfigType,
    pub core_type: CoreType,
}

impl From<&DomainCoreTypeBinding> for CoreTypeBindingDto {
    fn from(v: &DomainCoreTypeBinding) -> Self {
        Self {
            config_type: v.config_type,
            core_type: v.core_type,
        }
    }
}

impl From<CoreTypeBindingDto> for DomainCoreTypeBinding {
    fn from(d: CoreTypeBindingDto) -> Self {
        DomainCoreTypeBinding::new(d.config_type, d.core_type)
    }
}

/// `SimpleDNSItem` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct SimpleDnsItemDto {
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
    pub extra_json: String,
}

impl From<&SimpleDnsItem> for SimpleDnsItemDto {
    fn from(v: &SimpleDnsItem) -> Self {
        Self {
            use_system_hosts: v.use_system_hosts,
            add_common_hosts: v.add_common_hosts,
            fake_ip: v.fake_ip,
            global_fake_ip: v.global_fake_ip,
            fake_ip_range: v.fake_ip_range.clone(),
            block_binding_query: v.block_binding_query,
            block_aaaa_query: v.block_aaaa_query,
            direct_dns: v.direct_dns.clone(),
            remote_dns: v.remote_dns.clone(),
            bootstrap_dns: v.bootstrap_dns.clone(),
            strategy4_freedom: v.strategy4_freedom.clone(),
            strategy4_proxy: v.strategy4_proxy.clone(),
            strategy4_proxy_dial: v.strategy4_proxy_dial.clone(),
            serve_stale: v.serve_stale,
            parallel_query: v.parallel_query,
            hosts: v.hosts.clone(),
            direct_expected_ips: v.direct_expected_ips.clone(),
            enable_happy_eyeballs: v.enable_happy_eyeballs,
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<SimpleDnsItemDto> for SimpleDnsItem {
    fn from(d: SimpleDnsItemDto) -> Self {
        Self {
            use_system_hosts: d.use_system_hosts,
            add_common_hosts: d.add_common_hosts,
            fake_ip: d.fake_ip,
            global_fake_ip: d.global_fake_ip,
            fake_ip_range: d.fake_ip_range,
            block_binding_query: d.block_binding_query,
            block_aaaa_query: d.block_aaaa_query,
            direct_dns: d.direct_dns,
            remote_dns: d.remote_dns,
            bootstrap_dns: d.bootstrap_dns,
            strategy4_freedom: d.strategy4_freedom,
            strategy4_proxy: d.strategy4_proxy,
            strategy4_proxy_dial: d.strategy4_proxy_dial,
            serve_stale: d.serve_stale,
            parallel_query: d.parallel_query,
            hosts: d.hosts,
            direct_expected_ips: d.direct_expected_ips,
            enable_happy_eyeballs: d.enable_happy_eyeballs,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// `HappyEyeballs4RayItem` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct HappyEyeballs4RayItemDto {
    pub try_delay_ms: Option<i32>,
    pub prioritize_ipv6: Option<bool>,
    pub interleave: Option<i32>,
    pub max_concurrent_try: Option<i32>,
    pub extra_json: String,
}

impl From<&HappyEyeballs4RayItem> for HappyEyeballs4RayItemDto {
    fn from(v: &HappyEyeballs4RayItem) -> Self {
        Self {
            try_delay_ms: v.try_delay_ms,
            prioritize_ipv6: v.prioritize_ipv6,
            interleave: v.interleave,
            max_concurrent_try: v.max_concurrent_try,
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<HappyEyeballs4RayItemDto> for HappyEyeballs4RayItem {
    fn from(d: HappyEyeballs4RayItemDto) -> Self {
        Self {
            try_delay_ms: d.try_delay_ms,
            prioritize_ipv6: d.prioritize_ipv6,
            interleave: d.interleave,
            max_concurrent_try: d.max_concurrent_try,
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// The full settings tree DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsDto {
    pub index_id: Option<String>,
    pub sub_index_id: Option<String>,
    pub core_basic_item: CoreBasicItemDto,
    pub tun_mode_item: TunModeItemDto,
    pub kcp_item: KcpItemDto,
    pub grpc_item: GrpcItemDto,
    pub routing_basic_item: RoutingBasicItemDto,
    pub gui_item: GuiItemDto,
    pub msg_ui_item: MsgUiItemDto,
    pub ui_item: UiItemDto,
    pub const_item: ConstItemDto,
    pub speed_test_item: SpeedTestItemDto,
    pub mux4_ray_item: Mux4RayItemDto,
    pub mux4_sbox_item: Mux4SboxItemDto,
    pub hysteria_item: HysteriaItemDto,
    pub clash_ui_item: ClashUiItemDto,
    pub system_proxy_item: SystemProxyItemDto,
    pub web_dav_item: WebDavItemDto,
    pub check_update_item: CheckUpdateItemDto,
    pub fragment4_ray_item: Option<Fragment4RayItemDto>,
    pub inbound: Vec<InboundListenerDto>,
    pub global_hotkeys: Vec<GlobalHotkeyDto>,
    pub core_type_item: Option<Vec<CoreTypeBindingDto>>,
    pub simple_dns_item: SimpleDnsItemDto,
    pub happy_eyeballs4_ray_item: HappyEyeballs4RayItemDto,
    pub extra_json: String,
}

impl From<&AppSettings> for SettingsDto {
    fn from(v: &AppSettings) -> Self {
        Self {
            index_id: v.index_id.clone(),
            sub_index_id: v.sub_index_id.clone(),
            core_basic_item: CoreBasicItemDto::from(&v.core_basic_item),
            tun_mode_item: TunModeItemDto::from(&v.tun_mode_item),
            kcp_item: KcpItemDto::from(&v.kcp_item),
            grpc_item: GrpcItemDto::from(&v.grpc_item),
            routing_basic_item: RoutingBasicItemDto::from(&v.routing_basic_item),
            gui_item: GuiItemDto::from(&v.gui_item),
            msg_ui_item: MsgUiItemDto::from(&v.msg_ui_item),
            ui_item: UiItemDto::from(&v.ui_item),
            const_item: ConstItemDto::from(&v.const_item),
            speed_test_item: SpeedTestItemDto::from(&v.speed_test_item),
            mux4_ray_item: Mux4RayItemDto::from(&v.mux4_ray_item),
            mux4_sbox_item: Mux4SboxItemDto::from(&v.mux4_sbox_item),
            hysteria_item: HysteriaItemDto::from(&v.hysteria_item),
            clash_ui_item: ClashUiItemDto::from(&v.clash_ui_item),
            system_proxy_item: SystemProxyItemDto::from(&v.system_proxy_item),
            web_dav_item: WebDavItemDto::from(&v.web_dav_item),
            check_update_item: CheckUpdateItemDto::from(&v.check_update_item),
            fragment4_ray_item: v.fragment4_ray_item.as_ref().map(Fragment4RayItemDto::from),
            inbound: v.inbound.iter().map(InboundListenerDto::from).collect(),
            global_hotkeys: v.global_hotkeys.iter().map(GlobalHotkeyDto::from).collect(),
            core_type_item: v
                .core_type_item
                .as_ref()
                .map(|items| items.iter().map(CoreTypeBindingDto::from).collect()),
            simple_dns_item: SimpleDnsItemDto::from(&v.simple_dns_item),
            happy_eyeballs4_ray_item: HappyEyeballs4RayItemDto::from(&v.happy_eyeballs4_ray_item),
            extra_json: extra_to_json(&v.extra),
        }
    }
}

impl From<SettingsDto> for AppSettings {
    fn from(d: SettingsDto) -> Self {
        Self {
            index_id: d.index_id,
            sub_index_id: d.sub_index_id,
            core_basic_item: d.core_basic_item.into(),
            tun_mode_item: d.tun_mode_item.into(),
            kcp_item: d.kcp_item.into(),
            grpc_item: d.grpc_item.into(),
            routing_basic_item: d.routing_basic_item.into(),
            gui_item: d.gui_item.into(),
            msg_ui_item: d.msg_ui_item.into(),
            ui_item: d.ui_item.into(),
            const_item: d.const_item.into(),
            speed_test_item: d.speed_test_item.into(),
            mux4_ray_item: d.mux4_ray_item.into(),
            mux4_sbox_item: d.mux4_sbox_item.into(),
            hysteria_item: d.hysteria_item.into(),
            clash_ui_item: d.clash_ui_item.into(),
            system_proxy_item: d.system_proxy_item.into(),
            web_dav_item: d.web_dav_item.into(),
            check_update_item: d.check_update_item.into(),
            fragment4_ray_item: d.fragment4_ray_item.map(Fragment4RayItem::from),
            inbound: d.inbound.into_iter().map(InboundListener::from).collect(),
            global_hotkeys: d
                .global_hotkeys
                .into_iter()
                .map(GlobalHotkey::from)
                .collect(),
            core_type_item: d
                .core_type_item
                .map(|items| items.into_iter().map(DomainCoreTypeBinding::from).collect()),
            simple_dns_item: d.simple_dns_item.into(),
            happy_eyeballs4_ray_item: d.happy_eyeballs4_ray_item.into(),
            extra: json_to_extra(&d.extra_json),
        }
    }
}

/// One classified settings change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsChangeDto {
    pub group: String,
    pub field: String,
    pub path: String,
    pub timing: String,
}

/// `get_settings` result.
#[derive(Clone)]
pub struct SettingsLoadDto {
    pub ok: bool,
    pub revision: u64,
    pub group_revisions_json: String,
    pub settings: Option<SettingsDto>,
    /// Canonical `guiNConfig.json` `Config` JSON (PascalCase, unknown keys and
    /// null/empty distinctions preserved). The UI edits this directly and saves
    /// it back through [`save_settings_json`].
    pub settings_json: String,
    pub error: Option<ErrorDto>,
}

/// `save_settings` / `save_settings_group` result.
#[derive(Clone)]
pub struct SaveSettingsResult {
    pub ok: bool,
    pub new_revision: Option<u64>,
    pub changes: Vec<SettingsChangeDto>,
    pub restart_core_fields: Vec<String>,
    pub restart_app_fields: Vec<String>,
    pub next_launch_fields: Vec<String>,
    pub error: Option<ErrorDto>,
}

fn changes_to_dto(changes: &[domain::SettingsChange]) -> Vec<SettingsChangeDto> {
    changes
        .iter()
        .map(|c| SettingsChangeDto {
            group: c.group.clone(),
            field: c.field.clone(),
            path: c.path(),
            timing: c.timing.as_str().to_string(),
        })
        .collect()
}

/// `get_settings` — the normalised tree plus revision counters.
#[frb(sync)]
pub fn get_settings() -> SettingsLoadDto {
    match engine().load_settings() {
        Ok(loaded) => SettingsLoadDto {
            ok: true,
            revision: loaded.revision,
            group_revisions_json: group_revisions_to_json(&loaded.group_revisions),
            settings: Some(SettingsDto::from(&loaded.settings)),
            settings_json: serde_json::to_string(&loaded.settings)
                .unwrap_or_else(|_| "{}".to_string()),
            error: None,
        },
        Err(error) => SettingsLoadDto {
            ok: false,
            revision: 0,
            group_revisions_json: "{}".to_string(),
            settings: None,
            settings_json: "{}".to_string(),
            error: Some(error_dto(error)),
        },
    }
}

/// `save_settings_json` — whole-tree save from the canonical `Config` JSON.
///
/// This is the UI's primary save path: it keeps unknown keys and the
/// null/empty-string distinction intact without round-tripping every DTO.
#[frb(sync)]
pub fn save_settings_json(settings_json: String, expected_revision: u64) -> SaveSettingsResult {
    let settings: AppSettings = match serde_json::from_str(&settings_json) {
        Ok(settings) => settings,
        Err(error) => {
            return SaveSettingsResult {
                ok: false,
                new_revision: None,
                changes: Vec::new(),
                restart_core_fields: Vec::new(),
                restart_app_fields: Vec::new(),
                next_launch_fields: Vec::new(),
                error: Some(error_dto(
                    domain::DomainError::new(domain::codes::FIELD_FORMAT, "error.settings_json")
                        .with_detail(error.to_string()),
                )),
            }
        }
    };
    match engine().save_settings(normalize_for_save(settings), expected_revision) {
        Ok(outcome) => SaveSettingsResult {
            ok: true,
            new_revision: Some(outcome.new_revision),
            changes: changes_to_dto(&outcome.changes),
            restart_core_fields: outcome.restart_core_fields,
            restart_app_fields: outcome.restart_app_fields,
            next_launch_fields: outcome.next_launch_fields,
            error: None,
        },
        Err(error) => SaveSettingsResult {
            ok: false,
            new_revision: None,
            changes: Vec::new(),
            restart_core_fields: Vec::new(),
            restart_app_fields: Vec::new(),
            next_launch_fields: Vec::new(),
            error: Some(error_dto(error)),
        },
    }
}

/// `settings_revision` — the current whole-tree revision.
#[frb(sync)]
pub fn settings_revision() -> u64 {
    engine().settings_revision()
}

/// `save_settings` — whole-tree optimistic save.
#[frb(sync)]
pub fn save_settings(settings: SettingsDto, expected_revision: u64) -> SaveSettingsResult {
    let domain = normalize_for_save(AppSettings::from(settings));
    match engine().save_settings(domain, expected_revision) {
        Ok(outcome) => SaveSettingsResult {
            ok: true,
            new_revision: Some(outcome.new_revision),
            changes: changes_to_dto(&outcome.changes),
            restart_core_fields: outcome.restart_core_fields,
            restart_app_fields: outcome.restart_app_fields,
            next_launch_fields: outcome.next_launch_fields,
            error: None,
        },
        Err(error) => SaveSettingsResult {
            ok: false,
            new_revision: None,
            changes: Vec::new(),
            restart_core_fields: Vec::new(),
            restart_app_fields: Vec::new(),
            next_launch_fields: Vec::new(),
            error: Some(error_dto(error)),
        },
    }
}

/// `save_settings_group` — replace one top-level group.
#[frb(sync)]
pub fn save_settings_group(
    group: String,
    patch_json: String,
    expected_revision: u64,
) -> SaveSettingsResult {
    let patch: Value = match serde_json::from_str(&patch_json) {
        Ok(value) => value,
        Err(error) => {
            return SaveSettingsResult {
                ok: false,
                new_revision: None,
                changes: Vec::new(),
                restart_core_fields: Vec::new(),
                restart_app_fields: Vec::new(),
                next_launch_fields: Vec::new(),
                error: Some(error_dto(
                    domain::DomainError::new(domain::codes::FIELD_FORMAT, "error.settings_patch")
                        .with_detail(error.to_string()),
                )),
            }
        }
    };
    match engine().save_settings_group(&group, patch, expected_revision) {
        Ok(outcome) => SaveSettingsResult {
            ok: true,
            new_revision: Some(outcome.new_revision),
            changes: changes_to_dto(&outcome.changes),
            restart_core_fields: outcome.restart_core_fields,
            restart_app_fields: outcome.restart_app_fields,
            next_launch_fields: outcome.next_launch_fields,
            error: None,
        },
        Err(error) => SaveSettingsResult {
            ok: false,
            new_revision: None,
            changes: Vec::new(),
            restart_core_fields: Vec::new(),
            restart_app_fields: Vec::new(),
            next_launch_fields: Vec::new(),
            error: Some(error_dto(error)),
        },
    }
}

/// Content hash of an already-saved settings document for SP-12 retries.
///
/// Plain helper (not an FRB method, so no codegen change): the Dart
/// `retrySettingsApply` path carries the hash and the Rust side re-checks it
/// before running apply phases. New FRB surface, if ever needed, is owned by
/// the SP-00 integrator (see SP-12 evidence interface registry).
pub fn settings_content_hash_for_retry(settings_json: &str) -> String {
    application::settings::settings_content_hash(settings_json)
}

/// Whether a retry may run: draft and persisted hashes must equal the saved
/// hash (SP-12). Pure so it can be unit tested without the engine.
pub fn retry_content_matches_saved(
    saved_hash: &str,
    draft_hash: &str,
    persisted_hash: &str,
) -> bool {
    application::settings::retry_content_matches_saved(saved_hash, draft_hash, persisted_hash)
}

/// Test/debug helper: validate the current tree without saving it.
#[frb(sync)]
pub fn validate_current_settings() -> bool {
    engine()
        .load_settings()
        .map(|loaded| validate_settings(&loaded.settings).is_ok())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(settings: AppSettings) -> AppSettings {
        let dto = SettingsDto::from(&settings);
        AppSettings::from(dto)
    }

    #[test]
    fn dto_round_trip_preserves_tree_and_extras() {
        let mut settings = AppSettings::default();
        settings
            .gui_item
            .extra
            .insert("Future".into(), serde_json::json!(7));
        settings.core_basic_item.loglevel = None;
        settings.sub_index_id = Some(String::new());
        assert_eq!(roundtrip(settings.clone()), settings);
    }

    #[test]
    fn save_settings_group_returns_timing() {
        // Serialized with the other global-engine tests; the group revision
        // is still re-read instead of assuming zero.
        let _guard = crate::api::engine::engine_test_lock();
        let loaded = get_settings();
        assert!(loaded.ok);
        let group_rev: u64 = serde_json::from_str::<std::collections::BTreeMap<String, u64>>(
            &loaded.group_revisions_json,
        )
        .unwrap()
        .get("UiItem")
        .copied()
        .unwrap_or(0);
        let result = save_settings_group(
            "UiItem".to_string(),
            r#"{"CurrentTheme":"Dark"}"#.to_string(),
            group_rev,
        );
        assert!(
            result.ok,
            "unexpected error: {}",
            result.error.unwrap().code
        );
        assert!(result
            .changes
            .iter()
            .any(|c| c.path == "UiItem.CurrentTheme" && c.timing == "immediate"));
    }

    #[test]
    fn save_settings_group_rejects_unknown_group() {
        let _guard = crate::api::engine::engine_test_lock();
        let result = save_settings_group("NotAGroup".to_string(), "{}".to_string(), 0);
        assert!(!result.ok);
        assert_eq!(result.error.unwrap().code, domain::codes::INVALID_ARGUMENT);
    }

    #[test]
    fn retry_hash_helpers_reject_diverged_content() {
        let saved = settings_content_hash_for_retry(r#"{"a":1}"#);
        let same = settings_content_hash_for_retry(r#"{"a":1}"#);
        let other = settings_content_hash_for_retry(r#"{"a":2}"#);
        assert_eq!(saved, same);
        assert!(retry_content_matches_saved(&saved, &same, &same));
        assert!(!retry_content_matches_saved(&saved, &other, &same));
        assert!(!retry_content_matches_saved(&saved, &same, &other));
    }

    #[test]
    fn dto_round_trip_all_groups_with_extras() {
        let mut settings = AppSettings::default();
        settings
            .ui_item
            .extra
            .insert("FutureUi".to_string(), serde_json::json!(1));
        settings.system_proxy_item.sys_proxy_type = SysProxyType::Pac;
        settings.inbound[0].protocol = InboundProtocol::Mixed;
        settings.ui_item.main_gird_orientation = GirdOrientation::Horizontal;
        settings.simple_dns_item.fake_ip = Some(true);
        settings.global_hotkeys.push(GlobalHotkey {
            action: 2,
            control: true,
            key_code: Some(67),
            ..Default::default()
        });
        settings.core_type_item = Some(vec![DomainCoreTypeBinding::new(
            ConfigType::Vless,
            CoreType::SingBox,
        )]);
        settings.fragment4_ray_item.as_mut().unwrap().lengths = Some(vec!["1-1".to_string()]);

        let restored = AppSettings::from(SettingsDto::from(&settings));
        assert_eq!(restored, settings);
    }
}
