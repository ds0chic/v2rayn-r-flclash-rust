import 'dart:async';

import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';

/// Deterministic, in-process bridge for widget tests. It lives under `test/`
/// so no fake Running implementation (hard-coded pid) can ever ship in `lib/`.
/// It never reports a running process unless a test explicitly drives it.
class SyntheticRuntimeBridge implements RuntimeBridge {
  SyntheticRuntimeBridge({RuntimeView? initial})
    : _view = initial ?? const RuntimeView();

  RuntimeView _view;
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
