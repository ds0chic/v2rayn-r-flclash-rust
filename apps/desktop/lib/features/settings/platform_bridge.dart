import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as contract;
import 'package:v2rayn_desktop/bridge/api/platform.dart' as rust;

/// Structured platform error surfaced to the UI (mirrors `ErrorDto`).
class PlatformErrorView {
  const PlatformErrorView({
    required this.code,
    required this.messageKey,
    this.detail,
  });

  final String code;
  final String messageKey;
  final String? detail;

  @override
  String toString() =>
      detail == null ? '$code ($messageKey)' : '$code ($messageKey): $detail';
}

/// The four `ESysProxyType` modes, in upstream order.
enum SysProxyMode {
  forcedClear(0, '清除系统代理'),
  forcedChange(1, '自动配置系统代理'),
  unchanged(2, '不改变系统代理'),
  pac(3, 'Pac 模式');

  const SysProxyMode(this.value, this.label);

  final int value;
  final String label;

  static SysProxyMode fromValue(int value) => SysProxyMode.values.firstWhere(
    (m) => m.value == value,
    orElse: () => SysProxyMode.unchanged,
  );
}

/// One externally-modified proxy field.
class ProxyConflictView {
  const ProxyConflictView({
    required this.field,
    this.ownedValue,
    this.currentValue,
  });

  final String field;
  final String? ownedValue;
  final String? currentValue;
}

/// Per-field ownership classification.
class ProxyOwnershipView {
  const ProxyOwnershipView({required this.field, required this.ownership});

  final String field;
  final String ownership;
}

/// Live system-proxy read model assembled only from the bridge.
class PlatformView {
  const PlatformView({
    this.loaded = false,
    this.desiredMode = SysProxyMode.unchanged,
    this.enabled = false,
    this.server,
    this.bypass,
    this.autoConfigUrl,
    this.autoDetect = false,
    this.hasOwnership = false,
    this.pacRunning = false,
    this.pacUrl,
    this.pacPort,
    this.conflicts = const <ProxyConflictView>[],
    this.ownership = const <ProxyOwnershipView>[],
    this.error,
    this.message,
  });

  final bool loaded;
  final SysProxyMode desiredMode;
  final bool enabled;
  final String? server;
  final String? bypass;
  final String? autoConfigUrl;
  final bool autoDetect;
  final bool hasOwnership;
  final bool pacRunning;
  final String? pacUrl;
  final int? pacPort;
  final List<ProxyConflictView> conflicts;
  final List<ProxyOwnershipView> ownership;
  final PlatformErrorView? error;

  /// Last user-facing feedback (apply/restore result).
  final String? message;

  PlatformView copyWith({
    bool? loaded,
    SysProxyMode? desiredMode,
    bool? enabled,
    String? server,
    String? bypass,
    String? autoConfigUrl,
    bool? autoDetect,
    bool? hasOwnership,
    bool? pacRunning,
    String? pacUrl,
    int? pacPort,
    List<ProxyConflictView>? conflicts,
    List<ProxyOwnershipView>? ownership,
    PlatformErrorView? error,
    bool clearError = false,
    String? message,
    bool clearMessage = false,
    bool clearServer = false,
    bool clearBypass = false,
    bool clearAutoConfigUrl = false,
  }) {
    return PlatformView(
      loaded: loaded ?? this.loaded,
      desiredMode: desiredMode ?? this.desiredMode,
      enabled: enabled ?? this.enabled,
      server: clearServer ? null : (server ?? this.server),
      bypass: clearBypass ? null : (bypass ?? this.bypass),
      autoConfigUrl: clearAutoConfigUrl
          ? null
          : (autoConfigUrl ?? this.autoConfigUrl),
      autoDetect: autoDetect ?? this.autoDetect,
      hasOwnership: hasOwnership ?? this.hasOwnership,
      pacRunning: pacRunning ?? this.pacRunning,
      pacUrl: pacUrl ?? this.pacUrl,
      pacPort: pacPort ?? this.pacPort,
      conflicts: conflicts ?? this.conflicts,
      ownership: ownership ?? this.ownership,
      error: clearError ? null : (error ?? this.error),
      message: clearMessage ? null : (message ?? this.message),
    );
  }

  /// Human label for the current proxy state, e.g. `已启用 127.0.0.1:10809`.
  String get stateLabel {
    if (pacRunning && autoConfigUrl != null) {
      return 'PAC ${pacUrl ?? autoConfigUrl}';
    }
    if (enabled && server != null && server!.isNotEmpty) {
      return '已启用 $server';
    }
    if (autoConfigUrl != null && autoConfigUrl!.isNotEmpty) {
      return 'PAC $autoConfigUrl';
    }
    return '未启用';
  }
}

/// Outcome of a proxy apply/restore command.
class PlatformActionResult {
  const PlatformActionResult({
    required this.ok,
    this.clean = true,
    this.error,
    this.conflicts = const <ProxyConflictView>[],
  });

  final bool ok;
  final bool clean;
  final PlatformErrorView? error;
  final List<ProxyConflictView> conflicts;
}

/// Thin, testable seam over the generated platform bridge so widget tests can
/// avoid loading the native library.
abstract class PlatformBridge {
  PlatformView getSystemProxyState(int desiredMode);

  PlatformActionResult setSystemProxy({
    required int mode,
    String? server,
    String? bypass,
    String? autoConfigUrl,
  });

  PlatformActionResult restoreSystemProxy();

  PlatformActionResult restoreSystemProxyOnExit(int desiredMode);

  bool getAutostart(String name);

  bool setAutostart({
    required String name,
    required bool enabled,
    required String exe,
    required String args,
  });

  String autostartValueName(String startupPath);

  bool validateCustomProxyScript({String? pacPath, String? scriptPath});

  bool resolveUwpLoopbackTool({String? binDir});

  /// Optional real-backend opt-in; a no-op for the synthetic bridge.
  void initBackend(String backend);
}

class FrbPlatformBridge implements PlatformBridge {
  const FrbPlatformBridge();

  @override
  PlatformView getSystemProxyState(int desiredMode) {
    return _toView(rust.getSystemProxyState(desiredMode: desiredMode));
  }

  @override
  PlatformActionResult setSystemProxy({
    required int mode,
    String? server,
    String? bypass,
    String? autoConfigUrl,
  }) {
    final result = rust.setSystemProxy(
      mode: mode,
      server: server,
      bypass: bypass,
      autoConfigUrl: autoConfigUrl,
    );
    return PlatformActionResult(ok: result.ok, error: _error(result.error));
  }

  @override
  PlatformActionResult restoreSystemProxy() {
    final result = rust.restoreSystemProxy();
    return _restore(result);
  }

  @override
  PlatformActionResult restoreSystemProxyOnExit(int desiredMode) {
    final result = rust.restoreSystemProxyOnExit(desiredMode: desiredMode);
    return _restore(result);
  }

  @override
  bool getAutostart(String name) => rust.getAutostart(name: name);

  @override
  bool setAutostart({
    required String name,
    required bool enabled,
    required String exe,
    required String args,
  }) {
    final result = rust.setAutostart(
      name: name,
      enabled: enabled,
      exe: exe,
      args: args,
    );
    return result.ok;
  }

  @override
  String autostartValueName(String startupPath) =>
      rust.autostartValueName(startupPath: startupPath);

  @override
  bool validateCustomProxyScript({String? pacPath, String? scriptPath}) {
    final result = rust.validateCustomProxyScript(
      pacPath: pacPath,
      scriptPath: scriptPath,
    );
    return result.ok;
  }

  @override
  bool resolveUwpLoopbackTool({String? binDir}) {
    final result = rust.resolveUwpLoopbackTool(binDir: binDir);
    return result.ok;
  }

  @override
  void initBackend(String backend) =>
      rust.initPlatformBackend(backend: backend);

  PlatformActionResult _restore(rust.SysProxyRestoreResult result) {
    return PlatformActionResult(
      ok: result.ok,
      clean: result.clean,
      error: _error(result.error),
      conflicts: result.conflicts
          .map(
            (c) => ProxyConflictView(
              field: c.field,
              ownedValue: c.ownedValue,
              currentValue: c.currentValue,
            ),
          )
          .toList(),
    );
  }

  PlatformView _toView(rust.ProxyStateDto dto) {
    return PlatformView(
      loaded: dto.ok,
      desiredMode: SysProxyMode.fromValue(dto.desiredMode),
      enabled: dto.enabled,
      server: dto.server,
      bypass: dto.bypass,
      autoConfigUrl: dto.autoConfigUrl,
      autoDetect: dto.autoDetect,
      hasOwnership: dto.hasOwnership,
      pacRunning: dto.pacRunning,
      pacUrl: dto.pacUrl,
      pacPort: dto.pacPort,
      conflicts: dto.conflicts.map((f) => ProxyConflictView(field: f)).toList(),
      ownership: dto.ownership
          .map(
            (o) => ProxyOwnershipView(field: o.field, ownership: o.ownership),
          )
          .toList(),
      error: _error(dto.error),
    );
  }

  PlatformErrorView? _error(contract.ErrorDto? e) {
    if (e == null) return null;
    return PlatformErrorView(
      code: e.code,
      messageKey: e.messageKey,
      detail: e.detail,
    );
  }
}

final platformBridgeProvider = Provider<PlatformBridge>(
  (ref) => const FrbPlatformBridge(),
);
