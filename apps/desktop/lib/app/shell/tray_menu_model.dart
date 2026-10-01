import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

/// One tray menu item. `actionId` is the `ACT-TRAY-*` / `ACT-WIN-*` ledger id;
/// submenus carry dynamic entries (routing / nodes) filled at runtime.
class TrayMenuItem {
  const TrayMenuItem({
    required this.actionId,
    required this.label,
    this.radioGroup,
    this.checked = false,
    this.separatorBefore = false,
    this.kind = TrayItemKind.item,
  });

  final String actionId;
  final String label;

  /// For the four system-proxy modes, the mode this item selects.
  final SysProxyMode? radioGroup;

  /// Whether this item is the currently active mode/selection.
  final bool checked;

  /// Emit a separator before this item (upstream `Separator`).
  final bool separatorBefore;

  final TrayItemKind kind;
}

enum TrayItemKind {
  /// A clickable leaf item.
  item,

  /// An inline `<routing>` submenu (upstream `cmbRoutings` in the tray).
  routingSubmenu,

  /// An inline `<servers>` submenu (upstream `cmbServers`, limited by
  /// `TrayMenuServersLimit`).
  serversSubmenu,
}

/// One node entry for the tray servers submenu.
class TrayNodeEntry {
  const TrayNodeEntry({required this.id, required this.label});

  final String id;
  final String label;
}

/// Static tray menu model, in the exact upstream `StatusBarView.xaml`
/// `ContextMenu` order (ACT-TRAY-002..013). Dynamic submenu contents (routings,
/// nodes) are filled separately so the model stays testable without a backend.
List<TrayMenuItem> trayMenuModel({
  required SysProxyMode currentMode,
  required bool pacVisible,
}) {
  final items = <TrayMenuItem>[
    TrayMenuItem(
      actionId: 'ACT-TRAY-002',
      label: '清除系统代理',
      radioGroup: SysProxyMode.forcedClear,
      checked: currentMode == SysProxyMode.forcedClear,
    ),
    TrayMenuItem(
      actionId: 'ACT-TRAY-003',
      label: '自动配置系统代理',
      radioGroup: SysProxyMode.forcedChange,
      checked: currentMode == SysProxyMode.forcedChange,
    ),
    TrayMenuItem(
      actionId: 'ACT-TRAY-004',
      label: '不改变系统代理',
      radioGroup: SysProxyMode.unchanged,
      checked: currentMode == SysProxyMode.unchanged,
    ),
  ];
  // The PAC entry is Windows-only (`BlSystemProxyPacVisible`).
  if (pacVisible) {
    items.add(
      TrayMenuItem(
        actionId: 'ACT-TRAY-005',
        label: 'Pac 模式',
        radioGroup: SysProxyMode.pac,
        checked: currentMode == SysProxyMode.pac,
      ),
    );
  }
  items.addAll(const <TrayMenuItem>[
    TrayMenuItem(
      actionId: 'ACT-TRAY-006',
      label: '路由',
      separatorBefore: true,
      kind: TrayItemKind.routingSubmenu,
    ),
    TrayMenuItem(
      actionId: 'ACT-TRAY-007',
      label: '节点',
      kind: TrayItemKind.serversSubmenu,
    ),
    TrayMenuItem(
      actionId: 'ACT-TRAY-008',
      label: '从剪贴板导入分享链接',
      separatorBefore: true,
    ),
    TrayMenuItem(actionId: 'ACT-TRAY-009', label: '扫描屏幕上的二维码'),
    TrayMenuItem(actionId: 'ACT-TRAY-010', label: '更新订阅 (不通过代理)'),
    TrayMenuItem(actionId: 'ACT-TRAY-011', label: '更新订阅 (通过代理)'),
    TrayMenuItem(
      actionId: 'ACT-TRAY-012',
      label: '复制代理命令到剪贴板',
      separatorBefore: true,
    ),
    TrayMenuItem(actionId: 'ACT-TRAY-013', label: '退出', separatorBefore: true),
  ]);
  return items;
}

/// Whether the tray servers submenu should be shown: upstream hides it when the
/// node count exceeds `TrayMenuServersLimit` (`BlServers = false`).
bool trayServersVisible({required int nodeCount, required int limit}) {
  return nodeCount <= limit;
}

/// Build the node entries shown in the tray submenu (empty when hidden).
List<TrayNodeEntry> trayNodeEntries({
  required List<TrayNodeEntry> nodes,
  required int limit,
}) {
  if (!trayServersVisible(nodeCount: nodes.length, limit: limit)) {
    return const <TrayNodeEntry>[];
  }
  return nodes;
}
