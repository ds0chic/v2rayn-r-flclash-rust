/// Data model for the top menu. Each backend entry carries the `ACT-*` ledger
/// id from `compat/actions.yaml`; UI-only entries carry a synthetic `UI-*`
/// id. The wire-up in [MainShell] decides what actually runs.
class AppMenuEntry {
  const AppMenuEntry({
    required this.label,
    this.actionId,
    this.shortcut,
    this.enabled = true,
    this.submenu = const <AppMenuEntry>[],
  });

  final String label;
  final String? actionId;
  final String? shortcut;
  final bool enabled;
  final List<AppMenuEntry> submenu;

  bool get isSubmenu => submenu.isNotEmpty;
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
  AppMenuEntry(label: '扫描图片中的二维码', actionId: 'ACT-MAIN-018'),
  AppMenuEntry(label: '添加自定义配置', actionId: 'ACT-MAIN-012'),
  AppMenuEntry(label: '添加策略组', actionId: 'ACT-MAIN-014'),
  AppMenuEntry(label: '添加链式代理', actionId: 'ACT-MAIN-015'),
  AppMenuEntry(label: '添加自定义出站', actionId: 'ACT-MAIN-013'),
  AppMenuEntry(label: '添加 [VMess]', actionId: 'ACT-MAIN-001'),
  AppMenuEntry(label: '添加 [VLESS]', actionId: 'ACT-MAIN-002'),
  AppMenuEntry(label: '添加 [Shadowsocks]', actionId: 'ACT-MAIN-003'),
  AppMenuEntry(label: '添加 [Trojan]', actionId: 'ACT-MAIN-006'),
  AppMenuEntry(label: '添加 [Hysteria2]', actionId: 'ACT-MAIN-007'),
  AppMenuEntry(label: '添加 [WireGuard]', actionId: 'ACT-MAIN-009'),
  AppMenuEntry(label: '添加 [SOCKS]', actionId: 'ACT-MAIN-004'),
  AppMenuEntry(label: '添加 [HTTP]', actionId: 'ACT-MAIN-005'),
  AppMenuEntry(label: '添加 [TUIC]', actionId: 'ACT-MAIN-008'),
  AppMenuEntry(label: '添加 [Anytls]', actionId: 'ACT-MAIN-010'),
  AppMenuEntry(label: '添加 [NaïveProxy]', actionId: 'ACT-MAIN-011'),
];

/// ACT-MAIN-019 (sub settings) and ACT-* subscription update entries.
const _subscriptionEntries = <AppMenuEntry>[
  AppMenuEntry(label: '订阅分组设置', actionId: 'ACT-MAIN-019'),
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
];

const _settingEntries = <AppMenuEntry>[
  AppMenuEntry(label: '参数设置', actionId: 'ACT-MAIN-024'),
  AppMenuEntry(label: '路由设置', actionId: 'ACT-MAIN-025'),
  AppMenuEntry(label: 'DNS 设置', actionId: 'ACT-MAIN-026'),
  AppMenuEntry(label: '完整配置模板设置', actionId: 'ACT-MAIN-027'),
  AppMenuEntry(label: '全局热键设置', actionId: 'ACT-MAIN-028'),
  AppMenuEntry(label: '以管理员身份重启', actionId: 'ACT-MAIN-029'),
  AppMenuEntry(label: '解除 Win10 UWP 应用回环代理限制', actionId: 'ACT-WIN-004'),
  AppMenuEntry(label: '清除所有服务统计数据', actionId: 'ACT-MAIN-030'),
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
  AppMenuEntry(label: '主界面 (占位)', actionId: 'UI-MAIN', submenu: _uiEntries),
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
  AppMenuEntry(
    label: '帮助',
    actionId: 'UI-GROUP-HELP',
    submenu: <AppMenuEntry>[
      AppMenuEntry(label: '检查更新', actionId: 'ACT-WIN-005'),
      AppMenuEntry(label: '核心网站 (占位)', actionId: 'ACT-WIN-008'),
    ],
  ),
  AppMenuEntry(label: '重启服务', actionId: 'ACT-MAIN-035', shortcut: 'F5'),
  AppMenuEntry(label: '推广', actionId: 'ACT-WIN-003'),
  AppMenuEntry(label: '关闭', actionId: 'ACT-WIN-002', shortcut: 'Alt+F4'),
];
