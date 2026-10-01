import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/menu/main_menu.dart';
import 'package:v2rayn_desktop/app/shell/side_tabs.dart';
import 'package:v2rayn_desktop/app/shell/status_bar_view.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/profiles/column_settings_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_page.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// Main window shell: top menu/toolbar, three grid layouts, bottom status bar.
/// Mirrors compat/layouts.yaml LAY-MAIN-001/002/003 and LAY-MAIN-004.
class MainShell extends ConsumerWidget {
  const MainShell({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(uiShellControllerProvider);
    final scheme = Theme.of(context).colorScheme;

    return Scaffold(
      body: CallbackShortcuts(
        bindings: <ShortcutActivator, VoidCallback>{
          const SingleActivator(LogicalKeyboardKey.f5): () => ref
              .read(uiShellControllerProvider.notifier)
              .notImplemented('重启服务', 'ACT-MAIN-035'),
          const SingleActivator(LogicalKeyboardKey.keyS, control: true): () =>
              _guarded(
                ref,
                () => ref
                    .read(uiShellControllerProvider.notifier)
                    .notImplemented('扫描屏幕上的二维码', 'ACT-MAIN-017'),
              ),
          const SingleActivator(LogicalKeyboardKey.keyV, control: true): () =>
              _guarded(
                ref,
                () => ref
                    .read(uiShellControllerProvider.notifier)
                    .notImplemented('从剪贴板导入分享链接', 'ACT-MAIN-016'),
              ),
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

  void _onMenuAction(BuildContext context, WidgetRef ref, AppMenuEntry entry) {
    final shell = ref.read(uiShellControllerProvider.notifier);
    final profiles = ref.read(profilesControllerProvider.notifier);
    switch (entry.actionId) {
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
      default:
        shell.notImplemented(entry.label, entry.actionId);
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
            child: SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: MenuBar(
                children: <Widget>[
                  for (final group in mainMenuModel) _submenu(group),
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
          MenuItemButton(
            key: ValueKey('menu-item-${entry.label}'),
            onPressed: entry.enabled ? () => onAction(entry) : null,
            child: _menuItemLabel(entry),
          ),
        );
      }
    }
    return widgets;
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

/// Minimal runtime controls (T03): start the Xray smoke session and stop it.
class _RuntimeToolbar extends ConsumerWidget {
  const _RuntimeToolbar();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final runtime = ref.watch(runtimeControllerProvider);
    final controller = ref.read(runtimeControllerProvider.notifier);
    return Row(
      children: <Widget>[
        Text(
          '运行时: ${runtime.state}',
          key: const ValueKey('runtime-state-chip'),
          style: const TextStyle(fontSize: 12),
        ),
        const SizedBox(width: 8),
        FilledButton.tonal(
          key: const ValueKey('runtime-start'),
          onPressed: runtime.isBusy ? null : controller.applySmoke,
          child: const Text('启动测试会话', style: TextStyle(fontSize: 12)),
        ),
        const SizedBox(width: 6),
        OutlinedButton(
          key: const ValueKey('runtime-stop'),
          onPressed: controller.stop,
          child: const Text('停止', style: TextStyle(fontSize: 12)),
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
