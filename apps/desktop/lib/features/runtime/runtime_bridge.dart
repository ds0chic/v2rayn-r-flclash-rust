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
    this.epoch,
    this.lastSeq,
    this.sequenceWarning,
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

  /// Last event epoch observed by the controller (event stream only; the
  /// snapshot does not carry it). Used to detect a reconnecting/restarted host.
  final BigInt? epoch;

  /// Last event sequence observed on [epoch].
  final BigInt? lastSeq;

  /// Set when an event arrived out of order or with a gap; the UI logs it and
  /// never silently replays. A full reconnect/replay is a T09+ item.
  final String? sequenceWarning;

  /// `desired/applied` revision pair, rendered as `rev: d/a`.
  String get revisionLabel =>
      'rev: ${desiredRevision ?? '-'}/${appliedRevision ?? '-'}';

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

  RuntimeView copyWith({
    RuntimeErrorView? error,
    bool clearError = false,
    BigInt? epoch,
    BigInt? lastSeq,
    String? sequenceWarning,
  }) {
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
      epoch: epoch ?? this.epoch,
      lastSeq: lastSeq ?? this.lastSeq,
      sequenceWarning: sequenceWarning ?? this.sequenceWarning,
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

/// A raw runtime event (kind + JSON payload + stream position).
class RuntimeEvent {
  const RuntimeEvent({
    required this.kind,
    required this.payloadJson,
    this.epoch,
    this.seq,
  });

  final String kind;
  final String payloadJson;

  /// Stream position; `null` for synthetic test events that bypass the host.
  final BigInt? epoch;
  final BigInt? seq;
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
    (e) => RuntimeEvent(
      kind: e.kind,
      payloadJson: e.payloadJson,
      epoch: e.epoch,
      seq: e.seq,
    ),
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

final runtimeBridgeProvider = Provider<RuntimeBridge>(
  (ref) => const FrbRuntimeBridge(),
);
