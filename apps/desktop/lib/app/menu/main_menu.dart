/// Data model for the top menu. Each backend entry carries the `ACT-*` ledger
/// id from `compat/actions.yaml`; UI-only entries carry a synthetic `UI-*`
/// id. The wire-up in [MainShell] decides what actually runs.
///
/// T17: an entry that exists upstream but has no backend yet is marked
/// [preservedOnly]. It stays in place (layout/text unchanged) but renders
/// disabled with the tooltip 保留原版入口（未实现） — it must never be
/// clickable and pretend to succeed.
class AppMenuEntry {
  const AppMenuEntry({
    required this.label,
    this.actionId,
    this.shortcut,
    this.enabled = true,
    this.preservedOnly = false,
    this.separatorAfter = false,
    this.submenu = const <AppMenuEntry>[],
  });

  final String label;
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

  static const String preservedTooltip = '保留原版入口（未实现）';
}

/// ACT-MAIN-016 / 017 / 018 and 001..015 (menuServers, 18 entries).
const _serverEntries = <AppMenuEntry>[
  AppMenuEntry(
    label: '从剪贴板导入分享链接',
    actionId: 'ACT-MAIN-016',
    shortcut: 'Ctrl+V',
  ),
  AppMenuEntry(
    label: '扫描屏幕上的二维码',
    actionId: 'ACT-MAIN-017',
    shortcut: 'Ctrl+S',
  ),
  AppMenuEntry(
    label: '扫描图片中的二维码',
    actionId: 'ACT-MAIN-018',
    separatorAfter: true,
  ),
  AppMenuEntry(label: '添加自定义配置', actionId: 'ACT-MAIN-012'),
  AppMenuEntry(label: '添加策略组', actionId: 'ACT-MAIN-014'),
  AppMenuEntry(label: '添加链式代理', actionId: 'ACT-MAIN-015'),
  AppMenuEntry(
    label: '添加自定义出站',
    actionId: 'ACT-MAIN-013',
    separatorAfter: true,
  ),
  AppMenuEntry(label: '添加 [VMess]', actionId: 'ACT-MAIN-001'),
  AppMenuEntry(label: '添加 [VLESS]', actionId: 'ACT-MAIN-002'),
  AppMenuEntry(label: '添加 [Shadowsocks]', actionId: 'ACT-MAIN-003'),
  AppMenuEntry(label: '添加 [Trojan]', actionId: 'ACT-MAIN-006'),
  AppMenuEntry(label: '添加 [Hysteria2]', actionId: 'ACT-MAIN-007'),
  AppMenuEntry(label: '添加 [WireGuard]', actionId: 'ACT-MAIN-009'),
  AppMenuEntry(label: '添加 [SOCKS]', actionId: 'ACT-MAIN-004'),
  AppMenuEntry(
    label: '添加 [HTTP]',
    actionId: 'ACT-MAIN-005',
    separatorAfter: true,
  ),
  AppMenuEntry(label: '添加 [TUIC]', actionId: 'ACT-MAIN-008'),
  AppMenuEntry(label: '添加 [Anytls]', actionId: 'ACT-MAIN-010'),
  AppMenuEntry(label: '添加 [NaïveProxy]', actionId: 'ACT-MAIN-011'),
];

/// ACT-MAIN-019 (sub settings) and ACT-* subscription update entries.
const _subscriptionEntries = <AppMenuEntry>[
  AppMenuEntry(label: '订阅分组设置', actionId: 'ACT-MAIN-019', separatorAfter: true),
  AppMenuEntry(label: '更新全部订阅 (不通过代理)', actionId: 'ACT-MAIN-020'),
  AppMenuEntry(label: '更新全部订阅 (通过代理)', actionId: 'ACT-MAIN-021'),
  AppMenuEntry(label: '更新当前订阅 (不通过代理)', actionId: 'ACT-MAIN-022'),
  AppMenuEntry(label: '更新当前订阅 (通过代理)', actionId: 'ACT-MAIN-023'),
];

/// UI-only entries (no backend yet) exposed under 设置/主界面. They really take
/// effect in the UI layer; no ACT-* id exists upstream for a direct menu entry.
const _uiEntries = <AppMenuEntry>[
  AppMenuEntry(label: '水平布局', actionId: 'UI-LAYOUT-H'),
  AppMenuEntry(label: '垂直布局', actionId: 'UI-LAYOUT-V'),
  AppMenuEntry(label: '标签布局', actionId: 'UI-LAYOUT-T'),
  AppMenuEntry(label: '显示列设置', actionId: 'UI-COLUMNS'),
  AppMenuEntry(label: '切换双击激活', actionId: 'UI-DBLCLICK'),
  AppMenuEntry(label: '切换浅色/深色', actionId: 'UI-THEME'),
  AppMenuEntry(label: '表格斑马纹', actionId: 'UI-ZEBRA'),
];

const _settingEntries = <AppMenuEntry>[
  AppMenuEntry(label: '参数设置', actionId: 'ACT-MAIN-024'),
  AppMenuEntry(label: '路由设置', actionId: 'ACT-MAIN-025'),
  AppMenuEntry(label: 'DNS 设置', actionId: 'ACT-MAIN-026'),
  AppMenuEntry(label: '完整配置模板设置', actionId: 'ACT-MAIN-027'),
  AppMenuEntry(label: '全局热键设置', actionId: 'ACT-MAIN-028', separatorAfter: true),
  AppMenuEntry(label: '主题设置', actionId: 'UI-THEME-WINDOW'),
  // ACT-MAIN-029: relaunch the current exe elevated with the runner's
  // `rebootas` marker (upstream `ProcUtils.RebootAsAdmin`). Wired in MainShell.
  AppMenuEntry(label: '以管理员身份重启', actionId: 'ACT-MAIN-029'),
  // ACT-WIN-004: UWP loopback exemption entry. MainShell resolves the bundled
  // `EnableLoopback.exe` / builds the reversible `CheckNetIsolation` command.
  AppMenuEntry(label: '解除 Win10 UWP 应用回环代理限制', actionId: 'ACT-WIN-004'),
  AppMenuEntry(
    label: '清除所有服务统计数据',
    actionId: 'ACT-MAIN-030',
    separatorAfter: true,
  ),
  AppMenuEntry(
    label: '区域预置设置',
    actionId: 'ACT-MAIN-032',
    submenu: <AppMenuEntry>[
      AppMenuEntry(label: '默认区域', actionId: 'ACT-MAIN-032'),
      AppMenuEntry(label: '俄罗斯', actionId: 'ACT-MAIN-033'),
      AppMenuEntry(label: '伊朗', actionId: 'ACT-MAIN-034'),
    ],
  ),
  AppMenuEntry(label: '备份和还原', actionId: 'ACT-WIN-007'),
  AppMenuEntry(label: '打开存储所在的位置', actionId: 'ACT-MAIN-031'),
  AppMenuEntry(label: '清理日志与临时文件', actionId: 'UI-CLEANUP'),
  AppMenuEntry(label: '主界面 (占位)', actionId: 'UI-MAIN', submenu: _uiEntries),
];

/// 帮助 submenu. Upstream only has 检查更新 plus a separator that is filled at
/// runtime with one `<core> 网站` entry per installed core (ACT-WIN-008). The
/// single entry opens the primary core's upstream home page through the OS
/// handler.
const _helpEntries = <AppMenuEntry>[
  AppMenuEntry(label: '检查更新', actionId: 'ACT-WIN-005', separatorAfter: true),
  AppMenuEntry(label: '核心网站', actionId: 'ACT-WIN-008'),
];

/// Top-level menu groups in upstream order: 配置项 / 订阅分组 / 设置 / 帮助 /
/// 重启服务 / 推广 / 关闭. `关闭` maps to upstream menuClose/menuExit.
final List<AppMenuEntry> mainMenuModel = <AppMenuEntry>[
  AppMenuEntry(
    label: '配置项',
    actionId: 'UI-GROUP-SERVERS',
    submenu: _serverEntries,
  ),
  AppMenuEntry(
    label: '订阅分组',
    actionId: 'UI-GROUP-SUBS',
    submenu: _subscriptionEntries,
  ),
  AppMenuEntry(
    label: '设置',
    actionId: 'UI-GROUP-SETTINGS',
    submenu: _settingEntries,
  ),
  AppMenuEntry(label: '帮助', actionId: 'UI-GROUP-HELP', submenu: _helpEntries),
  // ACT-MAIN-035: upstream `menuReload` (ResUI.zh-Hans "重启服务"). Upstream
  // draws no InputGestureText, so the RC-only F5 chip is dropped; F5 still
  // triggers the same use case through the shell shortcut.
  AppMenuEntry(label: '重启服务', actionId: 'ACT-MAIN-035'),
  AppMenuEntry(label: '推广', actionId: 'ACT-WIN-003', preservedOnly: true),
  AppMenuEntry(label: '关闭', actionId: 'ACT-WIN-002', shortcut: 'Alt+F4'),
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
