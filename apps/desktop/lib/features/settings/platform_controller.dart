import 'dart:convert';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/proxy_settings_view.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

/// Riverpod controller for the T13 system-proxy / PAC / autostart surface.
///
/// The view is only replaced by a real bridge result. Apply/restore never fake
/// success: a failure keeps the structured error and leaves the previous state
/// in place. The status bar reads [PlatformView] directly.
final platformControllerProvider =
    NotifierProvider<PlatformController, PlatformView>(PlatformController.new);

class PlatformController extends Notifier<PlatformView> {
  /// Applied-session key (`sessionId:port`) of the last `UpdateSysProxy`
  /// reconciliation. A plain snapshot refresh of the same session must not
  /// rewrite the host proxy again; only a new applied endpoint does.
  String? _lastAppliedSessionKey;

  /// Identity of the last applied system-proxy state (see [_syncKey]).
  String? _lastAppliedSyncKey;

  /// The controller of the most recently built container, or null. The settings
  /// save path drives the platform stage through [applySavedMode]; reading the
  /// provider from inside the settings notifier would close a dependency cycle
  /// with this controller's settings listener.
  static PlatformController? _active;

  /// Apply the persisted mode for [document] through the active controller.
  /// Returns null when no platform controller has been built (pure storage
  /// tests), so the caller can skip the platform stage honestly.
  static PlatformActionResult? applySavedMode(
    Map<String, dynamic> document, {
    bool silent = true,
  }) {
    final active = _active;
    if (active == null) return null;
    return active.applyModeFromConfig(
      active.desiredModeFromSettings(document),
      document: document,
      silent: silent,
    );
  }

  @override
  PlatformView build() {
    _active = this;
    ref.onDispose(() {
      if (identical(_active, this)) _active = null;
    });
    // Upstream `MainWindowViewModel.LoadCore` -> `SysProxyHandler.UpdateSysProxy`:
    // every successful apply re-points the selected system proxy / PAC at the
    // newly published endpoint. The listener is the production wiring; the
    // status-bar menu and launch restore call the same use case directly.
    ref.listen<RuntimeView>(runtimeControllerProvider, (_, next) {
      _onRuntimeChanged(next);
    });
    // R4-13.S21: upstream `OptionSettingViewModel.SaveSettingAsync` ->
    // `MainWindowViewModel.Reload` -> `LoadCore` -> `UpdateSysProxy` re-points
    // the selected system proxy whenever the persisted `SystemProxyItem`
    // changes, even when the applied session does not. Only reconcile while a
    // real endpoint is applied; a save with no live session keeps the selection
    // for the next launch restore.
    ref.listen<SettingsViewState>(settingsControllerProvider, (prev, next) {
      if (_systemProxyUnchanged(prev?.document, next.document)) return;
      if (!ref.read(runtimeControllerProvider).hasAppliedEndpoint) return;
      syncAppliedMode();
    });
    return const PlatformView();
  }

  /// Whether the persisted `SystemProxyItem` is byte-identical between two
  /// settings documents (drives the settings-save reconciliation above).
  static bool _systemProxyUnchanged(
    Map<String, dynamic>? before,
    Map<String, dynamic> after,
  ) =>
      jsonEncode(before?['SystemProxyItem']) ==
      jsonEncode(after['SystemProxyItem']);

  PlatformBridge get _bridge => ref.read(platformBridgeProvider);

  /// Opt into the real Windows backend. Production calls this once at startup
  /// (release build only); tests and evidence runs keep the default fake, so
  /// no automated path mutates the host proxy or the Run key.
  void enableRealBackend() => _bridge.initBackend('windows');

  /// Read the current state for the given desired mode. A background
  /// read-back (`silent`) must not leave a persistent status message that
  /// shadows later user feedback (import toasts, etc.).
  PlatformView refresh(SysProxyMode desiredMode, {bool silent = false}) {
    final view = _bridge.getSystemProxyState(desiredMode.value);
    state = silent ? view.copyWith(clearMessage: true) : view;
    return view;
  }

  /// The persisted mode is the settings document's `SystemProxyItem.SysProxyType`.
  SysProxyMode desiredModeFromSettings(Map<String, dynamic> document) {
    final item = document['SystemProxyItem'];
    final value = item is Map<String, dynamic>
        ? (item['SysProxyType'] as num?)?.toInt() ?? 2
        : 2;
    return SysProxyMode.fromValue(value);
  }

  /// Apply one of the four modes. `server`/`bypass` come from settings for
  /// `ForcedChange`; `autoConfigUrl` comes from the PAC server for `Pac`.
  PlatformActionResult apply({
    required SysProxyMode mode,
    String? server,
    String? bypass,
    String? autoConfigUrl,
    bool silent = false,
  }) {
    final result = _bridge.setSystemProxy(
      mode: mode.value,
      server: server,
      bypass: bypass,
      autoConfigUrl: autoConfigUrl,
    );
    if (!result.ok) {
      state = state.copyWith(
        error: result.error,
        message: silent ? null : '系统代理应用失败: ${result.error?.code ?? 'unknown'}',
        clearMessage: silent,
      );
      return result;
    }
    final view = _bridge.getSystemProxyState(mode.value);
    state = view.copyWith(
      message: silent ? null : '系统代理: ${mode.label}',
      clearMessage: silent,
    );
    return result;
  }

  /// Start (or refresh) the loopback PAC server. The real port is chosen by the
  /// backend at or above 11808; the forbidden 10808 is never used
  /// (`pac_port_base`). Returns the honest handle.
  PacHandleView startPac({
    required String pacText,
    String? proxyRule,
    int port = 0,
  }) {
    final handle = _bridge.pacStart(
      pacText: pacText,
      proxyRule: proxyRule,
      port: port,
    );
    _applyPac(handle);
    return handle;
  }

  /// Start the PAC server from a custom PAC file path.
  PacHandleView startPacFromFile({
    required String pacPath,
    String? proxyRule,
    int port = 0,
  }) {
    final handle = _bridge.pacStartFromFile(
      pacPath: pacPath,
      proxyRule: proxyRule,
      port: port,
    );
    _applyPac(handle);
    return handle;
  }

  /// Start the PAC server from the persisted configuration, mirroring upstream
  /// `PacManager.InitText`: use the custom PAC path when set, otherwise
  /// `<configDir>/pac.txt`, seeding the bundled default template when the file
  /// is missing. The backend reads and serves the file; only the path is
  /// resolved here.
  PacHandleView startPacFromConfig({
    required String configDir,
    String? customPacPath,
    String? proxyRule,
    int port = 0,
  }) {
    final selection = selectPacFile(
      customPacPath: customPacPath,
      configDir: configDir,
    );
    try {
      final file = File(selection.path);
      if (!file.existsSync()) {
        if (!file.parent.existsSync()) {
          file.parent.createSync(recursive: true);
        }
        file.writeAsStringSync(defaultPacScriptTemplate);
      }
    } on FileSystemException catch (e) {
      final handle = PacHandleView(
        ok: false,
        error: PlatformErrorView(
          code: 'E_IO',
          messageKey: 'error.pac_file_read',
          detail: e.message,
        ),
      );
      _applyPac(handle);
      return handle;
    }
    final handle = _bridge.pacStartFromFile(
      pacPath: selection.path,
      proxyRule: proxyRule,
      port: port,
    );
    _applyPac(handle);
    return handle;
  }

  /// Stop the PAC server (idempotent).
  bool stopPac() {
    final stopped = _bridge.pacStop();
    _applyPac(_bridge.pacState());
    return stopped;
  }

  void _applyPac(PacHandleView handle) {
    state = state.copyWith(
      pacRunning: handle.running,
      pacUrl: handle.url,
      pacPort: handle.port,
      clearPacUrl: handle.url == null,
      clearPacPort: handle.port == null,
      error: handle.error,
    );
  }

  /// Unified system-proxy mode command shared by the status bar, the tray and
  /// the startup restore (upstream `StatusBarViewModel.SetListenerType` +
  /// `ChangeSystemProxyAsync`, and `MainWindowViewModel.LoadCore` ->
  /// `UpdateSysProxy`).
  ///
  /// The proxy/PAC address prefers the actual applied session port carried by
  /// the runtime status; a mode that needs a live proxy reports honestly when
  /// there is no running session instead of pretending it applied. The
  /// `SysProxyType` is persisted in every case so a reopen keeps the selection,
  /// and PAC is served from the real `pac.txt` / custom script via
  /// [startPacFromConfig].
  PlatformActionResult applyModeFromConfig(
    SysProxyMode mode, {
    Map<String, dynamic>? document,
    String? configDir,
    bool silent = false,
  }) {
    final doc = document ?? ref.read(settingsControllerProvider).document;
    // Record the target state *before* applying: persisting the mode updates
    // the settings state synchronously, which re-enters the settings listener
    // while this call is still in flight. Without the pre-registration that
    // listener would apply the same state a second time (R4-13.S21 + R4-24).
    final key = _syncKey(mode, doc);
    _lastAppliedSyncKey = key;
    final result = _applyModeFromConfigInner(
      mode,
      document: doc,
      configDir: configDir,
      silent: silent,
    );
    if (!result.ok && _lastAppliedSyncKey == key) {
      // Failed attempts must not dedupe a later retry.
      _lastAppliedSyncKey = null;
    }
    return result;
  }

  PlatformActionResult _applyModeFromConfigInner(
    SysProxyMode mode, {
    Map<String, dynamic>? document,
    String? configDir,
    bool silent = false,
  }) {
    final doc = document ?? ref.read(settingsControllerProvider).document;
    final config = ProxySettingsView.fromDocument(doc);
    final applied = _appliedProxyInbound(doc);
    switch (mode) {
      case SysProxyMode.forcedChange:
        if (applied == null) {
          return _noRunningSession(mode, doc, silent: silent);
        }
        return _persistAndReturn(
          doc,
          mode,
          apply(
            mode: mode,
            server: buildSystemProxyServer(
              port: applied.port,
              protocol: applied.protocol,
              advancedProtocol: config.advancedProtocol,
            ),
            bypass: buildProxyBypass(
              exceptions: config.exceptions,
              notProxyLocalAddress: config.notProxyLocalAddress,
            ),
            silent: silent,
          ),
        );
      case SysProxyMode.pac:
        if (applied == null) {
          return _noRunningSession(mode, doc, silent: silent);
        }
        final handle = startPacFromConfig(
          configDir: configDir ?? _configDir(),
          customPacPath: config.customPacPath,
          proxyRule: buildPacProxyRule(
            port: applied.port,
            protocol: applied.protocol,
          ),
        );
        if (!handle.ok) {
          _persistMode(doc, mode);
          return PlatformActionResult(ok: false, error: handle.error);
        }
        return _persistAndReturn(
          doc,
          mode,
          apply(mode: mode, autoConfigUrl: state.pacUrl, silent: silent),
        );
      case SysProxyMode.forcedClear:
        stopPac();
        return _persistAndReturn(doc, mode, apply(mode: mode, silent: silent));
      case SysProxyMode.unchanged:
        return _persistAndReturn(doc, mode, apply(mode: mode, silent: silent));
    }
  }

  /// RR-03 startup restore: after the core restore, re-apply the persisted
  /// `SysProxyType` (upstream `MainWindowViewModel.LoadCore` -> `UpdateSysProxy`).
  /// `Unchanged` only reads the backend state back; the other modes reconcile
  /// the system proxy / PAC so a reopen keeps the saved effect.
  void restoreAppliedModeOnLaunch() {
    final document = ref.read(settingsControllerProvider).document;
    final mode = desiredModeFromSettings(document);
    if (mode == SysProxyMode.unchanged) {
      refresh(mode, silent: true);
      return;
    }
    // Background restore: a missing session must not leave a stale status
    // message that would hide later user feedback (import toasts, etc.).
    applyModeFromConfig(mode, document: document, silent: true);
  }

  /// Upstream `LoadCore` -> `UpdateSysProxy` reconciliation. A new applied
  /// session (id or port) re-applies the persisted mode against the actual
  /// endpoint; the same session is a no-op. No applied session clears the
  /// dedupe key so the next real session reconciles again.
  void _onRuntimeChanged(RuntimeView runtime) {
    if (!runtime.hasAppliedEndpoint) {
      _lastAppliedSessionKey = null;
      return;
    }
    final key = '${runtime.sessionId}:${runtime.proxyPort}';
    if (key == _lastAppliedSessionKey) return;
    _lastAppliedSessionKey = key;
    syncAppliedMode(silent: true);
  }

  /// Re-apply the persisted mode against the actual applied endpoint. Shared by
  /// the runtime listener and tests. `Unchanged` stays a no-op, mirroring
  /// upstream `UpdateSysProxy`; the other modes either reconcile the host
  /// proxy/PAC or report the no-running-session fact honestly. Re-applying the
  /// same (mode, session, endpoint) triple is a no-op.
  void syncAppliedMode({bool silent = false}) {
    final document = ref.read(settingsControllerProvider).document;
    final mode = desiredModeFromSettings(document);
    if (mode == SysProxyMode.unchanged) return;
    if (_lastAppliedSyncKey == _syncKey(mode, document)) return;
    applyModeFromConfig(mode, document: document, silent: silent);
  }

  /// Identity of one applied system-proxy state: mode plus the actual applied
  /// session endpoint (never the desired port). Used to dedupe the explicit
  /// command, the settings-save listener and the runtime listener. The protocol
  /// is deliberately not part of the key: an explicit command may carry an
  /// override document whose inbound differs from the provider document while
  /// still describing the same applied endpoint, and a protocol change always
  /// arrives with a new session/port (runtime listener).
  String _syncKey(SysProxyMode mode, Map<String, dynamic> document) {
    final runtime = ref.read(runtimeControllerProvider);
    final applied = _appliedProxyInbound(document);
    return '${mode.value}:${runtime.sessionId ?? '-'}:${applied?.port ?? '-'}';
  }

  /// The actual proxy endpoint published by the running session, if any, with
  /// its protocol. The port is always the applied session port (never the
  /// configured desired port); the protocol is taken from the persisted inbound
  /// only when that inbound's port matches the applied port, otherwise the
  /// default HTTP scheme is assumed instead of mislabeling the listener.
  LocalProxyInbound? _appliedProxyInbound(Map<String, dynamic> document) {
    final runtime = ref.read(runtimeControllerProvider);
    if (!runtime.isRunning) return null;
    final appliedPort = runtime.ports.firstWhere(
      (port) => port > 0,
      orElse: () => 0,
    );
    if (appliedPort <= 0) return null;
    final configured = primaryLocalProxyInbound(document);
    final protocol = (configured != null && configured.port == appliedPort)
        ? configured.protocol
        : ProxyProtocolKind.http;
    return LocalProxyInbound(port: appliedPort, protocol: protocol);
  }

  PlatformActionResult _persistAndReturn(
    Map<String, dynamic> document,
    SysProxyMode mode,
    PlatformActionResult result,
  ) {
    _persistMode(document, mode);
    return result;
  }

  PlatformActionResult _noRunningSession(
    SysProxyMode mode,
    Map<String, dynamic> document, {
    bool silent = false,
  }) {
    _persistMode(document, mode);
    if (silent) return const PlatformActionResult(ok: false);
    state = state.copyWith(
      desiredMode: mode,
      error: const PlatformErrorView(
        code: 'E_NO_RUNNING_SESSION',
        messageKey: 'error.sysproxy_no_running_session',
      ),
      clearMessage: false,
      message: '系统代理未应用：当前没有运行中的代理会话，请先运行节点',
    );
    return const PlatformActionResult(ok: false);
  }

  void _persistMode(Map<String, dynamic> document, SysProxyMode mode) {
    ref
        .read(settingsControllerProvider.notifier)
        .saveGroup('SystemProxyItem', systemProxyItemWithMode(document, mode));
  }

  /// Upstream `Utils.GetConfigPath()`: `<dataDir>/config`.
  String _configDir() {
    final dataDir = ref.read(bridgePortProvider).dataDir();
    if (dataDir.isEmpty) return 'config';
    final last = dataDir[dataDir.length - 1];
    if (last == '/' || last == r'\') return '${dataDir}config';
    return '$dataDir${Platform.pathSeparator}config';
  }

  /// Restore only the fields this process still owns.
  PlatformActionResult restore() {
    final result = _bridge.restoreSystemProxy();
    if (!result.ok) {
      state = state.copyWith(
        error: result.error,
        message: '系统代理恢复失败: ${result.error?.code ?? 'unknown'}',
      );
      return result;
    }
    _applyRestoreMessage(result);
    return result;
  }

  /// Exit-path restore (per-field ownership; user edits preserved).
  PlatformActionResult restoreOnExit(SysProxyMode desiredMode) {
    final result = _bridge.restoreSystemProxyOnExit(desiredMode.value);
    _applyRestoreMessage(result);
    return result;
  }

  void _applyRestoreMessage(PlatformActionResult result) {
    final refreshed = _bridge.getSystemProxyState(state.desiredMode.value);
    if (result.clean) {
      state = refreshed.copyWith(message: '系统代理已恢复');
    } else {
      final fields = result.conflicts.map((c) => c.field).join(', ');
      state = refreshed.copyWith(
        message: '系统代理部分恢复；用户已修改字段保留: $fields',
        conflicts: result.conflicts,
      );
    }
  }

  void setMessage(String? message) {
    state = state.copyWith(message: message, clearMessage: message == null);
  }

  void clearMessage() => state = state.copyWith(clearMessage: true);
}
