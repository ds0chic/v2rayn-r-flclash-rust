import '../../shared/l10n/l10n.dart';

/// Data model for the top menu. Each backend entry carries the `ACT-*` ledger
/// id from `compat/actions.yaml`; UI-only entries carry a synthetic `UI-*` id.
/// The wire-up in [MainShell] decides what actually runs.
///
/// R4-30: labels are no longer baked in. Every entry carries a frozen upstream
/// ResUI resource [key]; [AppMenuEntry.labelFor] resolves the text for the
/// active locale (see `lib/shared/l10n/strings.g.dart`). `label` keeps the
/// frozen Chinese source text so existing tests and `ValueKey('menu-<label>')`
/// keys remain stable and reviewable against upstream.
///
/// T17: an entry that exists upstream but has no backend yet is marked
/// [preservedOnly]. It stays in place (layout/text unchanged) but renders
/// disabled with the tooltip 保留原版入口（未实现） — it must never be
/// clickable and pretend to succeed.
class AppMenuEntry {
  const AppMenuEntry({
    required this.label,
    required this.l10nKey,
    this.actionId,
    this.shortcut,
    this.enabled = true,
    this.preservedOnly = false,
    this.separatorAfter = false,
    this.submenu = const <AppMenuEntry>[],
  });

  /// Frozen upstream `ResUI.zh-Hans` menu text. Used for stable widget keys and
  /// as the last-resort fallback; the visible text is [labelFor].
  final String label;

  /// Frozen upstream `ResUI` resource key. `null` for synthetic separators.
  final String? l10nKey;

  final String? actionId;
  final String? shortcut;
  final bool enabled;

  /// Entry kept for upstream parity but not implemented; rendered disabled.
  final bool preservedOnly;

  /// Upstream `MainWindow.xaml` draws a `<Separator>` after this entry within
  /// its parent submenu (e.g. 配置项 after 扫描图片中的二维码 / 添加自定义出站 /
  /// 添加 [HTTP]). The shell renders it (R3-WPF-Main-Chrome M8).
  final bool separatorAfter;
  final List<AppMenuEntry> submenu;

  bool get isSubmenu => submenu.isNotEmpty;

  /// Whether the entry can actually be invoked (not disabled, has an action).
  bool get isInvocable =>
      enabled && !preservedOnly && (actionId != null || isSubmenu);

  /// Resolve the visible label for [l10n]. Falls back to the source [label]
  /// when the entry has no resource key.
  String labelFor(L10n l10n) {
    final key = l10nKey;
    if (key == null) return label;
    return l10n.t(key);
  }

  /// Tooltip key shown for preserved-only entries (upstream never localizes the
  /// internal "not implemented" marker; kept Chinese, matching the frozen tool).
  static const String preservedTooltip = '保留原版入口（未实现）';
}

/// ACT-MAIN-016 / 017 / 018 and 001..015 (menuServers, 18 entries).
const _serverEntries = <AppMenuEntry>[
  AppMenuEntry(
    label: '从剪贴板导入分享链接',
    l10nKey: 'menuAddServerViaClipboard',
    actionId: 'ACT-MAIN-016',
    shortcut: 'Ctrl+V',
  ),
  AppMenuEntry(
    label: '扫描屏幕上的二维码',
    l10nKey: 'menuAddServerViaScan',
    actionId: 'ACT-MAIN-017',
    shortcut: 'Ctrl+S',
  ),
  AppMenuEntry(
    label: '扫描图片中的二维码',
    l10nKey: 'menuAddServerViaImage',
    actionId: 'ACT-MAIN-018',
    separatorAfter: true,
  ),
  AppMenuEntry(
    label: '添加自定义配置',
    l10nKey: 'menuAddCustomServer',
    actionId: 'ACT-MAIN-012',
  ),
  AppMenuEntry(
    label: '添加策略组',
    l10nKey: 'menuAddPolicyGroupServer',
    actionId: 'ACT-MAIN-014',
  ),
  AppMenuEntry(
    label: '添加链式代理',
    l10nKey: 'menuAddProxyChainServer',
    actionId: 'ACT-MAIN-015',
  ),
  AppMenuEntry(
    label: '添加自定义出站',
    l10nKey: 'menuAddCustomOutboundServer',
    actionId: 'ACT-MAIN-013',
    separatorAfter: true,
  ),
  AppMenuEntry(
    label: '添加 [VMess]',
    l10nKey: 'menuAddVmessServer',
    actionId: 'ACT-MAIN-001',
  ),
  AppMenuEntry(
    label: '添加 [VLESS]',
    l10nKey: 'menuAddVlessServer',
    actionId: 'ACT-MAIN-002',
  ),
  AppMenuEntry(
    label: '添加 [Shadowsocks]',
    l10nKey: 'menuAddShadowsocksServer',
    actionId: 'ACT-MAIN-003',
  ),
  AppMenuEntry(
    label: '添加 [Trojan]',
    l10nKey: 'menuAddTrojanServer',
    actionId: 'ACT-MAIN-006',
  ),
  AppMenuEntry(
    label: '添加 [Hysteria2]',
    l10nKey: 'menuAddHysteria2Server',
    actionId: 'ACT-MAIN-007',
  ),
  AppMenuEntry(
    label: '添加 [WireGuard]',
    l10nKey: 'menuAddWireguardServer',
    actionId: 'ACT-MAIN-009',
  ),
  AppMenuEntry(
    label: '添加 [SOCKS]',
    l10nKey: 'menuAddSocksServer',
    actionId: 'ACT-MAIN-004',
  ),
  AppMenuEntry(
    label: '添加 [HTTP]',
    l10nKey: 'menuAddHttpServer',
    actionId: 'ACT-MAIN-005',
    separatorAfter: true,
  ),
  AppMenuEntry(
    label: '添加 [TUIC]',
    l10nKey: 'menuAddTuicServer',
    actionId: 'ACT-MAIN-008',
  ),
  AppMenuEntry(
    label: '添加 [Anytls]',
    l10nKey: 'menuAddAnytlsServer',
    actionId: 'ACT-MAIN-010',
  ),
  AppMenuEntry(
    label: '添加 [NaïveProxy]',
    l10nKey: 'menuAddNaiveServer',
    actionId: 'ACT-MAIN-011',
  ),
];

/// ACT-MAIN-019 (sub settings) and ACT-* subscription update entries.
const _subscriptionEntries = <AppMenuEntry>[
  AppMenuEntry(
    label: '订阅分组设置',
    l10nKey: 'menuSubSetting',
    actionId: 'ACT-MAIN-019',
    separatorAfter: true,
  ),
  AppMenuEntry(
    label: '更新全部订阅 (不通过代理)',
    l10nKey: 'menuSubUpdate',
    actionId: 'ACT-MAIN-020',
  ),
  AppMenuEntry(
    label: '更新全部订阅 (通过代理)',
    l10nKey: 'menuSubUpdateViaProxy',
    actionId: 'ACT-MAIN-021',
  ),
  AppMenuEntry(
    label: '更新当前订阅 (不通过代理)',
    l10nKey: 'menuSubGroupUpdate',
    actionId: 'ACT-MAIN-022',
  ),
  AppMenuEntry(
    label: '更新当前订阅 (通过代理)',
    l10nKey: 'menuSubGroupUpdateViaProxy',
    actionId: 'ACT-MAIN-023',
  ),
];

/// UI-only entries (no backend yet) exposed under 设置/主界面. They really take
/// effect in the UI layer; no ACT-* id exists upstream for a direct menu entry.
const _uiEntries = <AppMenuEntry>[
  AppMenuEntry(
    label: '水平布局',
    l10nKey: 'uiLayoutHorizontal',
    actionId: 'UI-LAYOUT-H',
  ),
  AppMenuEntry(
    label: '垂直布局',
    l10nKey: 'uiLayoutVertical',
    actionId: 'UI-LAYOUT-V',
  ),
  AppMenuEntry(label: '标签布局', l10nKey: 'uiLayoutTab', actionId: 'UI-LAYOUT-T'),
  AppMenuEntry(label: '显示列设置', l10nKey: 'uiColumns', actionId: 'UI-COLUMNS'),
  AppMenuEntry(
    label: '切换双击激活',
    l10nKey: 'uiDoubleClick',
    actionId: 'UI-DBLCLICK',
  ),
  AppMenuEntry(label: '切换浅色/深色', l10nKey: 'uiTheme', actionId: 'UI-THEME'),
  AppMenuEntry(label: '表格斑马纹', l10nKey: 'uiZebra', actionId: 'UI-ZEBRA'),
];

const _settingEntries = <AppMenuEntry>[
  AppMenuEntry(
    label: '参数设置',
    l10nKey: 'menuOptionSetting',
    actionId: 'ACT-MAIN-024',
  ),
  AppMenuEntry(
    label: '路由设置',
    l10nKey: 'menuRoutingSetting',
    actionId: 'ACT-MAIN-025',
  ),
  AppMenuEntry(
    label: 'DNS 设置',
    l10nKey: 'menuDNSSetting',
    actionId: 'ACT-MAIN-026',
  ),
  AppMenuEntry(
    label: '完整配置模板设置',
    l10nKey: 'menuFullConfigTemplate',
    actionId: 'ACT-MAIN-027',
  ),
  AppMenuEntry(
    label: '全局热键设置',
    l10nKey: 'menuGlobalHotkeySetting',
    actionId: 'ACT-MAIN-028',
    separatorAfter: true,
  ),
  AppMenuEntry(
    label: '主题设置',
    l10nKey: 'uiThemeWindow',
    actionId: 'UI-THEME-WINDOW',
  ),
  // ACT-MAIN-029: relaunch the current exe elevated with the runner's
  // `rebootas` marker (upstream `ProcUtils.RebootAsAdmin`). Wired in MainShell.
  AppMenuEntry(
    label: '以管理员身份重启',
    l10nKey: 'menuRebootAsAdmin',
    actionId: 'ACT-MAIN-029',
  ),
  // ACT-WIN-004: UWP loopback exemption entry. MainShell resolves the bundled
  // `EnableLoopback.exe` / builds the reversible `CheckNetIsolation` command.
  AppMenuEntry(
    label: '解除 Win10 UWP 应用回环代理限制',
    l10nKey: 'uiUwpLoopback',
    actionId: 'ACT-WIN-004',
  ),
  AppMenuEntry(
    label: '清除所有服务统计数据',
    l10nKey: 'menuClearServerStatistics',
    actionId: 'ACT-MAIN-030',
    separatorAfter: true,
  ),
  AppMenuEntry(
    label: '区域预置设置',
    l10nKey: 'menuRegionalPresets',
    actionId: 'ACT-MAIN-032',
    submenu: <AppMenuEntry>[
      AppMenuEntry(
        label: '默认区域',
        l10nKey: 'menuRegionalPresetsDefault',
        actionId: 'ACT-MAIN-032',
      ),
      AppMenuEntry(
        label: '俄罗斯',
        l10nKey: 'menuRegionalPresetsRussia',
        actionId: 'ACT-MAIN-033',
      ),
      AppMenuEntry(
        label: '伊朗',
        l10nKey: 'menuRegionalPresetsIran',
        actionId: 'ACT-MAIN-034',
      ),
    ],
  ),
  AppMenuEntry(
    label: '备份和还原',
    l10nKey: 'menuBackupAndRestore',
    actionId: 'ACT-WIN-007',
  ),
  AppMenuEntry(
    label: '打开存储所在的位置',
    l10nKey: 'menuOpenTheFileLocation',
    actionId: 'ACT-MAIN-031',
  ),
  AppMenuEntry(
    label: '清理日志与临时文件',
    l10nKey: 'uiCleanup',
    actionId: 'UI-CLEANUP',
  ),
  AppMenuEntry(
    label: '主界面 (占位)',
    l10nKey: 'uiMainPlaceholder',
    actionId: 'UI-MAIN',
    submenu: _uiEntries,
  ),
];

/// 帮助 submenu. Upstream only has 检查更新 plus a separator that is filled at
/// runtime with one `<core> 网站` entry per installed core (ACT-WIN-008). The
/// single entry opens the primary core's upstream home page through the OS
/// handler.
const _helpEntries = <AppMenuEntry>[
  AppMenuEntry(
    label: '检查更新',
    l10nKey: 'menuCheckUpdate',
    actionId: 'ACT-WIN-005',
    separatorAfter: true,
  ),
  AppMenuEntry(
    label: '核心网站',
    l10nKey: 'menuWebsiteItem',
    actionId: 'ACT-WIN-008',
  ),
];

/// Top-level menu groups in upstream order: 配置项 / 订阅分组 / 设置 / 帮助 /
/// 重启服务 / 推广 / 关闭. `关闭` maps to upstream menuClose/menuExit.
final List<AppMenuEntry> mainMenuModel = <AppMenuEntry>[
  AppMenuEntry(
    label: '配置项',
    l10nKey: 'menuServers',
    actionId: 'UI-GROUP-SERVERS',
    submenu: _serverEntries,
  ),
  AppMenuEntry(
    label: '订阅分组',
    l10nKey: 'menuSubscription',
    actionId: 'UI-GROUP-SUBS',
    submenu: _subscriptionEntries,
  ),
  AppMenuEntry(
    label: '设置',
    l10nKey: 'menuSetting',
    actionId: 'UI-GROUP-SETTINGS',
    submenu: _settingEntries,
  ),
  AppMenuEntry(
    label: '帮助',
    l10nKey: 'menuHelp',
    actionId: 'UI-GROUP-HELP',
    submenu: _helpEntries,
  ),
  // ACT-MAIN-035: upstream `menuReload` (ResUI "重启服务" / "Reload"). Upstream
  // draws no InputGestureText, so the RC-only F5 chip is dropped; F5 still
  // triggers the same use case through the shell shortcut.
  AppMenuEntry(label: '重启服务', l10nKey: 'menuReload', actionId: 'ACT-MAIN-035'),
  AppMenuEntry(
    label: '推广',
    l10nKey: 'menuPromotion',
    actionId: 'ACT-WIN-003',
    preservedOnly: true,
  ),
  AppMenuEntry(
    label: '关闭',
    l10nKey: 'menuClose',
    actionId: 'ACT-WIN-002',
    shortcut: 'Alt+F4',
  ),
];

/// Flat lookup of every entry in the tree (for tests/assertions).
Iterable<AppMenuEntry> flattenMenu(
  List<AppMenuEntry> entries, [
  String prefix = '',
]) sync* {
  for (final entry in entries) {
    yield entry;
    if (entry.submenu.isNotEmpty) {
      yield* flattenMenu(entry.submenu, '$prefix${entry.label}/');
    }
  }
}

/// True when [entry] is preserved-only and therefore must render disabled.
bool isPreservedOnly(AppMenuEntry entry) => entry.preservedOnly;
