import 'dart:async';

import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';

/// Deterministic, in-process bridge for widget tests. It lives under `test/`
/// so no fake Running implementation (hard-coded pid) can ever ship in `lib/`.
/// It never reports a running process unless a test explicitly drives it.
class SyntheticRuntimeBridge implements RuntimeBridge {
  SyntheticRuntimeBridge({RuntimeView? initial, this.failApply = false})
    : _view = initial ?? const RuntimeView();

  RuntimeView _view;

  /// UB2-11 seam: when true, [applyActive] reports failure so tests can
  /// drive the saved-but-unapplied path without a real core.
  final bool failApply;
  final StreamController<RuntimeEvent> _controller =
      StreamController<RuntimeEvent>.broadcast();

  @override
  Future<RuntimeView> snapshot() async => _view;

  @override
  String? activeProfileId() => 'synthetic-active';

  @override
  Future<RuntimeActionResult> applyActive({
    required BigInt expectedRevision,
  }) async {
    if (failApply) {
      return const RuntimeActionResult(
        ok: false,
        operationId: 'op-synthetic-fail',
      );
    }
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
