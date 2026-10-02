import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:system_tray/system_tray.dart';
import 'package:window_manager/window_manager.dart';
import 'package:v2rayn_desktop/app/shell/tray_menu_model.dart';
import 'package:v2rayn_desktop/features/settings/hotkeys.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_controller.dart';
import 'package:v2rayn_desktop/features/settings/proxy_settings_view.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// Close-button semantics from `UiItem` (ACT-WIN-001 / F-DESKTOP-004).
class CloseBehavior {
  const CloseBehavior({
    this.hide2TrayWhenClose = false,
    this.autoHideStartup = false,
  });

  final bool hide2TrayWhenClose;
  final bool autoHideStartup;

  static CloseBehavior fromDocument(Map<String, dynamic> document) {
    final ui = document['UiItem'];
    final map = ui is Map<String, dynamic> ? ui : const <String, dynamic>{};
    return CloseBehavior(
      hide2TrayWhenClose: map['Hide2TrayWhenClose'] == true,
      autoHideStartup: map['AutoHideStartup'] == true,
    );
  }
}

/// The real desktop integration: tray icon + menu, window show/hide, global
/// hotkeys, autostart and exit restore.
///
/// Instantiated only by the release app's bootstrap (never in widget tests), so
/// every plugin call here is a real OS interaction gated behind a user action.
class DesktopIntegration with WindowListener {
  DesktopIntegration(this.ref);

  final WidgetRef ref;
  final SystemTray _tray = SystemTray();
  bool _started = false;

  PlatformController get _platform =>
      ref.read(platformControllerProvider.notifier);

  /// Start the tray, hotkeys, close behavior and initial proxy refresh.
  Future<void> start() async {
    if (_started) return;
    _started = true;
    await windowManager.ensureInitialized();
    // Title + minimum size follow the ledger (INV-WPF-002: title `v2rayN`,
    // 1200x800 default, min width 800). The Win32 runner owns the persisted
    // size, so only the title and floor are applied here.
    try {
      await windowManager.setTitle(AppWindowMetrics.title);
      await windowManager.setMinimumSize(
        const Size(AppWindowMetrics.minWidth, AppWindowMetrics.minHeight),
      );
    } on Object catch (e) {
      debugPrint('[desktop] window metrics failed: $e');
    }
    final settings = ref.read(settingsControllerProvider);
    final behavior = CloseBehavior.fromDocument(settings.document);
    // Close-to-tray: intercept the close button (ACT-WIN-001).
    await windowManager.setPreventClose(behavior.hide2TrayWhenClose);
    windowManager.addListener(this);
    // Select the real Windows backends. This only wires the backend: it does
    // not write the system proxy or the Run key. A write happens exclusively on
    // an explicit user action (mode toggle / autostart switch / exit restore).
    if (Platform.isWindows) {
      _platform.enableRealBackend();
    }
    // Refresh live proxy state from the real backend (a read, never a write).
    final config = ProxySettingsView.fromDocument(settings.document);
    _platform.refresh(config.mode);
    // Global hotkeys (F-DESKTOP-002 / ACT-HOTKEY-001..007).
    ref.read(hotkeyControllerProvider.notifier).loadFromSettings();
    await ref.read(hotkeyControllerProvider.notifier).registerAll();
    await _initTray();
    await _refreshTrayMenu();
  }

  Future<void> _initTray() async {
    try {
      await _tray.initSystemTray(
        title: 'v2rayN-R',
        iconPath: _trayIconPath(),
        toolTip: 'v2rayN-R',
      );
      _tray.registerSystemTrayEventHandler((eventName) {
        switch (eventName) {
          // ACT-TRAY-001: a single left click toggles the window
          // (upstream `NotifyLeftClickCmd` / `NoLeftClickDelay`).
          case 'leftMouseUp':
            _toggleWindow();
          // Right-click opens the context menu.
          case 'rightMouseUp':
            _tray.popUpContextMenu();
        }
      });
    } on Object catch (e) {
      debugPrint('[desktop] tray init failed: $e');
    }
  }

  String _trayIconPath() {
    final exeDir = File(Platform.resolvedExecutable).parent.path;
    return '$exeDir${Platform.pathSeparator}data'
        '${Platform.pathSeparator}flutter_assets'
        '${Platform.pathSeparator}assets'
        '${Platform.pathSeparator}tray_icon.ico';
  }

  Future<void> _toggleWindow() async {
    if (await windowManager.isVisible()) {
      await windowManager.hide();
    } else {
      await windowManager.show();
      await windowManager.focus();
    }
  }

  /// ACT-WIN-002: the 关闭 menu entry hides the window to the tray (the same
  /// semantics as the window close button when `Hide2TrayWhenClose` is on).
  Future<void> hideToTray() async {
    await windowManager.hide();
  }

  /// Rebuild the tray context menu from the ledger model. Dynamic submenus
  /// (routing/nodes) are added from the current controllers.
  Future<void> _refreshTrayMenu() async {
    final platform = ref.read(platformControllerProvider);
    final model = trayMenuModel(
      currentMode: platform.desiredMode,
      pacVisible: Platform.isWindows,
    );
    final items = <MenuItemBase>[];
    for (final entry in model) {
      if (entry.separatorBefore) items.add(MenuSeparator());
      switch (entry.kind) {
        case TrayItemKind.routingSubmenu:
        case TrayItemKind.serversSubmenu:
          // Inline combos become submenus; their contents are populated by the
          // respective controllers in follow-up wiring.
          items.add(SubMenu(label: entry.label, children: <MenuItemBase>[]));
        case TrayItemKind.item:
          items.add(
            MenuItem(
              label: entry.checked ? '✓ ${entry.label}' : entry.label,
              onClicked: () => _onTrayAction(entry),
            ),
          );
      }
    }
    await _tray.setContextMenu(items);
  }

  void _onTrayAction(TrayMenuItem entry) {
    final controller = _platform;
    final mode = entry.radioGroup;
    if (mode != null) {
      final settings = ref.read(settingsControllerProvider).document;
      final config = ProxySettingsView.fromDocument(settings);
      switch (mode) {
        case SysProxyMode.forcedChange:
          controller.apply(
            mode: mode,
            server: buildProxyServer(
              port: _firstInboundPort(settings),
              advancedProtocol: config.advancedProtocol,
            ),
            bypass: buildProxyBypass(
              exceptions: config.exceptions,
              notProxyLocalAddress: config.notProxyLocalAddress,
            ),
          );
        case SysProxyMode.pac:
          controller.setMessage('PAC 模式需要先启动 PAC 服务');
        case SysProxyMode.forcedClear:
        case SysProxyMode.unchanged:
          controller.apply(mode: mode);
      }
      _refreshTrayMenu();
      return;
    }
    switch (entry.actionId) {
      case 'ACT-TRAY-013':
        exitApp();
      case 'ACT-TRAY-001':
        _toggleWindow();
      default:
        // Actions whose backend lands in later tasks report honestly.
        controller.setMessage('${entry.label}（${entry.actionId}）尚未接入后端');
    }
  }

  int _firstInboundPort(Map<String, dynamic> document) {
    final inbound = document['Inbound'];
    if (inbound is List && inbound.isNotEmpty) {
      final first = inbound.first;
      if (first is Map<String, dynamic>) {
        return (first['LocalPort'] as num?)?.toInt() ?? 10808;
      }
    }
    return 10808;
  }

  /// Window listener: intercept the close button and hide to tray instead
  /// (ACT-WIN-001 / upstream `MainWindow_Closing` `e.Cancel = true`).
  @override
  void onWindowClose() async {
    final behavior = CloseBehavior.fromDocument(
      ref.read(settingsControllerProvider).document,
    );
    if (behavior.hide2TrayWhenClose) {
      await windowManager.hide();
    } else {
      await windowManager.destroy();
    }
  }

  /// Exit path (ACT-TRAY-013 / ACT-WIN-002): restore the system proxy per field,
  /// then quit.
  Future<void> exitApp() async {
    final mode = ref.read(platformControllerProvider).desiredMode;
    _platform.restoreOnExit(mode);
    await ref.read(hotkeyControllerProvider.notifier).unregisterAll();
    await windowManager.destroy();
  }

  /// Detach the window listener on teardown.
  void removeListener() {
    windowManager.removeListener(this);
  }

  /// Toggle autostart (`GuiItem.AutoRun`). The real write happens only on this
  /// explicit user action.
  bool setAutostart(bool enabled, String exePath) {
    final name = ref.read(platformBridgeProvider).autostartValueName(exePath);
    return ref
        .read(platformBridgeProvider)
        .setAutostart(name: name, enabled: enabled, exe: exePath, args: '');
  }
}

/// Mutable holder for the live integration instance. It stays null in widget
/// tests; the real app bootstrap overwrites [value]. Menu entries that need a
/// real OS action read it and fall back to an honest status message when the
/// holder is empty.
class DesktopIntegrationHolder {
  DesktopIntegration? value;
}

final desktopIntegrationProvider = Provider<DesktopIntegrationHolder>(
  (ref) => DesktopIntegrationHolder(),
);
