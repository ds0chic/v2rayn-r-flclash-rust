// R4-04 contract: command queue, queue-inclusive deadline, refresh merge and
// stale-response eviction.
//
// Synthetic only: no native library, no network, no port use and no user data.
import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';

/// A bridge whose snapshot/apply/stop can each be held open so a test can prove
/// queueing, coalescing and stale-response handling deterministically.
class _GatedRuntimeBridge
    implements RuntimeBridge, ExplicitTargetRuntimeBridge {
  String? activeId = 'node-a';
  RuntimeView view = const RuntimeView();
  Completer<void>? snapshotGate;
  Completer<void>? applyGate;
  RuntimeErrorView? applyError;
  RuntimeErrorView? stopError;

  int snapshotCalls = 0;
  int applyCalls = 0;
  int stopCalls = 0;
  String? lastTargetId;

  @override
  Future<RuntimeView> snapshot() async {
    snapshotCalls++;
    final gate = snapshotGate;
    if (gate != null) await gate.future;
    return view;
  }

  @override
  String? activeProfileId() => activeId;

  @override
  Future<RuntimeActionResult> applyActive({required BigInt expectedRevision}) =>
      _apply(null);

  @override
  Future<RuntimeActionResult> applyTarget({
    required String targetId,
    required BigInt expectedRevision,
  }) => _apply(targetId);

  Future<RuntimeActionResult> _apply(String? targetId) async {
    applyCalls++;
    lastTargetId = targetId;
    final gate = applyGate;
    if (gate != null) await gate.future;
    final error = applyError;
    if (error != null) {
      view = RuntimeView(error: error);
      return RuntimeActionResult(ok: false, error: error);
    }
    view = const RuntimeView(
      state: 'Running',
      hostAlive: true,
      ports: <int>[11808],
      sessionId: 'r4-04-session',
    );
    return const RuntimeActionResult(ok: true, operationId: 'r4-04-op');
  }

  @override
  Future<RuntimeActionResult> stop() async {
    stopCalls++;
    final error = stopError;
    if (error != null) {
      return RuntimeActionResult(ok: false, error: error);
    }
    view = const RuntimeView();
    return const RuntimeActionResult(ok: true);
  }

  @override
  Stream<RuntimeEvent> events() => const Stream<RuntimeEvent>.empty();

  @override
  BigInt desiredRevision() => BigInt.one;
}

({ProviderContainer container, RuntimeController controller}) _makeR4Container(
  _GatedRuntimeBridge runtime,
) {
  final container = ProviderContainer(
    overrides: [runtimeBridgeProvider.overrideWithValue(runtime)],
  );
  addTearDown(container.dispose);
  return (
    container: container,
    controller: container.read(runtimeControllerProvider.notifier),
  );
}

void main() {
  test(
    'local pending is visible before the backend reports anything',
    () async {
      final runtime = _GatedRuntimeBridge();
      runtime.applyGate = Completer<void>();
      final made = _makeR4Container(runtime);

      final pending = made.controller.applyActive(targetId: 'node-b');
      // Synchronous read, before the gated apply can complete: the UI shows a
      // command is in flight and it never claims a running session.
      final during = made.container.read(runtimeControllerProvider);
      expect(during.commandPending, isTrue);
      expect(during.pendingCommands, 1);
      expect(during.isRunning, isFalse);
      expect(during.state, 'Stopped');

      runtime.applyGate!.complete();
      await pending;
      final after = made.container.read(runtimeControllerProvider);
      expect(after.commandPending, isFalse);
      expect(after.isRunning, isTrue);
      expect(after.sessionId, 'r4-04-session');
    },
  );

  test(
    'the command queue is bounded and coalesces to the latest target',
    () async {
      final runtime = _GatedRuntimeBridge();
      runtime.applyGate = Completer<void>();
      final made = _makeR4Container(runtime);

      final first = made.controller.applyActive(targetId: 'node-a');
      final second = made.controller.applyActive(targetId: 'node-b');
      final third = made.controller.applyActive(targetId: 'node-c');
      // Let the pump reach the gated first apply before inspecting the queue.
      await Future<void>.delayed(Duration.zero);
      expect(runtime.applyCalls, 1, reason: 'only one command runs at a time');
      final queued = made.container.read(runtimeControllerProvider);
      expect(queued.pendingCommands, 2, reason: 'one active + one coalesced');
      expect(queued.commandPending, isTrue);

      runtime.applyGate!.complete();
      await Future.wait(<Future<void>>[first, second, third]);
      expect(
        runtime.applyCalls,
        2,
        reason: 'two coalesced applies map to one call',
      );
      expect(runtime.lastTargetId, 'node-c', reason: 'latest intent wins');
      expect(made.container.read(runtimeControllerProvider).pendingCommands, 0);
    },
  );

  test(
    'a command that waited past the deadline is rejected, not run late',
    () async {
      final runtime = _GatedRuntimeBridge();
      runtime.applyGate = Completer<void>();
      final made = _makeR4Container(runtime);
      var now = DateTime(2026, 1, 1);
      made.controller.clock = () => now;
      made.controller.commandDeadline = const Duration(seconds: 30);

      final first = made.controller.applyActive(targetId: 'node-a');
      final stale = made.controller.applyActive(targetId: 'node-b');
      now = now.add(const Duration(seconds: 31));

      runtime.applyGate!.complete();
      await Future.wait(<Future<void>>[first, stale]);

      expect(
        runtime.applyCalls,
        1,
        reason: 'the stale queued command never runs',
      );
      final view = made.container.read(runtimeControllerProvider);
      expect(view.error?.code, 'E_TIMEOUT');
      expect(view.pendingCommands, 0);
    },
  );

  test('concurrent refreshes are merged, not stacked', () async {
    final runtime = _GatedRuntimeBridge();
    final gate = Completer<void>();
    runtime.snapshotGate = gate;
    final made = _makeR4Container(runtime);

    final a = made.controller.refresh();
    final b = made.controller.refresh();
    final c = made.controller.refresh();
    expect(runtime.snapshotCalls, 1, reason: 'merged onto one in-flight read');

    runtime.view = const RuntimeView(state: 'Running', hostAlive: true);
    runtime.snapshotGate = null;
    gate.complete();
    await Future.wait(<Future<void>>[a, b, c]);
    expect(
      runtime.snapshotCalls,
      2,
      reason: 'one rerun for the coalesced pending read, never a storm',
    );
    expect(made.container.read(runtimeControllerProvider).isRunning, isTrue);
  });

  test('a snapshot overtaken by a command generation is dropped', () async {
    final runtime = _GatedRuntimeBridge();
    final gate = Completer<void>();
    runtime.snapshotGate = gate;
    final made = _makeR4Container(runtime);

    final staleRefresh = made.controller.refresh();
    runtime.applyGate = Completer<void>();
    final apply = made.controller.applyActive(targetId: 'node-b');
    // Pump runs to its generation bump and merged refresh before we resume.
    await Future<void>.delayed(Duration.zero);

    runtime.view = const RuntimeView(state: 'Running', sessionId: 'new');
    runtime.snapshotGate = null;
    runtime.applyGate!.complete();
    gate.complete();
    await staleRefresh;
    await apply;

    final view = made.container.read(runtimeControllerProvider);
    expect(view.staleResponsesDropped, greaterThan(0));
    expect(view.isRunning, isTrue);
  });

  test(
    'a stop with an unknown outcome reconciles instead of claiming success',
    () async {
      final runtime = _GatedRuntimeBridge();
      final made = _makeR4Container(runtime);
      runtime.stopError = const RuntimeErrorView(
        code: 'E_TIMEOUT',
        messageKey: 'error.runtime_timeout',
      );

      await made.controller.stop();
      final view = made.container.read(runtimeControllerProvider);
      expect(view.error?.code, 'E_TIMEOUT');
      expect(view.reconcileNeeded, isTrue);
      expect(
        runtime.snapshotCalls,
        greaterThan(0),
        reason: 'reconciled from truth',
      );
    },
  );
}
