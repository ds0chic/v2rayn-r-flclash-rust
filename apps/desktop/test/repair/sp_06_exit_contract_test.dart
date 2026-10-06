import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';

/// SP-06: managed-core/sidecar exit observation (Dart contract half).
///
/// The Rust net-host owns exit observation (handle-authoritative reconcile on
/// every snapshot/detail read); this file locks the Dart side of the same
/// contract with a scripted bridge (no native library, no sockets, no OS
/// writes; synthetic ids and probed-style ports >= 11808 only):
///
/// - a post-readiness exit never reads as Running: the snapshot withdraws the
///   PID/ports and carries the structured `error.core_exited` cause;
/// - the view labels the actual exit (not a generic stopped) and offers the
///   recovery entry (`canRecover`: retry via apply / restart via stop+apply);
/// - a sidecar exit under a live main core degrades instead of reporting a
///   clean Running, and the main endpoint is kept;
/// - a successful retry returns to Running and clears the recovery entry.
class ExitBridge
    implements
        RuntimeBridge,
        ExplicitTargetRuntimeBridge,
        OperationQueryBridge {
  ExitBridge({this.snapshotView = const RuntimeView()});

  final StreamController<RuntimeEvent> _events =
      StreamController<RuntimeEvent>.broadcast();

  final List<String> calls = [];
  RuntimeView snapshotView;

  Future<void> get done => _events.close();

  @override
  String? activeProfileId() => 'synthetic-node';

  @override
  BigInt desiredRevision() => BigInt.one;

  @override
  Stream<RuntimeEvent> events() => _events.stream;

  @override
  Future<RuntimeView> snapshot() async => snapshotView;

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
    calls.add('apply:$targetId');
    snapshotView = const RuntimeView(
      state: 'Running',
      hostAlive: true,
      pid: 4242,
      ports: [11977],
      sessionId: 'synthetic-session',
    );
    return const RuntimeActionResult(ok: true, operationId: 'sp06-op-1');
  }

  @override
  Future<RuntimeActionResult> stop() async {
    calls.add('stop');
    snapshotView = const RuntimeView();
    return const RuntimeActionResult(ok: true);
  }

  @override
  Future<RuntimeOperationView?> operationStatus(String operationId) async =>
      null;
}

ProviderContainer _containerFor(ExitBridge bridge) {
  final container = ProviderContainer(
    overrides: [runtimeBridgeProvider.overrideWithValue(bridge)],
  );
  addTearDown(container.dispose);
  addTearDown(() => bridge.done);
  return container;
}

/// A reconciled post-exit snapshot: Stopped, no PID/ports, structured cause.
const exitedSnapshot = RuntimeView(
  state: 'Stopped',
  hostAlive: true,
  error: RuntimeErrorView(
    code: 'E_INTERNAL',
    messageKey: 'error.core_exited',
    detail: 'pid=4242 code=1',
  ),
);

void main() {
  test('SP-06: an exited core reads as exited with a recovery entry, never Running', () async {
    final bridge = ExitBridge();
    final container = _containerFor(bridge);
    final controller = container.read(runtimeControllerProvider.notifier);
    await controller.start();

    await controller.applyActive(targetId: 'synthetic-node');
    expect(container.read(runtimeControllerProvider).isRunning, isTrue);

    // The core exits on its own; the next reconciled read withdraws it.
    bridge.snapshotView = exitedSnapshot;
    await controller.refresh();

    final view = container.read(runtimeControllerProvider);
    expect(view.isRunning, isFalse);
    expect(view.pid, isNull);
    expect(view.ports, isEmpty);
    expect(view.isCoreExited, isTrue);
    expect(view.canRecover, isTrue);
    expect(view.statusLabel.contains('已退出'), isTrue);
    expect(view.statusLabel.contains('4242'), isTrue);
  });

  test(
    'SP-06: a sidecar exit degrades a live session and keeps the endpoint',
    () async {
      final bridge = ExitBridge();
      final container = _containerFor(bridge);
      final controller = container.read(runtimeControllerProvider.notifier);
      await controller.start();

      await controller.applyActive(targetId: 'synthetic-node');

      bridge.snapshotView = const RuntimeView(
        state: 'Degraded',
        hostAlive: true,
        pid: 4242,
        ports: [11977],
        sessionId: 'synthetic-session',
        error: RuntimeErrorView(
          code: 'E_INTERNAL',
          messageKey: 'error.sidecar_exited',
          detail: 'sidecar `pre-socks` exited (code=3)',
        ),
      );
      await controller.refresh();

      final view = container.read(runtimeControllerProvider);
      expect(view.isRunning, isFalse);
      expect(view.isSidecarDegraded, isTrue);
      expect(view.canRecover, isTrue);
      // The live main endpoint is kept for diagnosis, not withdrawn.
      expect(view.pid, 4242);
      expect(view.ports, [11977]);
      expect(view.statusLabel.contains('降级'), isTrue);
    },
  );

  test('SP-06: retry after an exit returns to Running', () async {
    final bridge = ExitBridge();
    final container = _containerFor(bridge);
    final controller = container.read(runtimeControllerProvider.notifier);
    await controller.start();

    await controller.applyActive(targetId: 'synthetic-node');
    bridge.snapshotView = exitedSnapshot;
    await controller.refresh();
    expect(container.read(runtimeControllerProvider).canRecover, isTrue);

    final ok = await controller.applyActive(targetId: 'synthetic-node');
    expect(ok, isTrue);
    final view = container.read(runtimeControllerProvider);
    expect(view.isRunning, isTrue);
    expect(view.canRecover, isFalse);
    expect(view.error, isNull);
    expect(bridge.calls, ['apply:synthetic-node', 'apply:synthetic-node']);
  });

  test(
    'SP-06: a clean stop is not an exit and offers no exit recovery',
    () async {
      final bridge = ExitBridge();
      final container = _containerFor(bridge);
      final controller = container.read(runtimeControllerProvider.notifier);
      await controller.start();

      await controller.applyActive(targetId: 'synthetic-node');
      await controller.stop();

      final view = container.read(runtimeControllerProvider);
      expect(view.isRunning, isFalse);
      expect(view.isCoreExited, isFalse);
      expect(view.canRecover, isFalse);
      expect(view.statusLabel, '未运行');
    },
  );
}
