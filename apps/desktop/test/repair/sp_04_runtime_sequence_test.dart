import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';

/// SP-04: one authoritative command sequence.
///
/// Scriptable bridge: calls are logged in execution order, apply/stop mutate
/// a backend view, and gates let the test interleave A-stop-B exactly like a
/// user clicking during an in-flight start. Only synthetic ids/ports/revisions.
class SequenceBridge
    implements
        RuntimeBridge,
        ExplicitTargetRuntimeBridge,
        OperationQueryBridge {
  SequenceBridge({this.snapshotView = const RuntimeView()});

  final StreamController<RuntimeEvent> _events =
      StreamController<RuntimeEvent>.broadcast();

  final List<String> calls = [];
  RuntimeView snapshotView;

  /// Blocks the first apply until the test releases it (A in flight).
  Completer<void>? applyGate;

  /// Error returned by apply (ok=false) even when the backend did run.
  RuntimeErrorView? applyError;

  /// When true the backend view becomes Running before the apply result
  /// returns (timeout-with-effect fault model).
  bool applySetsRunning = true;

  /// When set, apply throws instead of returning (transport ACK loss model).
  Object? applyThrow;

  /// Blocks the next snapshot until released (stale-read model).
  Completer<RuntimeView>? snapshotGate;
  int snapshotCalls = 0;

  /// Read-only operation queries observed (SP-04: unknown outcomes query
  /// first, never re-execute).
  final List<String> operationQueries = [];

  void emit(RuntimeEvent event) => _events.add(event);
  Future<void> get done => _events.close();

  @override
  String? activeProfileId() => 'synthetic-node';

  @override
  BigInt desiredRevision() => BigInt.one;

  @override
  Stream<RuntimeEvent> events() => _events.stream;

  @override
  Future<RuntimeView> snapshot() async {
    snapshotCalls++;
    final gate = snapshotGate;
    if (gate != null) return gate.future;
    return snapshotView;
  }

  @override
  Future<RuntimeActionResult> applyActive({required BigInt expectedRevision}) =>
      applyTarget(
        targetId: 'synthetic-node',
        expectedRevision: expectedRevision,
      );

  @override
  Future<RuntimeActionResult> applyTarget({
    required String targetId,
    required BigInt expectedRevision,
  }) async {
    final first = calls.isEmpty;
    calls.add('apply:$targetId');
    if (first) {
      final gate = applyGate;
      if (gate != null) await gate.future;
    }
    final thrown = applyThrow;
    // The backend effect lands before the reply is lost: mutate first, then
    // throw, so reconciliation must observe the applied truth.
    if (applySetsRunning) {
      snapshotView = RuntimeView(
        state: 'Running',
        hostAlive: true,
        ports: const [11810],
        sessionId: 'synthetic-$targetId',
      );
    }
    if (thrown != null) throw thrown;
    final error = applyError;
    return RuntimeActionResult(
      ok: error == null,
      operationId: 'synthetic-op-$targetId',
      error: error,
    );
  }

  @override
  Future<RuntimeActionResult> stop() async {
    calls.add('stop');
    snapshotView = const RuntimeView();
    return const RuntimeActionResult(ok: true);
  }

  @override
  Future<RuntimeOperationView?> operationStatus(String operationId) async {
    operationQueries.add(operationId);
    return null;
  }
}

ProviderContainer _containerFor(SequenceBridge bridge) {
  final container = ProviderContainer(
    overrides: [runtimeBridgeProvider.overrideWithValue(bridge)],
  );
  addTearDown(container.dispose);
  addTearDown(() => bridge.done);
  return container;
}

Future<void> _tick() => Future<void>.delayed(Duration.zero);

void main() {
  test(
    'SP-04: A in flight, stop, then B executes stop before B and ends on B',
    () async {
      final bridge = SequenceBridge()..applyGate = Completer<void>();
      final container = _containerFor(bridge);
      final controller = container.read(runtimeControllerProvider.notifier);
      await controller.start();

      final a = controller.applyActive(targetId: 'a');
      await _tick();
      final s = controller.stop();
      final b = controller.applyActive(targetId: 'b');
      bridge.applyGate!.complete();
      await Future.wait([a, s, b]);

      expect(bridge.calls, [
        'apply:a',
        'stop',
        'apply:b',
      ], reason: 'stop is a barrier: it must run before the later start');
      final view = container.read(runtimeControllerProvider);
      expect(view.isRunning, isTrue);
      expect(view.sessionId, 'synthetic-b');
    },
  );

  test('SP-04: A then B then stop ends stopped', () async {
    final bridge = SequenceBridge();
    final container = _containerFor(bridge);
    final controller = container.read(runtimeControllerProvider.notifier);
    await controller.start();

    await controller.applyActive(targetId: 'a');
    final b = controller.applyActive(targetId: 'b');
    final s = controller.stop();
    await Future.wait([b, s]);

    expect(bridge.calls, ['apply:a', 'apply:b', 'stop']);
    expect(container.read(runtimeControllerProvider).isRunning, isFalse);
  });

  test(
    'SP-04: queued applies merge to the latest target without a barrier',
    () async {
      final bridge = SequenceBridge()..applyGate = Completer<void>();
      final container = _containerFor(bridge);
      final controller = container.read(runtimeControllerProvider.notifier);
      await controller.start();

      final a = controller.applyActive(targetId: 'a');
      await _tick();
      final b = controller.applyActive(targetId: 'b');
      final c = controller.applyActive(targetId: 'c');
      bridge.applyGate!.complete();
      final results = await Future.wait([a, b, c]);

      expect(bridge.calls, ['apply:a', 'apply:c']);
      expect(results, everyElement(isTrue));
      expect(
        container.read(runtimeControllerProvider).sessionId,
        'synthetic-c',
      );
    },
  );

  test(
    'SP-04: unknown apply outcome reconciles from truth instead of replaying',
    () async {
      final bridge = SequenceBridge()
        ..applyError = const RuntimeErrorView(
          code: 'E_TIMEOUT',
          messageKey: 'error.runtime_timeout',
        )
        ..applySetsRunning = true;
      final container = _containerFor(bridge);
      final controller = container.read(runtimeControllerProvider.notifier);
      await controller.start();
      final snapshotsBefore = bridge.snapshotCalls;

      await controller.applyActive(targetId: 'b');

      // Exactly one submission: reconcile queries, it never re-executes.
      expect(bridge.calls, ['apply:b']);
      expect(bridge.operationQueries, ['synthetic-op-b']);
      expect(bridge.snapshotCalls, greaterThan(snapshotsBefore));
      final view = container.read(runtimeControllerProvider);
      expect(
        view.isRunning,
        isTrue,
        reason: 'timeout does not prove the backend apply failed',
      );
      expect(view.reconcileNeeded, isTrue);
      expect(view.error?.code, 'E_TIMEOUT');
    },
  );

  test(
    'SP-04: lost transport ACK reconciles from truth instead of replaying',
    () async {
      final bridge = SequenceBridge()
        ..applyThrow = StateError('synthetic pipe disconnect')
        ..applySetsRunning = true;
      final container = _containerFor(bridge);
      final controller = container.read(runtimeControllerProvider.notifier);
      await controller.start();

      await controller.applyActive(targetId: 'b');

      expect(bridge.calls, ['apply:b']);
      final view = container.read(runtimeControllerProvider);
      expect(view.isRunning, isTrue);
      expect(view.reconcileNeeded, isTrue);
    },
  );

  test(
    'SP-04: stale snapshot after an epoch change never overwrites',
    () async {
      final bridge = SequenceBridge();
      final container = _containerFor(bridge);
      final controller = container.read(runtimeControllerProvider.notifier);
      await controller.start();

      bridge.snapshotView = const RuntimeView(
        state: 'Running',
        hostAlive: true,
        ports: [11810],
        sessionId: 'synthetic-new',
      );
      final gate = Completer<RuntimeView>();
      bridge.snapshotGate = gate;
      final refresh = controller.refresh();
      await _tick();
      // Session moved underneath us while idle: the in-flight read is stale.
      bridge.emit(
        RuntimeEvent(
          kind: 'x',
          payloadJson: '{}',
          epoch: BigInt.one,
          seq: BigInt.one,
        ),
      );
      bridge.emit(
        RuntimeEvent(
          kind: 'x',
          payloadJson: '{}',
          epoch: BigInt.two,
          seq: BigInt.one,
        ),
      );
      bridge.snapshotGate = null;
      gate.complete(bridge.snapshotView);
      await refresh;

      final view = container.read(runtimeControllerProvider);
      // Every pre-epoch read is dropped, never applied; a follow-up re-read
      // converges on the fresh truth. (Broadcast delivery can interleave, so
      // more than one in-flight read may be dropped.)
      expect(view.staleResponsesDropped, greaterThanOrEqualTo(1));
      expect(view.sessionId, 'synthetic-new');
    },
  );

  test('SP-04: cancelling queued commands never executes them', () async {
    final bridge = SequenceBridge()..applyGate = Completer<void>();
    final container = _containerFor(bridge);
    final controller = container.read(runtimeControllerProvider.notifier);
    await controller.start();

    final a = controller.applyActive(targetId: 'a');
    await _tick();
    final b = controller.applyActive(targetId: 'b');
    final dropped = await controller.cancelPending();
    bridge.applyGate!.complete();
    final results = await Future.wait([a, b]);

    expect(dropped, 1);
    expect(results.first, isTrue);
    expect(results.last, isFalse);
    expect(bridge.calls, ['apply:a']);
    expect(container.read(runtimeControllerProvider).sessionId, 'synthetic-a');
  });
}
