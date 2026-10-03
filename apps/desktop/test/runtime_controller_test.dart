import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';

/// Minimal bridge that lets the test drive the event stream directly and
/// script the snapshot/apply outcomes (FIX-07).
class _FakeBridge implements RuntimeBridge {
  _FakeBridge({
    this.snapshotView = const RuntimeView(),
    this.activeId,
    this.applyResult,
  });

  final StreamController<RuntimeEvent> _controller =
      StreamController<RuntimeEvent>.broadcast();

  RuntimeView snapshotView;
  String? activeId;
  RuntimeActionResult? applyResult;
  BigInt? lastAppliedRevision;
  int snapshotCalls = 0;
  int applyCalls = 0;

  void emit(RuntimeEvent event) => _controller.add(event);

  @override
  Future<RuntimeView> snapshot() async {
    snapshotCalls++;
    return snapshotView;
  }

  @override
  String? activeProfileId() => activeId;

  @override
  Future<RuntimeActionResult> applyActive({
    required BigInt expectedRevision,
  }) async {
    applyCalls++;
    lastAppliedRevision = expectedRevision;
    return applyResult ?? const RuntimeActionResult(ok: true);
  }

  @override
  Future<RuntimeActionResult> stop() async =>
      const RuntimeActionResult(ok: true);

  @override
  Stream<RuntimeEvent> events() => _controller.stream;
}

ProviderContainer _containerFor(_FakeBridge bridge) {
  final container = ProviderContainer(
    overrides: [runtimeBridgeProvider.overrideWithValue(bridge)],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  test('revision label renders desired/applied without fabricating values', () {
    final view = RuntimeView(
      desiredRevision: BigInt.from(4),
      appliedRevision: BigInt.from(2),
    );
    expect(view.revisionLabel, 'rev: 4/2');
    expect(const RuntimeView().revisionLabel, 'rev: -/-');
  });

  test('applied endpoint is only exposed for a running session', () {
    const stopped = RuntimeView(ports: [11808], sessionId: 's-1');
    expect(stopped.hasAppliedEndpoint, isFalse);
    const running = RuntimeView(
      state: 'Running',
      ports: [11810],
      sessionId: 's-1',
    );
    expect(running.proxyPort, 11810);
    expect(running.hasAppliedEndpoint, isTrue);
    // Desired settings port without a session is never an applied endpoint.
    const noSession = RuntimeView(state: 'Running', ports: [11808]);
    expect(noSession.hasAppliedEndpoint, isFalse);
  });

  test('controller records epoch/seq and flags out-of-order events', () async {
    final bridge = _FakeBridge();
    addTearDown(bridge._controller.close);
    final container = _containerFor(bridge);

    final controller = container.read(runtimeControllerProvider.notifier);
    await controller.start();

    bridge.emit(
      RuntimeEvent(
        kind: 'runtime_detail',
        payloadJson: '{}',
        epoch: BigInt.one,
        seq: BigInt.from(5),
      ),
    );
    await Future<void>.delayed(Duration.zero);
    expect(container.read(runtimeControllerProvider).lastSeq, BigInt.from(5));

    // Same epoch, seq goes backwards: a gap/reorder, never silently replayed.
    bridge.emit(
      RuntimeEvent(
        kind: 'runtime_detail',
        payloadJson: '{}',
        epoch: BigInt.one,
        seq: BigInt.from(4),
      ),
    );
    await Future<void>.delayed(Duration.zero);
    expect(
      container.read(runtimeControllerProvider).sequenceWarning,
      isNotNull,
    );
  });

  test(
    'applyActive re-reads the latest desired revision before applying',
    () async {
      final bridge = _FakeBridge(
        snapshotView: RuntimeView(
          desiredRevision: BigInt.from(7),
          appliedRevision: BigInt.from(3),
        ),
      );
      addTearDown(bridge._controller.close);
      final container = _containerFor(bridge);
      final controller = container.read(runtimeControllerProvider.notifier);
      await controller.start();

      await controller.applyActive();

      expect(bridge.applyCalls, 1);
      expect(bridge.lastAppliedRevision, BigInt.from(7));
    },
  );

  test(
    'applyActive surfaces a structured error and never fakes success',
    () async {
      final bridge = _FakeBridge(
        snapshotView: const RuntimeView(state: 'Stopped'),
        applyResult: const RuntimeActionResult(
          ok: false,
          error: RuntimeErrorView(
            code: 'E_REVISION_STALE',
            messageKey: 'error.revision_stale',
          ),
        ),
      );
      addTearDown(bridge._controller.close);
      final container = _containerFor(bridge);
      final controller = container.read(runtimeControllerProvider.notifier);
      await controller.start();

      await controller.applyActive();

      final view = container.read(runtimeControllerProvider);
      expect(view.error?.code, 'E_REVISION_STALE');
      expect(view.isRunning, isFalse);
    },
  );

  test(
    'restoreActiveOnLaunch applies the persisted active node once',
    () async {
      final bridge = _FakeBridge(
        snapshotView: RuntimeView(
          desiredRevision: BigInt.one,
          appliedRevision: BigInt.zero,
        ),
        activeId: 'node-a',
      );
      addTearDown(bridge._controller.close);
      final container = _containerFor(bridge);
      final controller = container.read(runtimeControllerProvider.notifier);
      await controller.start();

      await controller.restoreActiveOnLaunch();

      expect(bridge.applyCalls, 1);
      expect(bridge.lastAppliedRevision, BigInt.one);
    },
  );

  test('restoreActiveOnLaunch is a no-op without an active node', () async {
    final bridge = _FakeBridge(snapshotView: const RuntimeView());
    addTearDown(bridge._controller.close);
    final container = _containerFor(bridge);
    final controller = container.read(runtimeControllerProvider.notifier);
    await controller.start();

    await controller.restoreActiveOnLaunch();

    expect(bridge.applyCalls, 0);
  });

  test(
    'restoreActiveOnLaunch does not re-apply an already running session',
    () async {
      final bridge = _FakeBridge(
        snapshotView: RuntimeView(
          state: 'Running',
          desiredRevision: BigInt.one,
          appliedRevision: BigInt.one,
          ports: const [11810],
          sessionId: 's-1',
        ),
        activeId: 'node-a',
      );
      addTearDown(bridge._controller.close);
      final container = _containerFor(bridge);
      final controller = container.read(runtimeControllerProvider.notifier);
      await controller.start();

      await controller.restoreActiveOnLaunch();

      expect(bridge.applyCalls, 0);
    },
  );
}
