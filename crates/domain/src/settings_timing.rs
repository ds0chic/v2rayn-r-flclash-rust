//! Auto-generated from `compat/fields.settings.yaml` (180 entries) for T12a.
//! Do not edit by hand: regenerate from the ledger.
//!
//! `apply_timing` is the upstream change-propagation class of every persisted
//! settings field: `immediate` (UI applies at once), `save` (storage only),
//! `restart_core` (kernel reload), `restart_app` (application restart) and
//! `next_launch` (read on next start).

use crate::settings::ApplyTiming;

/// `(group_json_key, field, timing)` for every ledger entry. Whole-group
/// entries use the group key as `field` under the `Config` group.
pub const FIELD_TIMING: &[(&str, &str, ApplyTiming)] = &[
    (
        "CheckUpdateItem",
        "CheckPreReleaseUpdate",
        ApplyTiming::Save,
    ), // FLD-CFG-147
    ("CheckUpdateItem", "SelectedCoreTypes", ApplyTiming::Save), // FLD-CFG-149
    ("CheckUpdateItem", "UpdateViaProxy", ApplyTiming::Save),    // FLD-CFG-148
    (
        "ClashUIItem",
        "ConnectionsAutoRefresh",
        ApplyTiming::Immediate,
    ), // FLD-CFG-134
    (
        "ClashUIItem",
        "ConnectionsColumnItem",
        ApplyTiming::Immediate,
    ), // FLD-CFG-136
    (
        "ClashUIItem",
        "ConnectionsRefreshInterval",
        ApplyTiming::Immediate,
    ), // FLD-CFG-135
    ("ClashUIItem", "EnableIPv6", ApplyTiming::RestartCore),     // FLD-CFG-129
    (
        "ClashUIItem",
        "EnableMixinContent",
        ApplyTiming::RestartCore,
    ), // FLD-CFG-130
    ("ClashUIItem", "ProxiesAutoRefresh", ApplyTiming::Immediate), // FLD-CFG-132
    (
        "ClashUIItem",
        "ProxiesRefreshInterval",
        ApplyTiming::Immediate,
    ), // FLD-CFG-133
    ("ClashUIItem", "ProxiesSorting", ApplyTiming::Immediate),   // FLD-CFG-131
    ("ColumnItem", "Index", ApplyTiming::Save),                  // FLD-CFG-119
    ("ColumnItem", "Name", ApplyTiming::Save),                   // FLD-CFG-117
    ("ColumnItem", "Width", ApplyTiming::Save),                  // FLD-CFG-118
    ("Config", "CheckUpdateItem", ApplyTiming::Save),            // FLD-CFG-019
    ("Config", "ClashUIItem", ApplyTiming::Save),                // FLD-CFG-016
    ("Config", "ConstItem", ApplyTiming::Save),                  // FLD-CFG-011
    ("Config", "CoreBasicItem", ApplyTiming::Save),              // FLD-CFG-003
    ("Config", "CoreTypeItem", ApplyTiming::Save),               // FLD-CFG-023
    ("Config", "Fragment4RayItem", ApplyTiming::Save),           // FLD-CFG-020
    ("Config", "GlobalHotkeys", ApplyTiming::Save),              // FLD-CFG-022
    ("Config", "GrpcItem", ApplyTiming::Save),                   // FLD-CFG-006
    ("Config", "GuiItem", ApplyTiming::Save),                    // FLD-CFG-008
    ("Config", "HappyEyeballs4RayItem", ApplyTiming::Save),      // FLD-CFG-025
    ("Config", "HysteriaItem", ApplyTiming::Save),               // FLD-CFG-015
    ("Config", "Inbound", ApplyTiming::Save),                    // FLD-CFG-021
    ("Config", "IndexId", ApplyTiming::Save),                    // FLD-CFG-001
    ("Config", "KcpItem", ApplyTiming::Save),                    // FLD-CFG-005
    ("Config", "MsgUIItem", ApplyTiming::Save),                  // FLD-CFG-009
    ("Config", "Mux4RayItem", ApplyTiming::Save),                // FLD-CFG-013
    ("Config", "Mux4SboxItem", ApplyTiming::Save),               // FLD-CFG-014
    ("Config", "RoutingBasicItem", ApplyTiming::Save),           // FLD-CFG-007
    ("Config", "SimpleDNSItem", ApplyTiming::Save),              // FLD-CFG-024
    ("Config", "SpeedTestItem", ApplyTiming::Save),              // FLD-CFG-012
    ("Config", "SubIndexId", ApplyTiming::Save),                 // FLD-CFG-002
    ("Config", "SystemProxyItem", ApplyTiming::Save),            // FLD-CFG-017
    ("Config", "TunModeItem", ApplyTiming::Save),                // FLD-CFG-004
    ("Config", "UiItem", ApplyTiming::Save),                     // FLD-CFG-010
    ("Config", "WebDavItem", ApplyTiming::Save),                 // FLD-CFG-018
    ("ConstItem", "GeoSourceUrl", ApplyTiming::Save),            // FLD-CFG-085
    (
        "ConstItem",
        "RouteRulesTemplateSourceUrl",
        ApplyTiming::Save,
    ), // FLD-CFG-087
    ("ConstItem", "SrsSourceUrl", ApplyTiming::Save),            // FLD-CFG-086
    ("ConstItem", "SubConvertUrl", ApplyTiming::Save),           // FLD-CFG-084
    ("CoreBasicItem", "BindInterface", ApplyTiming::RestartCore), // FLD-CFG-031
    ("CoreBasicItem", "DefFingerprint", ApplyTiming::RestartCore), // FLD-CFG-028
    ("CoreBasicItem", "DefUserAgent", ApplyTiming::RestartCore), // FLD-CFG-029
    (
        "CoreBasicItem",
        "EnableCacheFile4Sbox",
        ApplyTiming::RestartCore,
    ), // FLD-CFG-034
    (
        "CoreBasicItem",
        "EnableFinalFragment",
        ApplyTiming::RestartCore,
    ), // FLD-CFG-033
    ("CoreBasicItem", "EnableFragment", ApplyTiming::RestartCore), // FLD-CFG-032
    ("CoreBasicItem", "LogEnabled", ApplyTiming::RestartCore),   // FLD-CFG-026
    ("CoreBasicItem", "Loglevel", ApplyTiming::RestartCore),     // FLD-CFG-027
    ("CoreBasicItem", "SendThrough", ApplyTiming::RestartCore),  // FLD-CFG-030
    ("CoreTypeItem", "ConfigType", ApplyTiming::Save),           // FLD-CFG-093
    ("CoreTypeItem", "CoreType", ApplyTiming::RestartCore),      // FLD-CFG-094
    ("Fragment4RayItem", "Delays", ApplyTiming::RestartCore),    // FLD-CFG-152
    ("Fragment4RayItem", "Interval", ApplyTiming::NextLaunch),   // FLD-CFG-155
    ("Fragment4RayItem", "Length", ApplyTiming::NextLaunch),     // FLD-CFG-154
    ("Fragment4RayItem", "Lengths", ApplyTiming::RestartCore),   // FLD-CFG-151
    ("Fragment4RayItem", "MaxSplit", ApplyTiming::RestartCore),  // FLD-CFG-153
    ("Fragment4RayItem", "Packets", ApplyTiming::RestartCore),   // FLD-CFG-150
    ("GlobalHotkeys", "Alt", ApplyTiming::NextLaunch),           // FLD-CFG-089
    ("GlobalHotkeys", "Control", ApplyTiming::NextLaunch),       // FLD-CFG-090
    ("GlobalHotkeys", "EGlobalHotkey", ApplyTiming::NextLaunch), // FLD-CFG-088
    ("GlobalHotkeys", "KeyCode", ApplyTiming::NextLaunch),       // FLD-CFG-092
    ("GlobalHotkeys", "Shift", ApplyTiming::NextLaunch),         // FLD-CFG-091
    ("GrpcItem", "HealthCheckTimeout", ApplyTiming::RestartCore), // FLD-CFG-053
    ("GrpcItem", "IdleTimeout", ApplyTiming::RestartCore),       // FLD-CFG-052
    ("GrpcItem", "InitialWindowsSize", ApplyTiming::RestartCore), // FLD-CFG-055
    ("GrpcItem", "PermitWithoutStream", ApplyTiming::RestartCore), // FLD-CFG-054
    ("GuiItem", "AutoRun", ApplyTiming::Immediate),              // FLD-CFG-056
    ("GuiItem", "AutoUpdateInterval", ApplyTiming::Immediate),   // FLD-CFG-060
    ("GuiItem", "DisplayRealTimeSpeed", ApplyTiming::RestartApp), // FLD-CFG-058
    ("GuiItem", "EnableHWA", ApplyTiming::RestartApp),           // FLD-CFG-062
    ("GuiItem", "EnableLog", ApplyTiming::RestartApp),           // FLD-CFG-063
    ("GuiItem", "EnableStatistics", ApplyTiming::RestartApp),    // FLD-CFG-057
    ("GuiItem", "KeepOlderDedupl", ApplyTiming::Save),           // FLD-CFG-059
    ("GuiItem", "RootCertProvider", ApplyTiming::Immediate),     // FLD-CFG-064
    ("GuiItem", "TrayMenuServersLimit", ApplyTiming::Save),      // FLD-CFG-061
    (
        "HappyEyeballs4RayItem",
        "Interleave",
        ApplyTiming::RestartCore,
    ), // FLD-CFG-179
    (
        "HappyEyeballs4RayItem",
        "MaxConcurrentTry",
        ApplyTiming::RestartCore,
    ), // FLD-CFG-180
    (
        "HappyEyeballs4RayItem",
        "PrioritizeIPv6",
        ApplyTiming::RestartCore,
    ), // FLD-CFG-178
    (
        "HappyEyeballs4RayItem",
        "TryDelayMs",
        ApplyTiming::RestartCore,
    ), // FLD-CFG-177
    ("HysteriaItem", "DownMbps", ApplyTiming::RestartCore),      // FLD-CFG-127
    ("HysteriaItem", "HopInterval", ApplyTiming::RestartCore),   // FLD-CFG-128
    ("HysteriaItem", "UpMbps", ApplyTiming::RestartCore),        // FLD-CFG-126
    ("Inbound", "AllowLANConn", ApplyTiming::RestartCore),       // FLD-CFG-041
    ("Inbound", "DestOverride", ApplyTiming::RestartCore),       // FLD-CFG-039
    ("Inbound", "LocalPort", ApplyTiming::RestartCore),          // FLD-CFG-035
    ("Inbound", "NewPort4LAN", ApplyTiming::RestartCore),        // FLD-CFG-042
    ("Inbound", "Pass", ApplyTiming::RestartCore),               // FLD-CFG-044
    ("Inbound", "Protocol", ApplyTiming::RestartCore),           // FLD-CFG-036
    ("Inbound", "RouteOnly", ApplyTiming::RestartCore),          // FLD-CFG-040
    (
        "Inbound",
        "SecondLocalPortEnabled",
        ApplyTiming::RestartCore,
    ), // FLD-CFG-045
    ("Inbound", "SniffingEnabled", ApplyTiming::RestartCore),    // FLD-CFG-038
    ("Inbound", "UdpEnabled", ApplyTiming::RestartCore),         // FLD-CFG-037
    ("Inbound", "User", ApplyTiming::RestartCore),               // FLD-CFG-043
    ("KcpItem", "CwndMultiplier", ApplyTiming::RestartCore),     // FLD-CFG-050
    ("KcpItem", "DownlinkCapacity", ApplyTiming::RestartCore),   // FLD-CFG-049
    ("KcpItem", "MaxSendingWindow", ApplyTiming::RestartCore),   // FLD-CFG-051
    ("KcpItem", "Mtu", ApplyTiming::RestartCore),                // FLD-CFG-046
    ("KcpItem", "Tti", ApplyTiming::RestartCore),                // FLD-CFG-047
    ("KcpItem", "UplinkCapacity", ApplyTiming::RestartCore),     // FLD-CFG-048
    ("MsgUIItem", "AutoRefresh", ApplyTiming::Immediate),        // FLD-CFG-066
    ("MsgUIItem", "MainMsgFilter", ApplyTiming::Immediate),      // FLD-CFG-065
    ("Mux4RayItem", "Concurrency", ApplyTiming::RestartCore),    // FLD-CFG-120
    ("Mux4RayItem", "XudpConcurrency", ApplyTiming::RestartCore), // FLD-CFG-121
    ("Mux4RayItem", "XudpProxyUDP443", ApplyTiming::RestartCore), // FLD-CFG-122
    ("Mux4SboxItem", "MaxConnections", ApplyTiming::RestartCore), // FLD-CFG-124
    ("Mux4SboxItem", "Padding", ApplyTiming::RestartCore),       // FLD-CFG-125
    ("Mux4SboxItem", "Protocol", ApplyTiming::RestartCore),      // FLD-CFG-123
    (
        "RoutingBasicItem",
        "DomainStrategy",
        ApplyTiming::RestartCore,
    ), // FLD-CFG-114
    (
        "RoutingBasicItem",
        "DomainStrategy4Singbox",
        ApplyTiming::RestartCore,
    ), // FLD-CFG-115
    ("RoutingBasicItem", "RoutingIndexId", ApplyTiming::Save),   // FLD-CFG-116
    ("SimpleDNSItem", "AddCommonHosts", ApplyTiming::RestartCore), // FLD-CFG-160
    ("SimpleDNSItem", "BlockAAAAQuery", ApplyTiming::RestartCore), // FLD-CFG-165
    (
        "SimpleDNSItem",
        "BlockBindingQuery",
        ApplyTiming::RestartCore,
    ), // FLD-CFG-164
    ("SimpleDNSItem", "BootstrapDNS", ApplyTiming::RestartCore), // FLD-CFG-168
    ("SimpleDNSItem", "DirectDNS", ApplyTiming::RestartCore),    // FLD-CFG-166
    (
        "SimpleDNSItem",
        "DirectExpectedIPs",
        ApplyTiming::RestartCore,
    ), // FLD-CFG-175
    (
        "SimpleDNSItem",
        "EnableHappyEyeballs",
        ApplyTiming::RestartCore,
    ), // FLD-CFG-176
    ("SimpleDNSItem", "FakeIP", ApplyTiming::RestartCore),       // FLD-CFG-161
    ("SimpleDNSItem", "FakeIPRange", ApplyTiming::RestartCore),  // FLD-CFG-163
    ("SimpleDNSItem", "GlobalFakeIp", ApplyTiming::RestartCore), // FLD-CFG-162
    ("SimpleDNSItem", "Hosts", ApplyTiming::RestartCore),        // FLD-CFG-174
    ("SimpleDNSItem", "ParallelQuery", ApplyTiming::RestartCore), // FLD-CFG-173
    ("SimpleDNSItem", "RemoteDNS", ApplyTiming::RestartCore),    // FLD-CFG-167
    ("SimpleDNSItem", "ServeStale", ApplyTiming::RestartCore),   // FLD-CFG-172
    (
        "SimpleDNSItem",
        "Strategy4Freedom",
        ApplyTiming::RestartCore,
    ), // FLD-CFG-169
    ("SimpleDNSItem", "Strategy4Proxy", ApplyTiming::RestartCore), // FLD-CFG-170
    (
        "SimpleDNSItem",
        "Strategy4ProxyDial",
        ApplyTiming::RestartCore,
    ), // FLD-CFG-171
    ("SimpleDNSItem", "UseSystemHosts", ApplyTiming::RestartCore), // FLD-CFG-159
    ("SpeedTestItem", "IPAPIUrl", ApplyTiming::Immediate),       // FLD-CFG-110
    (
        "SpeedTestItem",
        "MixedConcurrencyCount",
        ApplyTiming::Immediate,
    ), // FLD-CFG-109
    ("SpeedTestItem", "SpeedPingTestUrl", ApplyTiming::Immediate), // FLD-CFG-108
    (
        "SpeedTestItem",
        "SpeedTestDelayInterval",
        ApplyTiming::Immediate,
    ), // FLD-CFG-113
    ("SpeedTestItem", "SpeedTestPageSize", ApplyTiming::Immediate), // FLD-CFG-112
    ("SpeedTestItem", "SpeedTestTimeout", ApplyTiming::Immediate), // FLD-CFG-106
    ("SpeedTestItem", "SpeedTestUrl", ApplyTiming::Immediate),   // FLD-CFG-107
    ("SpeedTestItem", "UdpTestTarget", ApplyTiming::Immediate),  // FLD-CFG-111
    (
        "SystemProxyItem",
        "CustomSystemProxyPacPath",
        ApplyTiming::Immediate,
    ), // FLD-CFG-141
    (
        "SystemProxyItem",
        "CustomSystemProxyScriptPath",
        ApplyTiming::Immediate,
    ), // FLD-CFG-142
    (
        "SystemProxyItem",
        "NotProxyLocalAddress",
        ApplyTiming::Immediate,
    ), // FLD-CFG-139
    ("SystemProxyItem", "SysProxyType", ApplyTiming::Immediate), // FLD-CFG-137
    (
        "SystemProxyItem",
        "SystemProxyAdvancedProtocol",
        ApplyTiming::Immediate,
    ), // FLD-CFG-140
    (
        "SystemProxyItem",
        "SystemProxyExceptions",
        ApplyTiming::Immediate,
    ), // FLD-CFG-138
    ("TunModeItem", "AutoRoute", ApplyTiming::RestartCore),      // FLD-CFG-096
    ("TunModeItem", "EnableIPv6Address", ApplyTiming::RestartCore), // FLD-CFG-100
    (
        "TunModeItem",
        "EnableLegacyProtect",
        ApplyTiming::RestartCore,
    ), // FLD-CFG-102
    ("TunModeItem", "EnableTun", ApplyTiming::RestartCore),      // FLD-CFG-095
    ("TunModeItem", "IPv4Address", ApplyTiming::RestartCore),    // FLD-CFG-104
    ("TunModeItem", "IPv6Address", ApplyTiming::RestartCore),    // FLD-CFG-105
    ("TunModeItem", "IcmpRouting", ApplyTiming::RestartCore),    // FLD-CFG-101
    ("TunModeItem", "Mtu", ApplyTiming::RestartCore),            // FLD-CFG-099
    (
        "TunModeItem",
        "RouteExcludeAddress",
        ApplyTiming::RestartCore,
    ), // FLD-CFG-103
    ("TunModeItem", "Stack", ApplyTiming::RestartCore),          // FLD-CFG-098
    ("TunModeItem", "StrictRoute", ApplyTiming::RestartCore),    // FLD-CFG-097
    ("UiItem", "AutoHideStartup", ApplyTiming::Immediate),       // FLD-CFG-078
    ("UiItem", "ColorPrimaryName", ApplyTiming::Immediate),      // FLD-CFG-071
    ("UiItem", "CurrentFontFamily", ApplyTiming::RestartApp),    // FLD-CFG-074
    ("UiItem", "CurrentFontSize", ApplyTiming::Immediate),       // FLD-CFG-075
    ("UiItem", "CurrentLanguage", ApplyTiming::RestartApp),      // FLD-CFG-073
    ("UiItem", "CurrentTheme", ApplyTiming::Immediate),          // FLD-CFG-072
    ("UiItem", "DoubleClick2Activate", ApplyTiming::Immediate),  // FLD-CFG-077
    (
        "UiItem",
        "EnableAutoAdjustMainLvColWidth",
        ApplyTiming::Immediate,
    ), // FLD-CFG-067
    ("UiItem", "EnableDragDropSort", ApplyTiming::RestartApp),   // FLD-CFG-076
    ("UiItem", "Hide2TrayWhenClose", ApplyTiming::Immediate),    // FLD-CFG-079
    ("UiItem", "HideColumnIpInfo", ApplyTiming::Immediate),      // FLD-CFG-083
    ("UiItem", "MacOSShowInDock", ApplyTiming::RestartApp),      // FLD-CFG-080
    ("UiItem", "MainColumnItem", ApplyTiming::Immediate),        // FLD-CFG-081
    ("UiItem", "MainGirdHeight1", ApplyTiming::Immediate),       // FLD-CFG-068
    ("UiItem", "MainGirdHeight2", ApplyTiming::Immediate),       // FLD-CFG-069
    ("UiItem", "MainGirdOrientation", ApplyTiming::RestartApp),  // FLD-CFG-070
    ("UiItem", "WindowSizeItem", ApplyTiming::Immediate),        // FLD-CFG-082
    ("WebDavItem", "DirName", ApplyTiming::Save),                // FLD-CFG-146
    ("WebDavItem", "Password", ApplyTiming::Save),               // FLD-CFG-145
    ("WebDavItem", "Url", ApplyTiming::Save),                    // FLD-CFG-143
    ("WebDavItem", "UserName", ApplyTiming::Save),               // FLD-CFG-144
    ("WindowSizeItem", "Height", ApplyTiming::Immediate),        // FLD-CFG-158
    ("WindowSizeItem", "TypeName", ApplyTiming::Immediate),      // FLD-CFG-156
    ("WindowSizeItem", "Width", ApplyTiming::Immediate),         // FLD-CFG-157
];

fn lookup(group: &str, field: &str) -> Option<ApplyTiming> {
    FIELD_TIMING
        .iter()
        .find(|(g, f, _)| *g == group && *f == field)
        .map(|(_, _, t)| *t)
}

fn severity(timing: ApplyTiming) -> u8 {
    match timing {
        ApplyTiming::Save => 0,
        ApplyTiming::Immediate => 1,
        ApplyTiming::NextLaunch => 2,
        ApplyTiming::RestartCore => 3,
        ApplyTiming::RestartApp => 4,
    }
}

/// Timing for a whole group being added, removed or replaced: the strongest
/// timing among the group's ledger fields (an added object changes all of them).
pub fn classify_group(group: &str) -> ApplyTiming {
    let mut best = ApplyTiming::Save;
    for (g, _, t) in FIELD_TIMING {
        if *g == group && severity(*t) > severity(best) {
            best = *t;
        }
    }
    best
}

/// Timing for one changed field inside a group. Falls back to the group-level
/// aggregate, then to `save`.
pub fn classify_leaf(group: &str, field: &str) -> ApplyTiming {
    if let Some(t) = lookup(group, field) {
        return t;
    }
    classify_group(group)
}
