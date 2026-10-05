/// Upstream `Global.RootCertProviders`; the first entry is the fallback
/// (`Global.cs:758`). Trust anchors for the app's own downloads only.
const List<String> rootCertProviders = <String>['system', 'chrome', 'mozilla'];

/// Upstream fallback value for [rootCertProviders].
const String defaultRootCertProvider = 'system';

/// Upstream `ConfigHandler.LoadConfig`: a value outside
/// `Global.RootCertProviders` is forced to the first entry (`system`).
String normalizeRootCertProvider(Object? value) =>
    value is String && rootCertProviders.contains(value)
    ? value
    : defaultRootCertProvider;

/// Top-level settings groups that can be saved independently
/// (`domain::SETTINGS_GROUPS`).
const List<String> defaultSettingsGroups = <String>[
  'IndexId',
  'SubIndexId',
  'CoreBasicItem',
  'TunModeItem',
  'KcpItem',
  'GrpcItem',
  'RoutingBasicItem',
  'GuiItem',
  'MsgUIItem',
  'UiItem',
  'ConstItem',
  'SpeedTestItem',
  'Mux4RayItem',
  'Mux4SboxItem',
  'HysteriaItem',
  'ClashUIItem',
  'SystemProxyItem',
  'WebDavItem',
  'CheckUpdateItem',
  'Fragment4RayItem',
  'Inbound',
  'GlobalHotkeys',
  'CoreTypeItem',
  'SimpleDNSItem',
  'HappyEyeballs4RayItem',
];

/// Canonical default `guiNConfig.json` `Config` tree.
///
/// Mirrors `AppSettings::default()` in `crates/domain/src/settings.rs`
/// (upstream `ConfigHandler.LoadConfig`). Used by the synthetic bridge in
/// widget tests and as a fallback when the native bridge is unavailable. Keys
/// are PascalCase and enums are integers, exactly like the persisted document.
Map<String, dynamic> defaultSettingsJson() => <String, dynamic>{
  'IndexId': null,
  'SubIndexId': null,
  'CoreBasicItem': <String, dynamic>{
    'LogEnabled': false,
    'Loglevel': 'warning',
    'DefFingerprint': null,
    'DefUserAgent': null,
    'SendThrough': null,
    'BindInterface': null,
    'EnableFragment': false,
    'EnableFinalFragment': false,
    'EnableCacheFile4Sbox': true,
  },
  'TunModeItem': <String, dynamic>{
    'EnableTun': false,
    'AutoRoute': true,
    'StrictRoute': true,
    'Stack': null,
    'Mtu': 9000,
    'EnableIPv6Address': false,
    'IcmpRouting': 'rule',
    'EnableLegacyProtect': true,
    'RouteExcludeAddress': null,
    'IPv4Address': null,
    'IPv6Address': null,
  },
  'KcpItem': <String, dynamic>{
    'Mtu': 1350,
    'Tti': 50,
    'UplinkCapacity': 12,
    'DownlinkCapacity': 100,
    'CwndMultiplier': 1,
    'MaxSendingWindow': 2097152,
  },
  'GrpcItem': <String, dynamic>{
    'IdleTimeout': 60,
    'HealthCheckTimeout': 20,
    'PermitWithoutStream': false,
    'InitialWindowsSize': 0,
  },
  'RoutingBasicItem': <String, dynamic>{
    'DomainStrategy': 'AsIs',
    'DomainStrategy4Singbox': null,
    'RoutingIndexId': null,
  },
  'GuiItem': <String, dynamic>{
    'AutoRun': false,
    'EnableStatistics': false,
    'DisplayRealTimeSpeed': false,
    'KeepOlderDedupl': false,
    'AutoUpdateInterval': 0,
    'TrayMenuServersLimit': 20,
    'EnableHWA': false,
    'EnableLog': true,
    'RootCertProvider': 'system',
  },
  'MsgUIItem': <String, dynamic>{'MainMsgFilter': null, 'AutoRefresh': null},
  'UiItem': <String, dynamic>{
    'EnableAutoAdjustMainLvColWidth': false,
    'MainGirdHeight1': 0,
    'MainGirdHeight2': 0,
    'MainGirdOrientation': 1,
    'ColorPrimaryName': null,
    'CurrentTheme': null,
    'CurrentLanguage': 'zh-Hans',
    'CurrentFontFamily': null,
    'CurrentFontSize': 0,
    'EnableDragDropSort': false,
    'DoubleClick2Activate': false,
    'AutoHideStartup': false,
    'Hide2TrayWhenClose': false,
    'MacOSShowInDock': false,
    'MainColumnItem': <dynamic>[],
    'WindowSizeItem': <dynamic>[],
    'HideColumnIpInfo': false,
  },
  'ConstItem': <String, dynamic>{
    'SubConvertUrl': null,
    'GeoSourceUrl': null,
    'SrsSourceUrl': null,
    'RouteRulesTemplateSourceUrl': null,
  },
  'SpeedTestItem': <String, dynamic>{
    'SpeedTestTimeout': 10,
    'SpeedTestUrl': 'https://cachefly.cachefly.net/50mb.test',
    'SpeedPingTestUrl': 'https://www.google.com/generate_204',
    'MixedConcurrencyCount': 10,
    'IPAPIUrl': null,
    'UdpTestTarget': 'ntp:pool.ntp.org',
    'SpeedTestPageSize': null,
    'SpeedTestDelayInterval': null,
  },
  'Mux4RayItem': <String, dynamic>{
    'Concurrency': 8,
    'XudpConcurrency': 16,
    'XudpProxyUDP443': 'reject',
  },
  'Mux4SboxItem': <String, dynamic>{
    'Protocol': 'h2mux',
    'MaxConnections': 8,
    'Padding': null,
  },
  'HysteriaItem': <String, dynamic>{
    'UpMbps': 100,
    'DownMbps': 100,
    'HopInterval': 30,
  },
  'ClashUIItem': <String, dynamic>{
    'EnableIPv6': false,
    'EnableMixinContent': false,
    'ProxiesSorting': 0,
    'ProxiesAutoRefresh': false,
    'ProxiesRefreshInterval': 2,
    'ConnectionsAutoRefresh': false,
    'ConnectionsRefreshInterval': 2,
    'ConnectionsColumnItem': <dynamic>[],
  },
  'SystemProxyItem': <String, dynamic>{
    'SysProxyType': 0,
    'SystemProxyExceptions':
        'localhost;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.20.*;'
        '172.21.*;172.22.*;172.23.*;172.24.*;172.25.*;172.26.*;172.27.*;'
        '172.28.*;172.29.*;172.30.*;172.31.*;192.168.*',
    'NotProxyLocalAddress': true,
    'SystemProxyAdvancedProtocol': null,
    'CustomSystemProxyPacPath': null,
    'CustomSystemProxyScriptPath': null,
  },
  'WebDavItem': <String, dynamic>{
    'Url': null,
    'UserName': null,
    'Password': null,
    'DirName': null,
  },
  'CheckUpdateItem': <String, dynamic>{
    'CheckPreReleaseUpdate': false,
    'UpdateViaProxy': true,
    'SelectedCoreTypes': null,
  },
  'Fragment4RayItem': <String, dynamic>{
    'Packets': 'tlshello',
    'Lengths': <dynamic>['50-100'],
    'Delays': <dynamic>['10-20'],
    'MaxSplit': '0',
    'Length': null,
    'Interval': null,
  },
  'Inbound': <dynamic>[
    <String, dynamic>{
      'LocalPort': 10808,
      'Protocol': 0,
      'UdpEnabled': true,
      'SniffingEnabled': true,
      'DestOverride': <dynamic>['http', 'tls'],
      'RouteOnly': false,
      'AllowLANConn': false,
      'NewPort4LAN': false,
      'User': '',
      'Pass': '',
      'SecondLocalPortEnabled': false,
    },
  ],
  'GlobalHotkeys': <dynamic>[],
  'CoreTypeItem': null,
  'SimpleDNSItem': <String, dynamic>{
    'UseSystemHosts': false,
    'AddCommonHosts': true,
    'FakeIP': false,
    'GlobalFakeIp': true,
    'FakeIPRange': '198.18.0.0/15',
    'BlockBindingQuery': true,
    'BlockAAAAQuery': false,
    'DirectDNS': '119.29.29.29',
    'RemoteDNS': 'https://cloudflare-dns.com/dns-query',
    'BootstrapDNS': '119.29.29.29',
    'Strategy4Freedom': null,
    'Strategy4Proxy': null,
    'Strategy4ProxyDial': null,
    'ServeStale': false,
    'ParallelQuery': false,
    'Hosts': null,
    'DirectExpectedIPs': null,
    'EnableHappyEyeballs': false,
  },
  'HappyEyeballs4RayItem': <String, dynamic>{
    'TryDelayMs': 250,
    'PrioritizeIPv6': false,
    'Interleave': 1,
    'MaxConcurrentTry': 4,
  },
};

/// Fill a loaded settings [document] with missing/`null` defaults.
///
/// Mirrors the upstream `ConfigHandler.LoadConfig` field-default layer so a
/// partial snapshot (or an explicit `null` written by the C# serializer) still
/// renders the canonical default instead of a CLR zero/blank value. Scalar
/// fields whose canonical default is `null` (e.g. `DefFingerprint`) are left
/// untouched: `null` is meaningful there. Present non-null values always win,
/// so this never overwrites stored data. Lists/maps are merged recursively; a
/// present list is kept as-is (inbound/hotkey/core-type rows are user data).
Map<String, dynamic> mergeWithSettingsDefaults(Map<String, dynamic> loaded) {
  final defaults = defaultSettingsJson();
  final merged = _deepCopyMap(loaded);
  // Upstream `ConfigHandler.LoadConfig` migrates the pre-7.x fragment scalars
  // into the list fields before the generic defaults layer runs, so an empty
  // list still reflects the user's stored `Length`/`Interval`.
  _promoteFragmentLegacy(merged);
  for (final entry in defaults.entries) {
    final key = entry.key;
    final fallback = entry.value;
    final current = merged[key];
    if (fallback is Map && fallback.isNotEmpty) {
      final base = current is Map<String, dynamic>
          ? current
          : current is Map
          ? Map<String, dynamic>.from(current)
          : <String, dynamic>{};
      merged[key] = _mergeMaps(base, Map<String, dynamic>.from(fallback));
      continue;
    }
    // A null/missing scalar falls back only when the canonical default is a
    // concrete non-null value; a null default stays null.
    if ((current == null) && fallback != null) {
      merged[key] = _deepCopyValue(fallback);
    }
  }
  return merged;
}

/// Upstream `ConfigHandler.LoadConfig:181-187`: when `Fragment4RayItem.Lengths`
/// (or `Delays`) is empty, it is seeded from the legacy `Length` (or
/// `Interval`) scalar, falling back to the frozen `50-100` / `10-20`. Applied
/// before the generic defaults merge so a stored legacy value is not shadowed
/// by the canonical list default.
void _promoteFragmentLegacy(Map<String, dynamic> document) {
  final fragment = document['Fragment4RayItem'];
  if (fragment is! Map) return;
  final map = fragment is Map<String, dynamic>
      ? fragment
      : Map<String, dynamic>.from(fragment);
  final lengths = map['Lengths'];
  if (lengths is! List || lengths.isEmpty) {
    final legacy = map['Length'];
    map['Lengths'] = <dynamic>[
      if (legacy is String && legacy.trim().isNotEmpty) legacy else '50-100',
    ];
  }
  final delays = map['Delays'];
  if (delays is! List || delays.isEmpty) {
    final legacy = map['Interval'];
    map['Delays'] = <dynamic>[
      if (legacy is String && legacy.trim().isNotEmpty) legacy else '10-20',
    ];
  }
  if (!identical(map, fragment)) document['Fragment4RayItem'] = map;
}

Map<String, dynamic> _mergeMaps(
  Map<String, dynamic> base,
  Map<String, dynamic> defaults,
) {
  final out = _deepCopyMap(base);
  for (final entry in defaults.entries) {
    final current = out[entry.key];
    final fallback = entry.value;
    if (fallback is Map && fallback.isNotEmpty) {
      final nested = current is Map<String, dynamic>
          ? current
          : current is Map
          ? Map<String, dynamic>.from(current)
          : <String, dynamic>{};
      out[entry.key] = _mergeMaps(nested, Map<String, dynamic>.from(fallback));
      continue;
    }
    if (current == null && fallback != null) {
      out[entry.key] = _deepCopyValue(fallback);
    }
  }
  return out;
}

Map<String, dynamic> _deepCopyMap(Map<String, dynamic> source) =>
    source.map((k, v) => MapEntry(k, _deepCopyValue(v)));

Object? _deepCopyValue(Object? value) {
  if (value is Map) {
    return value.map((k, v) => MapEntry(k.toString(), _deepCopyValue(v)));
  }
  if (value is List) return value.map(_deepCopyValue).toList();
  return value;
}
