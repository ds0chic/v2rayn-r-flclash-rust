import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/proxy_settings_view.dart';

/// Riverpod controller for the T13 system-proxy / PAC / autostart surface.
///
/// The view is only replaced by a real bridge result. Apply/restore never fake
/// success: a failure keeps the structured error and leaves the previous state
/// in place. The status bar reads [PlatformView] directly.
final platformControllerProvider =
    NotifierProvider<PlatformController, PlatformView>(PlatformController.new);

class PlatformController extends Notifier<PlatformView> {
  @override
  PlatformView build() => const PlatformView();

  PlatformBridge get _bridge => ref.read(platformBridgeProvider);

  /// Opt into the real Windows backend. Production calls this once at startup
  /// (release build only); tests and evidence runs keep the default fake, so
  /// no automated path mutates the host proxy or the Run key.
  void enableRealBackend() => _bridge.initBackend('windows');

  /// Read the current state for the given desired mode.
  PlatformView refresh(SysProxyMode desiredMode) {
    final view = _bridge.getSystemProxyState(desiredMode.value);
    state = view;
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
        message: '系统代理应用失败: ${result.error?.code ?? 'unknown'}',
      );
      return result;
    }
    final view = _bridge.getSystemProxyState(mode.value);
    state = view.copyWith(message: '系统代理: ${mode.label}');
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
  void stopPac() {
    _bridge.pacStop();
    _applyPac(_bridge.pacState());
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
