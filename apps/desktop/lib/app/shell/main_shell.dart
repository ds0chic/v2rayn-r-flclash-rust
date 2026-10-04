import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/menu/main_menu.dart';
import 'package:v2rayn_desktop/app/shell/desktop_integration.dart';
import 'package:v2rayn_desktop/app/shell/side_tabs.dart';
import 'package:v2rayn_desktop/app/shell/status_bar_view.dart';
import 'package:v2rayn_desktop/app/shell/tray_menu_model.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/backup/backup_and_restore_view.dart';
import 'package:v2rayn_desktop/features/backup/backup_controller.dart';
import 'package:v2rayn_desktop/features/update/check_update_view.dart';
import 'package:v2rayn_desktop/features/profiles/column_settings_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/profile_actions.dart'
    as profile_actions;
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_page.dart';
import 'package:v2rayn_desktop/features/routing/routing_actions.dart'
    as routing_actions;
import 'package:v2rayn_desktop/features/routing/routing_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/settings_actions.dart';
import 'package:v2rayn_desktop/features/subs/scan_image_qr.dart';
import 'package:v2rayn_desktop/features/subs/scan_screen_qr.dart';
import 'package:v2rayn_desktop/features/subs/subs_actions.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';
import 'package:v2rayn_desktop/shared/widgets/horizontal_toolbar.dart';

/// Main window shell: top menu/toolbar, three grid layouts, bottom status bar.
/// Mirrors compat/layouts.yaml LAY-MAIN-001/002/003 and LAY-MAIN-004.
class MainShell extends ConsumerStatefulWidget {
  const MainShell({super.key});

  @override
  ConsumerState<MainShell> createState() => _MainShellState();
}

class _MainShellState extends ConsumerState<MainShell> {
  @override
  void initState() {
    super.initState();
    // Evidence-run hook: `V2RAYN_R_OPEN_SUBS=1` opens the subscription settings
    // window on launch so the T09 screenshot can be captured without scripted
    // menu clicks. It is a no-op in normal runs.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      // RT-11: the tray runs the identical use cases as the main window.
      ref.read(trayCommandDelegateProvider).value = TrayCommandDelegate(
        onSharedCommand: (command) => _onMenuActionId(context, ref, command),
        onSelectNode: (id) => profile_actions.activateProfileById(ref, id),
        onSelectRouting: (id) =>
            ref.read(routingControllerProvider.notifier).select(id),
      );
      final open = Platform.environment['V2RAYN_R_OPEN_SUBS'];
      if (open == '1' || open == 'true') {
        openSubSettings(context, ref);
      }
      // T10 evidence hooks (same pattern): auto-open the group editor or the
      // template window for release screenshots. No-ops in normal runs.
      final openGroup = Platform.environment['V2RAYN_R_OPEN_GROUP'];
      if (openGroup == '1' || openGroup == 'true') {
        profile_actions.startAddGroupProfile(
          context,
          ref,
          ConfigType.policyGroup,
        );
      }
      final openTemplate = Platform.environment['V2RAYN_R_OPEN_TEMPLATE'];
      if (openTemplate == '1' || openTemplate == 'true') {
        profile_actions.openFullConfigTemplateWindow(context, ref);
      }
      // T12a evidence hooks: auto-open the settings/theme/hotkey windows for
      // release screenshots. No-ops in normal runs.
      final openSettings = Platform.environment['V2RAYN_R_OPEN_SETTINGS'];
      if (openSettings == '1' || openSettings == 'true') {
        openOptionSettingWindow(context, ref);
      }
      final openTheme = Platform.environment['V2RAYN_R_OPEN_THEME'];
      if (openTheme == '1' || openTheme == 'true') {
        openThemeSettingDialog(context, ref);
      }
      final openHotkey = Platform.environment['V2RAYN_R_OPEN_HOTKEY'];
      if (openHotkey == '1' || openHotkey == 'true') {
        openGlobalHotkeyWindow(context, ref);
      }
      // T11 evidence hooks: auto-open the routing / DNS windows for release
      // screenshots. No-ops in normal runs.
      final openRouting = Platform.environment['V2RAYN_R_OPEN_ROUTING'];
      if (openRouting == '1' || openRouting == 'true') {
        routing_actions.openRoutingSettings(context, ref);
      }
      final openDns = Platform.environment['V2RAYN_R_OPEN_DNS'];
      if (openDns == '1' || openDns == 'true') {
        routing_actions.openDnsSettings(context, ref);
      }
      // T16 evidence hooks: auto-open the backup / update windows for release
      // screenshots. No-ops in normal runs.
      final openBackup = Platform.environment['V2RAYN_R_OPEN_BACKUP'];
      if (openBackup == '1' || openBackup == 'true') {
        showBackupAndRestoreWindow(context, ref);
      }
      final openUpdate = Platform.environment['V2RAYN_R_OPEN_UPDATE'];
      if (openUpdate == '1' || openUpdate == 'true') {
        showCheckUpdateWindow(context, ref);
      }
      // T15a evidence hooks: configure the monitor against a loopback Xray
      // stats port and open a monitor tab for release screenshots. No-ops in
      // normal runs.
      final xrayPort = Platform.environment['V2RAYN_R_MONITOR_XRAY_PORT'];
      if (xrayPort != null) {
        final port = int.tryParse(xrayPort);
        if (port != null && port > 0) {
          final monitor = ref.read(monitorControllerProvider.notifier);
          monitor.configure(
            core: 2, // domain CoreType::Xray
            statePort: port,
            statePort2: 0,
            enableStatistics: true,
            displayRealTimeSpeed: true,
            refreshIntervalMs: 1000,
          );
          monitor.startPolling();
        }
      }
      // T15a/T20 evidence hook (only in smoke-armed builds, see main.dart):
      // apply the real persisted plan on launch so the logs tab has real core
      // output for release screenshots.
      const bool smokeArmed = bool.fromEnvironment(
        'V2RAYN_R_SMOKE_ARMED',
        defaultValue: false,
      );
      final autoSmoke = Platform.environment['V2RAYN_R_AUTO_SMOKE'];
      if (smokeArmed && (autoSmoke == '1' || autoSmoke == 'true')) {
        ref.read(runtimeControllerProvider.notifier).applyActive();
      }
      final openTab = Platform.environment['V2RAYN_R_OPEN_TAB'];
      if (openTab != null) {
        // Right-tab indices (vertical/horizontal layouts): 0 信息 / 1 当前代理
        // / 2 当前连接. Default layout is vertical, so these select the tab.
        final index = switch (openTab) {
          'logs' || 'info' => 0,
          'proxies' => 1,
          'connections' => 2,
          _ => 0,
        };
        ref.read(uiShellControllerProvider.notifier).setTabIndex(index);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(uiShellControllerProvider);
    final scheme = Theme.of(context).colorScheme;

    return Scaffold(
      body: CallbackShortcuts(
        bindings: <ShortcutActivator, VoidCallback>{
          const SingleActivator(LogicalKeyboardKey.f5): () => _guarded(
            ref,
            () => ref.read(runtimeControllerProvider.notifier).reload(),
          ),
          const SingleActivator(LogicalKeyboardKey.keyS, control: true): () =>
              _guarded(ref, () => scanScreenQr(context, ref)),
          const SingleActivator(LogicalKeyboardKey.keyV, control: true): () =>
              _guarded(ref, () => importFromClipboard(context, ref)),
          const SingleActivator(LogicalKeyboardKey.keyC, control: true): () =>
              _guarded(ref, () => exportProfiles(context, ref, kind: 'share')),
        },
        child: Column(
          children: <Widget>[
            _MenuToolbarBar(
              onAction: (entry) => _onMenuAction(context, ref, entry),
            ),
            const Divider(height: 1),
            Expanded(child: _buildLayout(context, ref, state)),
            const Divider(height: 1),
            const StatusBarView(),
          ],
        ),
      ),
      backgroundColor: scheme.surface,
    );
  }

  Widget _buildLayout(BuildContext context, WidgetRef ref, UiShellState state) {
    switch (state.layout) {
      case AppLayoutMode.horizontal:
        final split = state.horizontalSplit;
        return LayoutBuilder(
          builder: (context, constraints) => Row(
            children: <Widget>[
              Expanded(flex: _flex(split), child: const ProfilesPage()),
              _SplitHandle(
                key: const ValueKey('split-horizontal'),
                axis: Axis.horizontal,
                onDrag: (delta) => ref
                    .read(uiShellControllerProvider.notifier)
                    .nudgeHorizontalSplit(delta / constraints.maxWidth),
              ),
              Expanded(
                flex: _flex(1 - split),
                child: SideTabs(
                  tabs: _rightTabs,
                  placement: TabStripPlacement.top,
                  tabIndex: state.tabIndex,
                  onTabSelected: ref
                      .read(uiShellControllerProvider.notifier)
                      .setTabIndex,
                ),
              ),
            ],
          ),
        );
      case AppLayoutMode.vertical:
        final split = state.verticalSplit;
        return LayoutBuilder(
          builder: (context, constraints) => Column(
            children: <Widget>[
              Expanded(flex: _flex(split), child: const ProfilesPage()),
              _SplitHandle(
                key: const ValueKey('split-vertical'),
                axis: Axis.vertical,
                onDrag: (delta) => ref
                    .read(uiShellControllerProvider.notifier)
                    .nudgeVerticalSplit(delta / constraints.maxHeight),
              ),
              Expanded(
                flex: _flex(1 - split),
                child: SideTabs(
                  tabs: _rightTabs,
                  placement: TabStripPlacement.left,
                  tabIndex: state.tabIndex,
                  onTabSelected: ref
                      .read(uiShellControllerProvider.notifier)
                      .setTabIndex,
                ),
              ),
            ],
          ),
        );
      case AppLayoutMode.tab:
        return SideTabs(
          tabs: AppTab.values,
          placement: TabStripPlacement.left,
          tabIndex: state.tabIndex,
          onTabSelected: ref
              .read(uiShellControllerProvider.notifier)
              .setTabIndex,
        );
    }
  }

  static const _rightTabs = <AppTab>[
    AppTab.info,
    AppTab.proxies,
    AppTab.connections,
  ];

  static int _flex(double fraction) =>
      (fraction.clamp(0.05, 0.95) * 1000).round().clamp(1, 999);

  static void _guarded(WidgetRef ref, VoidCallback action) {
    final focus = FocusManager.instance.primaryFocus;
    final editing = focus?.context
        ?.findAncestorStateOfType<EditableTextState>();
    if (editing != null) return; // HKR-002: don't hijack text-field input.
    action();
  }

  void _onMenuAction(BuildContext context, WidgetRef ref, AppMenuEntry entry) =>
      _onMenuActionId(context, ref, entry.actionId ?? '');

  /// Finds a menu entry by action id for the string-based dispatch fallback.
  static AppMenuEntry? _findMenuEntry(String actionId) {
    AppMenuEntry? walk(List<AppMenuEntry> entries) {
      for (final entry in entries) {
        if (entry.actionId == actionId) return entry;
        final nested = walk(entry.submenu);
        if (nested != null) return nested;
      }
      return null;
    }

    return walk(mainMenuModel);
  }

  /// String-based dispatch shared by the main menu and the tray delegate so a
  /// tray click runs the identical `ACT-MAIN-*` use case (RT-11).
  void _onMenuActionId(BuildContext context, WidgetRef ref, String actionId) {
    final shell = ref.read(uiShellControllerProvider.notifier);
    final profiles = ref.read(profilesControllerProvider.notifier);
    switch (actionId) {
      case 'UI-LAYOUT-H':
        shell.setLayout(AppLayoutMode.horizontal);
      case 'UI-LAYOUT-V':
        shell.setLayout(AppLayoutMode.vertical);
      case 'UI-LAYOUT-T':
        shell.setLayout(AppLayoutMode.tab);
      case 'UI-COLUMNS':
        showColumnSettingsDialog(context, ref);
      case 'UI-DBLCLICK':
        profiles.toggleDoubleClick2Activate();
      case 'UI-THEME':
        shell.toggleTheme();
      case 'UI-ZEBRA':
        shell.toggleZebraStriping();
      case 'ACT-MAIN-001':
        profile_actions.startAddProfile(context, ref, ConfigType.vmess);
      case 'ACT-MAIN-002':
        profile_actions.startAddProfile(context, ref, ConfigType.vless);
      case 'ACT-MAIN-003':
        profile_actions.startAddProfile(context, ref, ConfigType.shadowsocks);
      case 'ACT-MAIN-004':
        profile_actions.startAddProfile(context, ref, ConfigType.socks);
      case 'ACT-MAIN-005':
        profile_actions.startAddProfile(context, ref, ConfigType.http);
      case 'ACT-MAIN-006':
        profile_actions.startAddProfile(context, ref, ConfigType.trojan);
      case 'ACT-MAIN-007':
        profile_actions.startAddProfile(context, ref, ConfigType.hysteria2);
      case 'ACT-MAIN-008':
        profile_actions.startAddProfile(context, ref, ConfigType.tuic);
      case 'ACT-MAIN-009':
        profile_actions.startAddProfile(context, ref, ConfigType.wireGuard);
      case 'ACT-MAIN-010':
        profile_actions.startAddProfile(context, ref, ConfigType.anytls);
      case 'ACT-MAIN-011':
        profile_actions.startAddProfile(context, ref, ConfigType.naive);
      case 'ACT-MAIN-012':
        profile_actions.startAddCustomProfile(context, ref, ConfigType.custom);
      case 'ACT-MAIN-013':
        profile_actions.startAddCustomProfile(
          context,
          ref,
          ConfigType.outbound,
        );
      case 'ACT-MAIN-014':
        profile_actions.startAddGroupProfile(
          context,
          ref,
          ConfigType.policyGroup,
        );
      case 'ACT-MAIN-015':
        profile_actions.startAddGroupProfile(
          context,
          ref,
          ConfigType.proxyChain,
        );
      case 'ACT-MAIN-016':
        importFromClipboard(context, ref);
      case 'ACT-MAIN-017':
        scanScreenQr(context, ref);
      case 'ACT-MAIN-018':
        scanImageQr(context, ref);
      case 'ACT-MAIN-019':
        openSubSettings(context, ref);
      case 'ACT-MAIN-020':
        updateAllSubscriptions(context, ref, viaProxy: false);
      case 'ACT-MAIN-021':
        updateAllSubscriptions(context, ref, viaProxy: true);
      case 'ACT-MAIN-022':
        updateCurrentGroup(context, ref, viaProxy: false);
      case 'ACT-MAIN-023':
        updateCurrentGroup(context, ref, viaProxy: true);
      case 'ACT-MAIN-027':
        profile_actions.openFullConfigTemplateWindow(context, ref);
      case 'ACT-MAIN-024':
        openOptionSettingWindow(context, ref);
      case 'ACT-MAIN-028':
        openGlobalHotkeyWindow(context, ref);
      case 'UI-THEME-WINDOW':
        openThemeSettingDialog(context, ref);
      case 'ACT-MAIN-025':
        routing_actions.openRoutingSettings(context, ref);
      case 'ACT-MAIN-026':
        routing_actions.openDnsSettings(context, ref);
      case 'ACT-MAIN-029':
        // F-DESKTOP-006 / ACT-MAIN-029: relaunch elevated. The actual `runas`
        // start happens in the desktop runtime only on this explicit action.
        shell.notImplemented('以管理员身份重启', 'ACT-MAIN-029');
      case 'ACT-WIN-004':
        // F-DESKTOP-005 / ACT-WIN-004: resolve the UWP loopback tool. Starting
        // it is a user-visible side effect; report precisely what would run.
        final ok = ref.read(platformBridgeProvider).resolveUwpLoopbackTool();
        shell.setMessage(
          ok
              ? '已找到 EnableLoopback.exe（启动需用户确认）'
              : '未找到 EnableLoopback.exe（bin 目录）',
        );
      case 'ACT-MAIN-030':
        // F-MONITOR-003 / ACT-MAIN-030: clear ServerStatItem rows.
        final cleared = ref
            .read(monitorControllerProvider.notifier)
            .clearStats();
        shell.setMessage(cleared ? '已清除所有服务统计数据' : '清除统计失败');
      case 'ACT-MAIN-031':
        ref.read(backupControllerProvider.notifier).openConfigDir();
      case 'ACT-WIN-005':
        showCheckUpdateWindow(context, ref);
      case 'ACT-WIN-007':
        showBackupAndRestoreWindow(context, ref);
      case 'UI-CLEANUP':
        ref.read(backupControllerProvider.notifier).cleanupLogsTmp();
      case 'ACT-WIN-002':
        // Upstream menuClose minimizes to the tray (ACT-WIN-002). Falls back
        // to an honest status message when no desktop integration is present
        // (widget tests) so the entry never pretends to have run.
        final integration = ref.read(desktopIntegrationProvider).value;
        if (integration != null) {
          integration.hideToTray();
        } else {
          shell.setMessage('已请求最小化到托盘 (ACT-WIN-002)');
        }
      case 'ACT-MAIN-035':
        ref.read(runtimeControllerProvider.notifier).reload();
      default:
        final entry = _findMenuEntry(actionId);
        if (entry != null && entry.preservedOnly) {
          shell.setMessage('${AppMenuEntry.preservedTooltip}: ${entry.label}');
        } else {
          shell.notImplemented(entry?.label ?? actionId, actionId);
        }
    }
  }
}

class _MenuToolbarBar extends ConsumerWidget {
  const _MenuToolbarBar({required this.onAction});

  final ValueChanged<AppMenuEntry> onAction;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final shell = ref.watch(uiShellControllerProvider);
    final controller = ref.read(uiShellControllerProvider.notifier);
    final scheme = Theme.of(context).colorScheme;
    return Material(
      color: scheme.surfaceContainer,
      child: Row(
        children: <Widget>[
          Expanded(
            child: HorizontalToolbar(
              child: MenuBar(
                children: <Widget>[
                  for (final group in mainMenuModel) _topLevel(group),
                ],
              ),
            ),
          ),
          const SizedBox(width: 8),
          const _RuntimeToolbar(),
          PopupMenuButton<AppLayoutMode>(
            key: const ValueKey('layout-selector'),
            tooltip: '主界面布局',
            initialValue: shell.layout,
            onSelected: controller.setLayout,
            itemBuilder: (context) => <PopupMenuEntry<AppLayoutMode>>[
              for (final mode in AppLayoutMode.values)
                PopupMenuItem<AppLayoutMode>(
                  value: mode,
                  child: Row(
                    children: <Widget>[
                      Icon(mode.icon, size: 16),
                      const SizedBox(width: 8),
                      Text(mode.label, style: const TextStyle(fontSize: 12)),
                    ],
                  ),
                ),
            ],
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
              child: Row(
                children: <Widget>[
                  Icon(shell.layout.icon, size: 16),
                  const SizedBox(width: 4),
                  Text(
                    shell.layout.label,
                    style: const TextStyle(fontSize: 12),
                  ),
                ],
              ),
            ),
          ),
          IconButton(
            key: const ValueKey('theme-toggle'),
            tooltip: '切换浅色/深色',
            iconSize: 18,
            onPressed: controller.toggleTheme,
            icon: Icon(
              shell.themeMode == ThemeMode.dark
                  ? Icons.light_mode_outlined
                  : Icons.dark_mode_outlined,
            ),
          ),
          Visibility(
            visible: false,
            maintainState: true,
            maintainAnimation: true,
            maintainSize: true,
            child: Padding(
              padding: const EdgeInsets.only(right: 8, left: 4),
              child: OutlinedButton(
                key: const ValueKey('btn-new-update'),
                onPressed: () =>
                    controller.notImplemented('有更新', 'ACT-WIN-006'),
                child: const Text('有更新', style: TextStyle(fontSize: 12)),
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _topLevel(AppMenuEntry entry) {
    if (entry.isSubmenu) return _submenu(entry);
    return _tooltip(
      entry,
      MenuItemButton(
        key: ValueKey('menu-${entry.label}'),
        onPressed: entry.isInvocable ? () => onAction(entry) : null,
        // A top-level MenuBar item lays out in an unbounded row, so it must
        // not use an Expanded label; keep the shortcut as a trailing chip.
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Text(
                entry.label,
                style: const TextStyle(fontSize: AppTokens.fontSize),
              ),
              if (entry.shortcut != null) ...<Widget>[
                const SizedBox(width: 6),
                Text(
                  entry.shortcut!,
                  style: const TextStyle(fontSize: 10, color: Colors.grey),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }

  Widget _submenu(AppMenuEntry entry) {
    return SubmenuButton(
      key: ValueKey('menu-${entry.label}'),
      menuChildren: _menuChildren(entry.submenu),
      child: Text(
        entry.label,
        style: const TextStyle(fontSize: AppTokens.fontSize),
      ),
    );
  }

  List<Widget> _menuChildren(List<AppMenuEntry> entries) {
    final widgets = <Widget>[];
    for (final entry in entries) {
      if (entry.isSubmenu) {
        widgets.add(
          SubmenuButton(
            key: ValueKey('menu-item-${entry.label}'),
            menuChildren: _menuChildren(entry.submenu),
            child: _menuItemLabel(entry),
          ),
        );
      } else {
        widgets.add(
          _tooltip(
            entry,
            MenuItemButton(
              key: ValueKey('menu-item-${entry.label}'),
              onPressed: entry.isInvocable ? () => onAction(entry) : null,
              child: _menuItemLabel(entry),
            ),
          ),
        );
      }
    }
    return widgets;
  }

  /// Preserved-only entries carry the "保留原版入口（未实现）" tooltip; a normal
  /// entry simply advertises its shortcut so the keyboard path is discoverable.
  Widget _tooltip(AppMenuEntry entry, Widget child) {
    final message = entry.preservedOnly
        ? AppMenuEntry.preservedTooltip
        : (entry.shortcut ?? '');
    if (message.isEmpty) return child;
    return Tooltip(message: message, child: child);
  }

  Widget _menuItemLabel(AppMenuEntry entry) {
    return Row(
      children: <Widget>[
        Expanded(
          child: Text(entry.label, style: const TextStyle(fontSize: 12)),
        ),
        if (entry.shortcut != null)
          Padding(
            padding: const EdgeInsets.only(left: 24),
            child: Text(
              entry.shortcut!,
              style: const TextStyle(fontSize: 11, color: Colors.grey),
            ),
          ),
      ],
    );
  }
}

/// Runtime controls (T18b): apply the real persisted plan for the active
/// node, or stop the managed core. Errors surface verbatim; a
/// desired/applied revision mismatch renders as 未应用.
class _RuntimeToolbar extends ConsumerWidget {
  const _RuntimeToolbar();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final runtime = ref.watch(runtimeControllerProvider);
    final controller = ref.read(runtimeControllerProvider.notifier);
    return Row(
      children: <Widget>[
        Text(
          runtime.hasUnappliedChanges
              ? '运行时: ${runtime.state} (未应用)'
              : '运行时: ${runtime.state}',
          key: const ValueKey('runtime-state-chip'),
          style: const TextStyle(fontSize: 12),
        ),
        const SizedBox(width: 8),
        Tooltip(
          message: '应用当前节点与设置/路由/DNS 的真实内核配置',
          child: FilledButton.tonal(
            key: const ValueKey('runtime-start'),
            onPressed: runtime.isBusy ? null : controller.applyActive,
            child: const Text('应用', style: TextStyle(fontSize: 12)),
          ),
        ),
        const SizedBox(width: 6),
        Tooltip(
          message: '停止由本应用启动的内核会话',
          child: OutlinedButton(
            key: const ValueKey('runtime-stop'),
            onPressed: controller.stop,
            child: const Text('停止', style: TextStyle(fontSize: 12)),
          ),
        ),
        const SizedBox(width: 8),
      ],
    );
  }
}

class _SplitHandle extends StatelessWidget {
  const _SplitHandle({super.key, required this.axis, required this.onDrag});

  final Axis axis;
  final ValueChanged<double> onDrag;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final horizontal = axis == Axis.horizontal;
    return MouseRegion(
      cursor: horizontal
          ? SystemMouseCursors.resizeColumn
          : SystemMouseCursors.resizeRow,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onHorizontalDragUpdate: horizontal ? (d) => onDrag(d.delta.dx) : null,
        onVerticalDragUpdate: horizontal ? null : (d) => onDrag(d.delta.dy),
        child: Container(
          width: horizontal ? AppTokens.splitterThickness : null,
          height: horizontal ? null : AppTokens.splitterThickness,
          color: scheme.surfaceContainerHighest,
          child: Center(
            child: Container(
              width: horizontal ? 1 : 24,
              height: horizontal ? 24 : 1,
              color: scheme.outline,
            ),
          ),
        ),
      ),
    );
  }
}
