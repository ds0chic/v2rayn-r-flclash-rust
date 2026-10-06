import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';

/// SP-17 red contract: switching A -> B with a failed B must keep the actual
/// A view (state/session/ports), surface the structured failure, remember the
/// failed target for retry, and never present desired state as applied.
class Sp17Bridge implements RuntimeBridge, ExplicitTargetRuntimeBridge {
  Sp17Bridge(this.view);

  RuntimeView view;
  String? lastTarget;
  RuntimeErrorView? nextError;
  String? nextOperationId;
  int applies = 0;

  @override
  String? activeProfileId() => 'synthetic-a';

  @override
  BigInt desiredRevision() => BigInt.from(7);

  @override
  Future<RuntimeView> snapshot() async => view;

  @override
  Stream<RuntimeEvent> events() => const Stream.empty();

  @override
  Future<RuntimeActionResult> applyActive({required BigInt expectedRevision}) =>
      applyTarget(targetId: 'synthetic-a', expectedRevision: expectedRevision);

  @override
  Future<RuntimeActionResult> applyTarget({
    required String targetId,
    required BigInt expectedRevision,
  }) async {
    applies++;
    lastTarget = targetId;
    final error = nextError;
    if (error != null) {
      return RuntimeActionResult(
        ok: false,
        operationId: nextOperationId,
        error: error,
      );
    }
    view = RuntimeView(
      state: 'Running',
      hostAlive: true,
      ports: const <int>[11911],
      sessionId: 'sess-$targetId',
      desiredRevision: BigInt.from(7),
      appliedRevision: BigInt.from(7),
    );
    return RuntimeActionResult(ok: true, operationId: 'op-$targetId');
  }

  @override
  Future<RuntimeActionResult> stop() async {
    view = const RuntimeView();
    return const RuntimeActionResult(ok: true);
  }
}

ProviderContainer _containerFor(Sp17Bridge bridge) {
  final container = ProviderContainer(
    overrides: [runtimeBridgeProvider.overrideWithValue(bridge)],
  );
  addTearDown(container.dispose);
  return container;
}

RuntimeView _runningA() => RuntimeView(
  state: 'Running',
  hostAlive: true,
  ports: const <int>[11911],
  sessionId: 'sess-a',
  desiredRevision: BigInt.from(7),
  appliedRevision: BigInt.from(7),
);

void main() {
  test('failed switch to B keeps actual A and records B for retry', () async {
    final bridge = Sp17Bridge(_runningA())
      ..nextError = const RuntimeErrorView(
        code: 'E_CORE_START_FAILED',
        messageKey: 'error.core_start_failed',
      )
      ..nextOperationId = 'op-b-1';
    final container = _containerFor(bridge);
    final controller = container.read(runtimeControllerProvider.notifier);
    await controller.start();

    final ok = await controller.applyActive(targetId: 'synthetic-b');

    expect(ok, isFalse);
    final view = container.read(runtimeControllerProvider);
    // Actual A is retained: backend still runs A, the failed B never applies.
    expect(view.isRunning, isTrue);
    expect(view.ports, <int>[11911]);
    expect(view.sessionId, 'sess-a');
    expect(view.hasAppliedEndpoint, isTrue);
    // The failure is visible with its operation identity, not swallowed.
    expect(view.error?.code, 'E_CORE_START_FAILED');
    expect(view.failedTargetId, 'synthetic-b');
    expect(view.failureOperationId, 'op-b-1');
    expect(view.canRetryFailed, isTrue);
    // Desired is never presented as applied.
    expect(view.hasUnappliedChanges, isFalse);
    expect(view.revisionLabel, isNot('rev: -/-'));
  });

  test(
    'retryFailed re-attempts the failed target and clears on success',
    () async {
      final bridge = Sp17Bridge(_runningA())
        ..nextError = const RuntimeErrorView(
          code: 'E_CORE_START_FAILED',
          messageKey: 'error.core_start_failed',
        )
        ..nextOperationId = 'op-b-1';
      final container = _containerFor(bridge);
      final controller = container.read(runtimeControllerProvider.notifier);
      await controller.start();
      await controller.applyActive(targetId: 'synthetic-b');

      bridge.nextError = null;
      bridge.nextOperationId = null;
      final ok = await controller.retryFailed();

      expect(ok, isTrue);
      expect(bridge.applies, 2);
      expect(bridge.lastTarget, 'synthetic-b');
      final view = container.read(runtimeControllerProvider);
      expect(view.sessionId, 'sess-synthetic-b');
      expect(view.error, isNull);
      expect(view.canRetryFailed, isFalse);
    },
  );

  test('retryFailed without a failure is a no-op', () async {
    final bridge = Sp17Bridge(_runningA());
    final container = _containerFor(bridge);
    final controller = container.read(runtimeControllerProvider.notifier);
    await controller.start();

    expect(await controller.retryFailed(), isFalse);
    expect(bridge.applies, 0);
  });
}
