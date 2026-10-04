import 'dart:async';

import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';

/// A runtime double that counts how many times the shared apply use case ran.
///
/// Used by the RE-PROF-02 / R-01 recheck tests to assert that saving/removing
/// the active node (and only the active node) re-applies the plan, and that the
/// F5 reload goes through the same path.
class CountingRuntimeBridge implements RuntimeBridge {
  CountingRuntimeBridge({
    this.activeId = 'synthetic-active',
    RuntimeView? initial,
    this.applyError,
  }) : _view = initial ?? const RuntimeView();

  String? activeId;
  RuntimeErrorView? applyError;
  int applyCalls = 0;
  int snapshotCalls = 0;
  int stopCalls = 0;

  /// Optional gate awaited at the start of [applyActive] so a test can hold a
  /// reload in flight and exercise re-entrancy (R3-ROOT-01).
  Completer<void>? applyGate;

  RuntimeView _view;

  @override
  Future<RuntimeView> snapshot() async {
    snapshotCalls++;
    return _view;
  }

  @override
  String? activeProfileId() => activeId;

  @override
  Future<RuntimeActionResult> applyActive({
    required BigInt expectedRevision,
  }) async {
    applyCalls++;
    final gate = applyGate;
    if (gate != null) await gate.future;
    if (applyError != null) {
      _view = RuntimeView(error: applyError);
      return RuntimeActionResult(ok: false, error: applyError);
    }
    _view = const RuntimeView(
      state: 'Running',
      hostAlive: true,
      ports: <int>[11808],
      sessionId: 'synthetic-session',
    );
    return const RuntimeActionResult(ok: true, operationId: 'synthetic-op');
  }

  @override
  Future<RuntimeActionResult> stop() async {
    stopCalls++;
    _view = const RuntimeView();
    return const RuntimeActionResult(ok: true);
  }

  @override
  Stream<RuntimeEvent> events() => const Stream<RuntimeEvent>.empty();
}
