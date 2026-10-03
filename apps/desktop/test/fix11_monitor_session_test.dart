import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
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

final _running = RuntimeView(
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
  test(
    'applied session enables polling and real traffic reaches the model',
    () async {
      final monitor = FakeMonitorBridge();
      final runtime = _FakeRuntimeBridge(_running);
      final container = _container(monitor, runtime);

      // The monitor controller is built first (status bar), then the applied
      // runtime session arrives through the normal startup/apply path.
      container.read(monitorControllerProvider);
      await container.read(runtimeControllerProvider.notifier).start();
      await Future<void>.delayed(Duration.zero);

      expect(monitor.syncSessionCount, greaterThan(0));
      expect(monitor.pollingStarted, isTrue);

      monitor.emitTraffic(
        proxyUp: BigInt.from(1024),
        proxyDown: BigInt.from(2048),
        proxyUpBps: BigInt.from(128),
      );
      final state = container.read(monitorControllerProvider);
      expect(state.hasTraffic, isTrue);
      expect(state.proxyUp, BigInt.from(1024));
      expect(state.proxyDown, BigInt.from(2048));
    },
  );

  test('hiding a page pauses only that page, never the collection', () async {
    final monitor = FakeMonitorBridge();
    final runtime = _FakeRuntimeBridge(_running);
    final container = _container(monitor, runtime);
    container.read(monitorControllerProvider);
    await container.read(runtimeControllerProvider.notifier).start();
    await Future<void>.delayed(Duration.zero);

    container
        .read(monitorControllerProvider.notifier)
        .setPageVisible('logs', false);
    expect(monitor.pageVisibility['logs'], isFalse);

    // Traffic collection continues while the page is hidden.
    monitor.emitTraffic(proxyUp: BigInt.from(7));
    expect(container.read(monitorControllerProvider).proxyUp, BigInt.from(7));
  });

  test('stopping the core re-syncs and stops the old session', () async {
    final monitor = FakeMonitorBridge();
    final runtime = _FakeRuntimeBridge(_running);
    final container = _container(monitor, runtime);
    container.read(monitorControllerProvider);
    await container.read(runtimeControllerProvider.notifier).start();
    await Future<void>.delayed(Duration.zero);
    final before = monitor.syncSessionCount;

    runtime.view = const RuntimeView();
    runtime.emit(
      const RuntimeEvent(kind: 'runtime_state_changed', payloadJson: '{}'),
    );
    await Future<void>.delayed(const Duration(milliseconds: 250));

    // The stop transition re-syncs the monitor; the Rust side clears the old
    // session's ports so its collection stops.
    expect(monitor.syncSessionCount, greaterThan(before));
  });
}
