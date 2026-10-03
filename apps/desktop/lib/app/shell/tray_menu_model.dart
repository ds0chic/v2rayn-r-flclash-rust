import 'package:flutter_riverpod/flutter_riverpod.dart';
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
    this.children = const <TraySubEntry>[],
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

  /// Dynamic entries for the routing/servers submenus, synced from the same
  /// controllers the main window uses (RT-11).
  final List<TraySubEntry> children;
}

/// One dynamic submenu entry (routing profile or node server).
class TraySubEntry {
  const TraySubEntry({
    required this.id,
    required this.label,
    this.checked = false,
  });

  final String id;
  final String label;
  final bool checked;
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

/// Bridges tray clicks to the same use cases the main window runs. The shell
/// installs the handlers at bootstrap so the tray never reimplements import /
/// scan / update / activate (RT-11). A null handler means the action reports an
/// honest "not wired" status instead of pretending to have run.
class TrayCommandDelegate {
  TrayCommandDelegate({
    this.onSharedCommand,
    this.onSelectNode,
    this.onSelectRouting,
  });

  /// Runs a shared `ACT-MAIN-*` command id.
  final void Function(String command)? onSharedCommand;
  final void Function(String nodeId)? onSelectNode;
  final void Function(String routingId)? onSelectRouting;
}

/// Shared main-window command a tray leaf action maps to (RT-11). Returning the
/// same `ACT-MAIN-*` id the main menu uses means the tray runs the identical use
/// case instead of a tray-only reimplementation. `null` = tray-only action.
String? sharedCommandForTrayAction(String trayActionId) {
  switch (trayActionId) {
    case 'ACT-TRAY-008':
      return 'ACT-MAIN-016'; // 从剪贴板导入分享链接
    case 'ACT-TRAY-009':
      return 'ACT-MAIN-017'; // 扫描屏幕上的二维码
    case 'ACT-TRAY-010':
      return 'ACT-MAIN-020'; // 更新订阅（不通过代理）
    case 'ACT-TRAY-011':
      return 'ACT-MAIN-021'; // 更新订阅（通过代理）
    default:
      return null;
  }
}

/// Static tray menu model, in the exact upstream `StatusBarView.xaml`
/// `ContextMenu` order (ACT-TRAY-002..013). Dynamic submenu contents (routings,
/// nodes) are synced from the live controllers so the menu matches the main
/// window (RT-11).
List<TrayMenuItem> trayMenuModel({
  required SysProxyMode currentMode,
  required bool pacVisible,
  List<TraySubEntry> routings = const <TraySubEntry>[],
  List<TraySubEntry> nodes = const <TraySubEntry>[],
  int serversLimit = 0,
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
  final serversVisible = trayServersVisible(
    nodeCount: nodes.length,
    limit: serversLimit,
  );
  items.addAll(<TrayMenuItem>[
    TrayMenuItem(
      actionId: 'ACT-TRAY-006',
      label: '路由',
      separatorBefore: true,
      kind: TrayItemKind.routingSubmenu,
      children: routings,
    ),
    TrayMenuItem(
      actionId: 'ACT-TRAY-007',
      label: '节点',
      kind: TrayItemKind.serversSubmenu,
      children: serversVisible ? nodes : const <TraySubEntry>[],
    ),
    const TrayMenuItem(
      actionId: 'ACT-TRAY-008',
      label: '从剪贴板导入分享链接',
      separatorBefore: true,
    ),
    const TrayMenuItem(actionId: 'ACT-TRAY-009', label: '扫描屏幕上的二维码'),
    const TrayMenuItem(actionId: 'ACT-TRAY-010', label: '更新订阅 (不通过代理)'),
    const TrayMenuItem(actionId: 'ACT-TRAY-011', label: '更新订阅 (通过代理)'),
    const TrayMenuItem(
      actionId: 'ACT-TRAY-012',
      label: '复制代理命令到剪贴板',
      separatorBefore: true,
    ),
    const TrayMenuItem(
      actionId: 'ACT-TRAY-013',
      label: '退出',
      separatorBefore: true,
    ),
  ]);
  return items;
}

/// Whether the tray servers submenu should be shown: upstream hides it when the
/// node count exceeds `TrayMenuServersLimit` (`BlServers = false`).
bool trayServersVisible({required int nodeCount, required int limit}) {
  if (limit <= 0) return true;
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

/// Outcome of a tray click, so the shell can route it to the shared use case or
/// the tray-only behavior (proxy mode / exit / window toggle).
class TrayAction {
  const TrayAction({
    required this.actionId,
    this.mode,
    this.nodeId,
    this.routingId,
    this.sharedCommand,
  });

  final String actionId;
  final SysProxyMode? mode;
  final String? nodeId;
  final String? routingId;

  /// The shared `ACT-MAIN-*` command, when this leaf maps to one.
  final String? sharedCommand;

  static TrayAction fromMenuItem(TrayMenuItem entry) => TrayAction(
    actionId: entry.actionId,
    mode: entry.radioGroup,
    sharedCommand: sharedCommandForTrayAction(entry.actionId),
  );

  static TrayAction selectNode(String nodeId) =>
      TrayAction(actionId: 'ACT-TRAY-007', nodeId: nodeId);

  static TrayAction selectRouting(String routingId) =>
      TrayAction(actionId: 'ACT-TRAY-006', routingId: routingId);
}

/// Mutable holder so the shell can install the tray delegate at bootstrap
/// (mirrors `DesktopIntegrationHolder`); stays null in widget tests.
class TrayCommandDelegateHolder {
  TrayCommandDelegate? value;
}

final trayCommandDelegateProvider = Provider<TrayCommandDelegateHolder>(
  (ref) => TrayCommandDelegateHolder(),
);
