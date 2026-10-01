import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as rust;
import 'package:v2rayn_desktop/bridge/api/engine.dart' as rust;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart' as rust;

/// Structured error surfaced to the status bar. Mirrors `ErrorDto`.
class RuntimeErrorView {
  const RuntimeErrorView({
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

/// A read model of the runtime, assembled only from snapshot/events.
class RuntimeView {
  const RuntimeView({
    this.state = 'Stopped',
    this.hostAlive = false,
    this.pid,
    this.ports = const <int>[],
    this.sessionId,
    this.configSha256,
    this.operationId,
    this.error,
    this.desiredRevision,
    this.appliedRevision,
  });

  final String state;
  final bool hostAlive;
  final int? pid;
  final List<int> ports;
  final String? sessionId;
  final String? configSha256;
  final String? operationId;
  final RuntimeErrorView? error;
  final BigInt? desiredRevision;
  final BigInt? appliedRevision;

  bool get isRunning => state == 'Running';
  bool get isBusy =>
      state == 'Validating' ||
      state == 'Preparing' ||
      state == 'Starting' ||
      state == 'Checking' ||
      state == 'RollingBack';

  /// Never fabricates a running label: unknown or stopped reads as 未运行.
  String get statusLabel {
    if (isRunning) {
      final pidText = pid == null ? '' : ' PID=$pid';
      final portText = ports.isEmpty ? '' : ' 端口=${ports.join(',')}';
      return '运行中$pidText$portText';
    }
    return '未运行';
  }

  RuntimeView copyWith({RuntimeErrorView? error, bool clearError = false}) {
    return RuntimeView(
      state: state,
      hostAlive: hostAlive,
      pid: pid,
      ports: ports,
      sessionId: sessionId,
      configSha256: configSha256,
      operationId: operationId,
      error: clearError ? null : (error ?? this.error),
      desiredRevision: desiredRevision,
      appliedRevision: appliedRevision,
    );
  }
}

/// Outcome of an apply/stop command.
class RuntimeActionResult {
  const RuntimeActionResult({required this.ok, this.operationId, this.error});

  final bool ok;
  final String? operationId;
  final RuntimeErrorView? error;
}

/// A raw runtime event (kind + JSON payload).
class RuntimeEvent {
  const RuntimeEvent({required this.kind, required this.payloadJson});

  final String kind;
  final String payloadJson;
}

/// Thin, testable seam over the generated bridge so widget tests can avoid
/// loading the native library.
abstract class RuntimeBridge {
  Future<RuntimeView> snapshot();

  Future<RuntimeActionResult> applySmoke({required BigInt expectedRevision});

  Future<RuntimeActionResult> stop();

  Stream<RuntimeEvent> events();
}

class FrbRuntimeBridge implements RuntimeBridge {
  const FrbRuntimeBridge();

  @override
  Future<RuntimeView> snapshot() async {
    final snap = await rust.getSnapshot();
    return _toView(snap);
  }

  @override
  Future<RuntimeActionResult> applySmoke({
    required BigInt expectedRevision,
  }) async {
    final result = await rust.applyRuntime(
      targetId: 'smoke',
      expectedRevision: expectedRevision,
    );
    return RuntimeActionResult(
      ok: result.ok,
      operationId: result.operationId,
      error: _error(result.error),
    );
  }

  @override
  Future<RuntimeActionResult> stop() async {
    final result = await rust.stopRuntime();
    return RuntimeActionResult(ok: result.ok, error: _error(result.error));
  }

  @override
  Stream<RuntimeEvent> events() => rust.subscribeEvents().map(
    (e) => RuntimeEvent(kind: e.kind, payloadJson: e.payloadJson),
  );

  /// Canonical state label matching the Rust `RuntimeState` variant names.
  static String stateLabel(rust.RuntimeState state) {
    switch (state) {
      case rust.RuntimeState.stopped:
        return 'Stopped';
      case rust.RuntimeState.validating:
        return 'Validating';
      case rust.RuntimeState.preparing:
        return 'Preparing';
      case rust.RuntimeState.starting:
        return 'Starting';
      case rust.RuntimeState.checking:
        return 'Checking';
      case rust.RuntimeState.running:
        return 'Running';
      case rust.RuntimeState.rollingBack:
        return 'RollingBack';
      case rust.RuntimeState.degraded:
        return 'Degraded';
    }
  }

  RuntimeView _toView(rust.SnapshotDto snap) {
    return RuntimeView(
      state: stateLabel(snap.runtimeState),
      hostAlive: snap.hostAlive,
      pid: snap.runtimePid,
      ports: snap.runtimePorts.toList(),
      sessionId: snap.runtimeSessionId,
      configSha256: snap.runtimeConfigSha256,
      operationId: snap.runtimeOperationId,
      error: _error(snap.runtimeError),
      desiredRevision: snap.desiredRevision,
      appliedRevision: snap.appliedRevision,
    );
  }

  RuntimeErrorView? _error(rust.ErrorDto? e) {
    if (e == null) return null;
    return RuntimeErrorView(
      code: e.code,
      messageKey: e.messageKey,
      detail: e.detail,
    );
  }
}

/// Deterministic, in-process bridge for widget tests. It never reports a
/// running process unless a test explicitly drives it.
class SyntheticRuntimeBridge implements RuntimeBridge {
  SyntheticRuntimeBridge({RuntimeView? initial})
    : _view = initial ?? const RuntimeView();

  RuntimeView _view;
  final StreamController<RuntimeEvent> _controller =
      StreamController<RuntimeEvent>.broadcast();

  @override
  Future<RuntimeView> snapshot() async => _view;

  @override
  Future<RuntimeActionResult> applySmoke({
    required BigInt expectedRevision,
  }) async {
    _view = RuntimeView(
      state: 'Running',
      hostAlive: true,
      pid: 4242,
      ports: const <int>[11808],
      sessionId: 'synthetic',
      desiredRevision: expectedRevision,
      appliedRevision: expectedRevision,
    );
    return const RuntimeActionResult(ok: true, operationId: 'op-synthetic');
  }

  @override
  Future<RuntimeActionResult> stop() async {
    _view = const RuntimeView();
    return const RuntimeActionResult(ok: true);
  }

  @override
  Stream<RuntimeEvent> events() => _controller.stream;
}

final runtimeBridgeProvider = Provider<RuntimeBridge>(
  (ref) => const FrbRuntimeBridge(),
);
