import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';

import '../support/fake_monitor_bridge.dart';

/// R4-23 reproduction: a slow Clash read from a replaced applied session must
/// not overwrite the new session's read model, and the connections auto-refresh
/// timer must not stack overlapping in-flight requests.
///
/// Before the fix the controller applied whatever came back, so a stale
/// session-A response landed under session B, and two timer ticks started two
/// requests that could arrive out of order.

class _FakeRuntimeBridge implements RuntimeBridge {
  _FakeRuntimeBridge(this.view);

  RuntimeView view;
  final StreamController<RuntimeEvent> _events =
      StreamController<RuntimeEvent>.broadcast();

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

final _sessionA = RuntimeView(
  state: 'Running',
  ports: <int>[11810],
  sessionId: 's-a',
  desiredRevision: BigInt.one,
  appliedRevision: BigInt.one,
);

final _sessionB = RuntimeView(
  state: 'Running',
  ports: <int>[11811],
  sessionId: 's-b',
  desiredRevision: BigInt.two,
  appliedRevision: BigInt.two,
);

m.ClashConnectionDto _conn(String id) => m.ClashConnectionDto(
  id: id,
  host: 'host-$id',
  network: 'tcp',
  connectionType: 'HTTP',
  chains: const <String>[],
  upload: BigInt.zero,
  download: BigInt.zero,
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
  test('a late connection read from a replaced session is dropped', () async {
    final gate = Completer<void>();
    final monitor = FakeMonitorBridge(
      clashApiSupported: true,
      connections: <m.ClashConnectionDto>[_conn('old')],
    )..connectionsGate = gate.future;
    final runtime = _FakeRuntimeBridge(_sessionA);
    final container = _container(monitor, runtime);
    final controller = container.read(monitorControllerProvider.notifier);

    // Session A's connection read is in flight on a slow Clash API.
    final pending = controller.refreshConnections();

    // The applied session switches to B before A's response arrives.
    controller.syncRuntimeSession(_sessionB);

    gate.complete();
    await pending;
    await Future<void>.delayed(Duration.zero);

    final state = container.read(monitorControllerProvider);
    expect(
      state.connections.where((c) => c.id == 'old'),
      isEmpty,
      reason: 'a stale session-A response must not be applied to session B',
    );
  });

  test('overlapping refreshConnections share one in-flight request', () async {
    final gate = Completer<void>();
    final monitor = FakeMonitorBridge(clashApiSupported: true)
      ..connectionsGate = gate.future;
    final runtime = _FakeRuntimeBridge(_sessionA);
    final container = _container(monitor, runtime);
    final controller = container.read(monitorControllerProvider.notifier);
    controller.syncRuntimeSession(_sessionA);

    final first = controller.refreshConnections();
    final second = controller.refreshConnections();
    gate.complete();
    await Future.wait(<Future<void>>[first, second]);

    expect(
      monitor.clashConnectionsCount,
      1,
      reason: 'the auto-refresh timer must not stack overlapping polls',
    );
  });
}
