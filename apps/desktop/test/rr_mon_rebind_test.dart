import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as contract;
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';

import 'support/fake_monitor_bridge.dart';

/// Runtime bridge whose snapshot can be swapped, so the test drives the
/// applied-session transition without touching net-host.
class _FakeRuntimeBridge implements RuntimeBridge {
  _FakeRuntimeBridge(this.view);

  RuntimeView view;
  final StreamController<RuntimeEvent> _events =
      StreamController<RuntimeEvent>.broadcast();

  void emit(RuntimeEvent event) => _events.add(event);

  @override
  Future<RuntimeView> snapshot() async => view;

  @override
  String? activeProfileId() => view.hasAppliedEndpoint ? 'node-a' : null;

  @override
  Future<RuntimeActionResult> applyActive({
    required BigInt expectedRevision,
  }) async => const RuntimeActionResult(ok: true);

  @override
  Future<RuntimeActionResult> stop() async {
    view = const RuntimeView();
    return const RuntimeActionResult(ok: true);
  }

  @override
  Stream<RuntimeEvent> events() => _events.stream;
}

RuntimeView _running() => RuntimeView(
  state: 'Running',
  ports: <int>[11810],
  sessionId: 's-1',
  desiredRevision: BigInt.one,
  appliedRevision: BigInt.one,
);

ProviderContainer _container(
  FakeMonitorBridge monitor,
  _FakeRuntimeBridge runtime,
) {
  final container = ProviderContainer(
    overrides: [
      monitorBridgeProvider.overrideWithValue(monitor),
      runtimeBridgeProvider.overrideWithValue(runtime),
    ],
  );
  addTearDown(container.dispose);
  addTearDown(monitor.disposeStreams);
  addTearDown(runtime._events.close);
  return container;
}

void main() {
  test('unrelated RuntimeView churn keeps the same session signature', () {
    final base = _running();
    final heartbeat = base.copyWith(epoch: BigInt.one, lastSeq: BigInt.from(7));
    expect(monitorSessionSignature(heartbeat), monitorSessionSignature(base));

    final switched = RuntimeView(
      state: 'Running',
      ports: <int>[11811],
      sessionId: 's-2',
      desiredRevision: BigInt.one,
      appliedRevision: BigInt.one,
    );
    expect(
      monitorSessionSignature(switched),
      isNot(monitorSessionSignature(base)),
    );
  });

  test(
    'heartbeat does not re-sync the source; a real applied change does',
    () async {
      final monitor = FakeMonitorBridge();
      final runtime = _FakeRuntimeBridge(_running());
      final container = _container(monitor, runtime);

      container.read(monitorControllerProvider);
      await container.read(runtimeControllerProvider.notifier).start();
      await Future<void>.delayed(Duration.zero);
      final afterApply = monitor.syncSessionCount;
      expect(afterApply, greaterThan(0));

      // Event sequence only moves (heartbeat); the applied session is unchanged
      // so the Rust poller must not be asked to rebuild its source.
      runtime.emit(
        RuntimeEvent(
          kind: 'runtime_heartbeat',
          payloadJson: '{}',
          epoch: BigInt.one,
          seq: BigInt.one,
        ),
      );
      await Future<void>.delayed(Duration.zero);
      expect(monitor.syncSessionCount, afterApply);

      // A real endpoint change re-syncs.
      runtime.view = RuntimeView(
        state: 'Running',
        ports: <int>[11811],
        sessionId: 's-2',
        desiredRevision: BigInt.one,
        appliedRevision: BigInt.one,
      );
      runtime.emit(
        RuntimeEvent(
          kind: 'runtime_state_changed',
          payloadJson: '{}',
          epoch: BigInt.one,
          seq: BigInt.two,
        ),
      );
      await Future<void>.delayed(const Duration(milliseconds: 250));
      expect(monitor.syncSessionCount, greaterThan(afterApply));
    },
  );

  test('a store bind/load failure is visible on the monitor state', () async {
    final monitor = FakeMonitorBridge(
      statsError: const contract.ErrorDto(
        code: 'E_MONITOR_STORE',
        messageKey: 'error.monitor.store_bind_failed',
        retryable: true,
      ),
    );
    final runtime = _FakeRuntimeBridge(_running());
    final container = _container(monitor, runtime);

    container.read(monitorControllerProvider);
    await container.read(runtimeControllerProvider.notifier).start();
    await Future<void>.delayed(Duration.zero);

    expect(container.read(monitorControllerProvider).error, 'E_MONITOR_STORE');
  });
}
