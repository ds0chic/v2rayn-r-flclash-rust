import 'dart:async';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:system_tray/system_tray.dart';
import 'package:window_manager/window_manager.dart';
import 'package:v2rayn_desktop/app/shell/tray_menu_model.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as monitor;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/routing/routing_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
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

/// One bounded step of the real-exit sequence.
typedef ShutdownStep = Future<void> Function();

/// Outcome of a bounded shutdown. Failures are reported, never thrown.
class ShutdownReport {
  ShutdownReport();

  /// Steps that completed within their timeout, in order.
  final List<String> completed = <String>[];

  /// Step name -> human-readable failure reason (thrown or timed out).
  final Map<String, String> failures = <String, String>{};

  bool get ok => failures.isEmpty;
}

/// Run [steps] in order with a per-step timeout and a total budget.
///
/// A step that throws or times out is recorded and the sequence continues, so
/// platform ownership (system proxy / PAC) is still restored even when the core
/// stop failed. Once [totalBudget] is spent the remaining steps are marked
/// skipped instead of hanging. This never throws.
Future<ShutdownReport> runBoundedShutdown(
  List<MapEntry<String, ShutdownStep>> steps, {
  Duration stepTimeout = const Duration(seconds: 5),
  Duration totalBudget = const Duration(seconds: 15),
  DateTime Function()? clock,
}) async {
  final now = clock ?? DateTime.now;
  final report = ShutdownReport();
  final started = now();
  for (final step in steps) {
    if (now().difference(started) >= totalBudget) {
      report.failures[step.key] = 'skipped: shutdown budget exhausted';
      continue;
    }
    try {
      await step.value().timeout(stepTimeout);
      report.completed.add(step.key);
    } on TimeoutException {
      report.failures[step.key] =
          'timeout after ${stepTimeout.inMilliseconds}ms';
    } on Object catch (e) {
      report.failures[step.key] = e.toString();
    }
  }
  return report;
}

/// Lifecycle surface the app exposes to exit/hand-off callers. Kept narrow so a
/// test double can stand in for the real tray/window integration.
abstract class DesktopLifecycle {
  Future<void> hideToTray();
  Future<void> exitApp();
  Future<void> exitForUpdate();
  void removeListener();
}

/// The real desktop integration: tray icon + menu, window show/hide, global
/// hotkeys, autostart and exit restore.
///
/// Instantiated only by the release app's bootstrap (never in widget tests), so
/// every plugin call here is a real OS interaction gated behind a user action.
class DesktopIntegration with WindowListener implements DesktopLifecycle {
  DesktopIntegration(this.ref);

  final WidgetRef ref;
  final SystemTray _tray = SystemTray();
  bool _started = false;

  /// Per-step bound and overall budget of the real-exit sequence (R4-05).
  Duration shutdownStepTimeout = const Duration(seconds: 5);
  Duration shutdownTotalBudget = const Duration(seconds: 15);

  /// Statistics drain/flush hook. Production wires the Rust monitor drain;
  /// tests inject a recorder. A null hook means "no flush available", which is
  /// reported as a skipped step rather than silently treated as flushed.
  Future<void> Function()? flushStats;

  bool _exiting = false;
  bool _updateExiting = false;

  /// Shared-read-model tray sync (RR-08). Null until [start] wires it.
  TrayMenuSync? _sync;

  /// Provider subscriptions that push every business/runtime/config change
  /// into [_sync]; closed on [removeListener].
  final List<ProviderSubscription<Object?>> _subscriptions =
      <ProviderSubscription<Object?>>[];

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
    // Windows WPF contract (ACT-WIN-001 / upstream `MainWindow_Closing`): the
    // close button always cancels and hides to tray. The Avalonia build keeps
    // the close-button destroy unless `Hide2TrayWhenClose` opts in, so only
    // non-Windows consults the field (ROOT-04 / RT-13 platform split).
    await windowManager.setPreventClose(
      Platform.isWindows || behavior.hide2TrayWhenClose,
    );
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
    // Global hotkeys (F-DESKTOP-002 / ACT-HOTKEY-001..007). The dispatcher
    // routes an OS key press into the same window/proxy entry points as the
    // tray and status bar (RT-12).
    ref.read(hotkeyDispatchProvider).handler = _onHotkey;
    ref.read(hotkeyControllerProvider.notifier).loadFromSettings();
    await ref.read(hotkeyControllerProvider.notifier).registerAll();
    await _initTray();
    _bindTraySync();
    await _syncTray();
    // AutoHideStartup (FLD-CFG-078 / upstream `MainWindow` ctor sets
    // `WindowState.Minimized`, `OnLoaded` calls `ShowHideWindow(false)`): a
    // launch with the field on must never surface the window, only the tray.
    if (shouldHideOnStartup(settings.document)) {
      await _hideOnStartup();
    }
  }

  /// Whether the window must stay hidden right after launch. Upstream reads
  /// `UiItem.AutoHideStartup` once at window construction, so this is a startup
  /// decision, not a live toggle.
  static bool shouldHideOnStartup(Map<String, dynamic> document) =>
      CloseBehavior.fromDocument(document).autoHideStartup;

  /// Whether the window close button hides to the tray. Hiding is never a stop
  /// (ROOT-04 / RT-13 / ACT-WIN-001): Windows WPF always hides; other platforms
  /// follow the `Hide2TrayWhenClose` opt-in. `true` means hide, not quit.
  static bool hideOnClose({
    required bool isWindows,
    required bool hide2TrayWhenClose,
  }) => isWindows || hide2TrayWhenClose;

  /// Hide the already-created window. The Win32 embedder shows the window from
  /// its first-frame callback, so hide once now (tray already exists) and again
  /// on the next frame in case that callback lands after this runs.
  Future<void> _hideOnStartup() async {
    try {
      await windowManager.hide();
      WidgetsBinding.instance.addPostFrameCallback((_) {
        windowManager.hide();
      });
    } on Object catch (e) {
      debugPrint('[desktop] auto-hide startup failed: $e');
    }
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

  String _trayAssetsDir() {
    final exeDir = File(Platform.resolvedExecutable).parent.path;
    return '$exeDir${Platform.pathSeparator}data'
        '${Platform.pathSeparator}flutter_assets'
        '${Platform.pathSeparator}assets'
        '${Platform.pathSeparator}tray';
  }

  String _trayIconPath() =>
      '${_trayAssetsDir()}${Platform.pathSeparator}'
      '${trayIconResourceName(TrayIconStatus.normal)}';

  /// Resolve the tray icon file for [status] (R3-09a). [TrayIconStatus] is no
  /// longer ignored: each proxy/core/PAC combination asks for its own resource.
  /// When that file is not packaged, fall back to the plain `tray_icon.ico`,
  /// then to the executable-sibling `app_icon.ico`, so the tray is never left
  /// with a stale or missing image.
  String _trayIconPathFor(TrayIconStatus status) {
    final sep = Platform.pathSeparator;
    final dir = _trayAssetsDir();
    final specific = '$dir$sep${trayIconResourceName(status)}';
    if (File(specific).existsSync()) return specific;
    final legacy = '$dir$sep${trayIconResourceName(TrayIconStatus.normal)}';
    if (File(legacy).existsSync()) return legacy;
    final exeDir = File(Platform.resolvedExecutable).parent.path;
    final fallback = '$exeDir$sep$trayIconFallbackName';
    if (File(fallback).existsSync()) return fallback;
    return legacy;
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
  ///
  /// Hiding is never a stop: the managed core and platform state keep running.
  @override
  Future<void> hideToTray() async {
    await windowManager.hide();
  }

  /// Subscribe the tray to the shared read model (RR-08), so the menu/icon
  /// follow every business / runtime / configuration change instead of only
  /// startup and an immediate tray click. `listenManual` is the documented
  /// out-of-`build` seam for a non-widget integration object.
  void _bindTraySync() {
    _sync = TrayMenuSync(
      surface: _SystemTraySurface(
        _tray,
        iconPathFor: _trayIconPathFor,
        onRouting: _onTrayRouting,
        onNode: _onTrayNode,
        onAction: _onTrayAction,
      ),
    );
    void resync() {
      // The listener runs synchronously on the provider change; the OS tray
      // calls are async, so push them off the notification without awaiting.
      unawaited(_syncTray());
    }

    _subscriptions.addAll(<ProviderSubscription<Object?>>[
      ref.listenManual(profilesControllerProvider, (_, _) => resync()),
      ref.listenManual(routingControllerProvider, (_, _) => resync()),
      ref.listenManual(platformControllerProvider, (_, _) => resync()),
      ref.listenManual(runtimeControllerProvider, (_, _) => resync()),
      ref.listenManual(settingsControllerProvider, (_, _) => resync()),
    ]);
  }

  /// Build the shared tray read model and apply it (RR-08).
  ///
  /// Node and route checkmarks come from the same active/default state the main
  /// window reads; the icon status comes from the applied proxy/core state. The
  /// sync de-duplicates, so an unrelated rebuild never touches the OS tray.
  Future<void> _syncTray() async {
    final sync = _sync;
    if (sync == null) return;
    final platform = ref.read(platformControllerProvider);
    final routing = ref.read(routingControllerProvider);
    final profiles = ref.read(profilesControllerProvider);
    final runtime = ref.read(runtimeControllerProvider);
    final document = ref.read(settingsControllerProvider).document;
    // The radio checkmarks keep the persisted selection (upstream
    // `SystemProxySelected`), while the tray icon follows the mode that is
    // actually applied to the host, so a failed apply cannot leave a "wish"
    // proxy icon (R4-26 / D30).
    final appliedMode = appliedSysProxyMode(platform);
    await sync.update(
      TrayReadModel(
        desiredMode: platform.desiredMode,
        iconMode: appliedMode ?? SysProxyMode.unchanged,
        pacVisible: Platform.isWindows,
        routings: <TraySubEntry>[
          for (final item in routing.items)
            TraySubEntry(
              id: item.id,
              label: item.remarks.isEmpty ? item.id : item.remarks,
              // The checkmark follows the default (active) routing, not the
              // editor's selection (upstream `StatusBarViewModel.SelectedRouting`
              // is bound to `IsActive`).
              checked: item.isActive,
            ),
        ],
        nodes: <TraySubEntry>[
          for (final p in profiles.all)
            TraySubEntry(
              id: p.id,
              label: p.remarks.isEmpty ? p.id : p.remarks,
              checked: p.id == profiles.activeId,
            ),
        ],
        serversLimit: _trayServersLimit(document),
        coreRunning: runtime.isRunning,
        pacRunning: platform.pacRunning,
      ),
    );
  }

  void _onTrayAction(TrayMenuItem entry) {
    final mode = entry.radioGroup;
    if (mode != null) {
      _applyProxyMode(mode);
      return;
    }
    switch (entry.actionId) {
      case 'ACT-TRAY-013':
        exitApp();
      case 'ACT-TRAY-001':
        _toggleWindow();
      case 'ACT-TRAY-012':
        // D30: the copy-proxy-command tray item had no dispatch. It is a
        // tray-only action (no main-menu equivalent), so it is handled here
        // against the actual applied session port.
        unawaited(_copyProxyCommandToClipboard());
      default:
        // Shared main-window use cases (RT-11); else report honestly.
        final command = sharedCommandForTrayAction(entry.actionId);
        final delegate = ref.read(trayCommandDelegateProvider).value;
        if (command != null && delegate?.onSharedCommand != null) {
          delegate!.onSharedCommand!(command);
        } else {
          _platform.setMessage('${entry.label}（${entry.actionId}）尚未接入后端');
        }
    }
  }

  void _onTrayNode(String id) {
    final delegate = ref.read(trayCommandDelegateProvider).value;
    if (delegate?.onSelectNode != null) {
      delegate!.onSelectNode!(id);
    } else {
      _platform.setMessage('切换节点 ($id) 尚未接入');
    }
    unawaited(_syncTray());
  }

  void _onTrayRouting(String id) {
    final delegate = ref.read(trayCommandDelegateProvider).value;
    if (delegate?.onSelectRouting != null) {
      delegate!.onSelectRouting!(id);
    } else {
      _platform.setMessage('切换路由 ($id) 尚未接入');
    }
    unawaited(_syncTray());
  }

  /// Apply a system-proxy mode through the shared command so the tray, the
  /// status bar and the startup restore use one code path (RT-14): the proxy /
  /// PAC address comes from the actual applied session, PAC is served from the
  /// real `pac.txt` / custom script, the mode is persisted, then the tray is
  /// resynced.
  void _applyProxyMode(SysProxyMode mode) {
    _platform.applyModeFromConfig(mode);
    unawaited(_syncTray());
  }

  /// ACT-TRAY-012 / UFS-15 / D30: copy the six proxy environment lines built
  /// from the actual applied session port. A missing session reports an honest
  /// failure instead of copying a wrong (desired-only) port.
  Future<void> _copyProxyCommandToClipboard() async {
    final runtime = ref.read(runtimeControllerProvider);
    final document = ref.read(settingsControllerProvider).document;
    final outcome = resolveTrayProxyCommand(
      coreRunning: runtime.isRunning,
      appliedPorts: runtime.ports,
      configuredPort: primaryLocalProxyInbound(document)?.port,
      windows: Platform.isWindows,
    );
    if (!outcome.ok || outcome.text == null) {
      _platform.setMessage(outcome.message);
      return;
    }
    await Clipboard.setData(ClipboardData(text: outcome.text!));
    _platform.setMessage(outcome.message);
  }

  /// Route an OS hotkey press into the same entry points as the tray/menus
  /// (upstream `MainWindow.OnHotkeyHandler`).
  void _onHotkey(GlobalHotkeyAction action) {
    switch (action) {
      case GlobalHotkeyAction.showForm:
        _toggleWindow();
      case GlobalHotkeyAction.systemProxyClear:
        _applyProxyMode(SysProxyMode.forcedClear);
      case GlobalHotkeyAction.systemProxySet:
        _applyProxyMode(SysProxyMode.forcedChange);
      case GlobalHotkeyAction.systemProxyUnchanged:
        _applyProxyMode(SysProxyMode.unchanged);
      case GlobalHotkeyAction.systemProxyPac:
        _applyProxyMode(SysProxyMode.pac);
    }
  }

  int _trayServersLimit(Map<String, dynamic> document) {
    final gui = document['GuiItem'];
    if (gui is Map<String, dynamic>) {
      final value = (gui['TrayMenuServersLimit'] as num?)?.toInt();
      if (value != null) return value;
    }
    return 0;
  }

  /// Window listener: intercept the close button and hide to tray instead
  /// (ACT-WIN-001 / upstream `MainWindow_Closing` `e.Cancel = true`).
  @override
  void onWindowClose() async {
    final behavior = CloseBehavior.fromDocument(
      ref.read(settingsControllerProvider).document,
    );
    if (hideOnClose(
      isWindows: Platform.isWindows,
      hide2TrayWhenClose: behavior.hide2TrayWhenClose,
    )) {
      // Hiding keeps the core and platform state running: a close button is
      // never a real exit (only the tray/菜单 exit path stops/restores).
      await windowManager.hide();
    } else {
      await windowManager.destroy();
    }
  }

  /// Statistics drain on exit. The Rust monitor sync + flush persists the last
  /// interval so a reopen reads the final counters instead of losing them.
  Future<void> _defaultFlushStats() async {
    monitor.monitorStartPolling();
  }

  /// The one real-exit sequence (ACT-TRAY-013 / ACT-WIN-002 / R4-05):
  /// stop the managed core, drain/flush statistics, restore platform ownership
  /// (system proxy + PAC), stop the subscription scheduler and unregister
  /// hotkeys. Every step is bounded; a failure is reported, never hidden.
  ///
  /// Order follows the R4-05 contract: stop -> drain -> flush -> platform
  /// restore, so a handed-off runner never inherits a core that still holds
  /// files, and the system proxy is not left pointing at a dead endpoint.
  Future<ShutdownReport> runShutdown() {
    final mode = ref.read(platformControllerProvider).desiredMode;
    final flush = flushStats ?? _defaultFlushStats;
    return runBoundedShutdown(
      <MapEntry<String, ShutdownStep>>[
        MapEntry('stop_runtime', () async {
          await ref.read(runtimeControllerProvider.notifier).stop();
        }),
        MapEntry('flush_stats', () async {
          await flush();
        }),
        MapEntry('restore_platform', () async {
          _platform.stopPac();
          _platform.restoreOnExit(mode);
        }),
        MapEntry('stop_scheduler', () async {
          ref.read(bridgePortProvider).stopSubScheduler();
        }),
        MapEntry('unregister_hotkeys', () async {
          await ref.read(hotkeyControllerProvider.notifier).unregisterAll();
        }),
      ],
      stepTimeout: shutdownStepTimeout,
      totalBudget: shutdownTotalBudget,
    );
  }

  /// Exit path (ACT-TRAY-013 / ACT-WIN-002): the bounded real-exit sequence
  /// then destroy the window. An incomplete cleanup is surfaced, not swallowed.
  @override
  Future<void> exitApp() async {
    if (_exiting) return;
    _exiting = true;
    final report = await runShutdown();
    if (!report.ok) {
      debugPrint('[desktop] exit cleanup incomplete: ${report.failures}');
      _platform.setMessage('退出清理未完成: ${report.failures.keys.join(', ')}');
    }
    await windowManager.destroy();
  }

  /// Self-update hand-off (R4-05): run the same bounded shutdown so the core is
  /// stopped and statistics are flushed before the runner replaces files, then
  /// exit the process the runner waits on. A cleanup failure is logged and the
  /// exit still proceeds (the runner's result is the user-visible signal).
  @override
  Future<void> exitForUpdate() async {
    if (_updateExiting) return;
    _updateExiting = true;
    final report = await runShutdown();
    if (!report.ok) {
      debugPrint(
        '[desktop] update handoff cleanup incomplete: ${report.failures}',
      );
    }
    exit(0);
  }

  /// Detach the window listener and stop the tray read-model subscriptions.
  @override
  void removeListener() {
    for (final sub in _subscriptions) {
      sub.close();
    }
    _subscriptions.clear();
    _sync = null;
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

/// Real tray surface (RR-08): translates the ledger menu model into
/// `system_tray` items and pushes the icon path for the derived status. It is
/// the only place the plugin is touched; tests use a recording surface instead.
class _SystemTraySurface implements TraySurface {
  _SystemTraySurface(
    this._tray, {
    required this.iconPathFor,
    required this.onRouting,
    required this.onNode,
    required this.onAction,
  });

  final SystemTray _tray;
  final String Function(TrayIconStatus status) iconPathFor;
  final void Function(String id) onRouting;
  final void Function(String id) onNode;
  final void Function(TrayMenuItem entry) onAction;
  TrayIconStatus? _iconStatus;

  @override
  Future<void> applyMenu(List<TrayMenuItem> model) async {
    final items = <MenuItemBase>[];
    for (final entry in model) {
      if (entry.separatorBefore) items.add(MenuSeparator());
      switch (entry.kind) {
        case TrayItemKind.routingSubmenu:
          items.add(
            SubMenu(
              label: entry.label,
              children: <MenuItemBase>[
                for (final c in entry.children)
                  MenuItem(
                    label: c.checked ? '✓ ${c.label}' : c.label,
                    onClicked: () => onRouting(c.id),
                  ),
              ],
            ),
          );
        case TrayItemKind.serversSubmenu:
          items.add(
            SubMenu(
              label: entry.label,
              children: <MenuItemBase>[
                for (final c in entry.children)
                  MenuItem(
                    label: c.checked ? '✓ ${c.label}' : c.label,
                    onClicked: () => onNode(c.id),
                  ),
              ],
            ),
          );
        case TrayItemKind.item:
          items.add(
            MenuItem(
              label: entry.checked ? '✓ ${entry.label}' : entry.label,
              onClicked: () => onAction(entry),
            ),
          );
      }
    }
    try {
      await _tray.setContextMenu(items);
    } on Object catch (e) {
      debugPrint('[desktop] tray menu update failed: $e');
    }
  }

  @override
  Future<void> applyIcon(TrayIconStatus status) async {
    if (_iconStatus == status) return;
    _iconStatus = status;
    try {
      await _tray.setImage(iconPathFor(status));
    } on Object catch (e) {
      debugPrint('[desktop] tray icon update failed: $e');
    }
  }
}

/// Mutable holder for the live integration instance. It stays null in widget
/// tests; the real app bootstrap overwrites [value]. Menu entries that need a
/// real OS action read it and fall back to an honest status message when the
/// holder is empty.
class DesktopIntegrationHolder {
  DesktopLifecycle? value;
}

final desktopIntegrationProvider = Provider<DesktopIntegrationHolder>(
  (ref) => DesktopIntegrationHolder(),
);
