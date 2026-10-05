import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as contract;
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_format.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';

import 'support/fake_monitor_bridge.dart';

/// R4-23 contract: the monitor read model is bound to the applied session.
///
/// Covers the card's mandatory scenarios at the controller level: session
/// binding / switch / stop, late-response eviction, in-flight coalescing,
/// separate collection vs scroll pause, hidden-page collection, real Clash
/// close semantics, non-zero per-node statistics rows and log filtering.
/// The real-core Rust side (flush outside the hub lock, retryable store bind,
/// Clash mock on a port >= 11808) is exercised by the `bridge_api` tests.

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

final _runningA = RuntimeView(
  state: 'Running',
  ports: <int>[11810],
  sessionId: 's-a',
  desiredRevision: BigInt.one,
  appliedRevision: BigInt.one,
);

final _runningB = RuntimeView(
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

const _proxy = m.ClashProxyDto(
  name: 'GROUP',
  proxyType: 'Selector',
  isGroup: true,
  now: 'A',
  all: <String>['A', 'B'],
  delay: 0,
  provider: null,
);

({
  ProviderContainer container,
  FakeMonitorBridge bridge,
  _FakeRuntimeBridge runtime,
})
_harness({FakeMonitorBridge? bridge}) {
  final monitor = bridge ?? FakeMonitorBridge();
  final runtime = _FakeRuntimeBridge(_runningA);
  final container = ProviderContainer(
    overrides: [
      monitorBridgeProvider.overrideWithValue(monitor),
      runtimeBridgeProvider.overrideWithValue(runtime),
    ],
  );
  addTearDown(container.dispose);
  addTearDown(monitor.disposeStreams);
  addTearDown(runtime._events.close);
  return (container: container, bridge: monitor, runtime: runtime);
}

void main() {
  test(
    'applied session binds statistics, then a switch drops the old session',
    () async {
      final monitor = FakeMonitorBridge(
        clashApiSupported: true,
        connections: <m.ClashConnectionDto>[_conn('a')],
      );
      monitor.statsNodes = const <m.NodeTrafficDto>[
        m.NodeTrafficDto(
          indexId: 'n1',
          totalUp: 500,
          totalDown: 600,
          todayUp: 111,
          todayDown: 222,
          dateNow: 20000,
        ),
      ];
      final h = _harness(bridge: monitor);
      final controller = h.container.read(monitorControllerProvider.notifier);

      controller.syncRuntimeSession(_runningA);
      await Future<void>.delayed(Duration.zero);
      await controller.refreshConnections();

      expect(h.bridge.syncSessionCount, greaterThan(0));
      expect(h.container.read(monitorControllerProvider).hasTodayNodes, isTrue);
      expect(
        h.container.read(monitorControllerProvider).todayUp,
        BigInt.from(111),
      );
      expect(
        h.container
            .read(monitorControllerProvider)
            .connections
            .map((c) => c.id),
        contains('a'),
      );

      // Switch to a new applied session: old Clash rows must not persist.
      controller.syncRuntimeSession(_runningB);
      final switched = h.container.read(monitorControllerProvider);
      expect(switched.connections, isEmpty);
      expect(switched.proxies, isEmpty);
      // Per-node statistics are session-independent and survive the switch.
      expect(switched.hasTodayNodes, isTrue);
    },
  );

  test('stopping the core clears the old session and re-syncs the poller', () {
    final monitor = FakeMonitorBridge(
      clashApiSupported: true,
      connections: <m.ClashConnectionDto>[_conn('a')],
    );
    final h = _harness(bridge: monitor);
    final controller = h.container.read(monitorControllerProvider.notifier);
    controller.syncRuntimeSession(_runningA);
    final before = h.bridge.syncSessionCount;

    controller.syncRuntimeSession(const RuntimeView());
    expect(h.bridge.syncSessionCount, greaterThan(before));
    expect(h.container.read(monitorControllerProvider).connections, isEmpty);
  });

  test('late proxes/mode reads from a replaced session are dropped', () async {
    final proxyGate = Completer<void>();
    final monitor = FakeMonitorBridge(
      clashApiSupported: true,
      proxies: const <m.ClashProxyDto>[_proxy],
    )..proxiesGate = proxyGate.future;
    final h = _harness(bridge: monitor);
    final controller = h.container.read(monitorControllerProvider.notifier);
    controller.syncRuntimeSession(_runningA);

    final pending = controller.refreshProxies();
    controller.syncRuntimeSession(_runningB);
    proxyGate.complete();
    await pending;

    expect(h.container.read(monitorControllerProvider).proxies, isEmpty);
  });

  test(
    'overlapping proxy refresh coalesces to one in-flight request',
    () async {
      final gate = Completer<void>();
      final monitor = FakeMonitorBridge(clashApiSupported: true)
        ..proxiesGate = gate.future;
      final h = _harness(bridge: monitor);
      final controller = h.container.read(monitorControllerProvider.notifier);
      controller.syncRuntimeSession(_runningA);

      final first = controller.refreshProxies();
      final second = controller.refreshProxies();
      gate.complete();
      await Future.wait(<Future<void>>[first, second]);

      expect(h.bridge.clashProxiesCount, 1);
    },
  );

  test('collection pause and scroll pause are independent', () {
    final h = _harness();
    final controller = h.container.read(monitorControllerProvider.notifier);

    controller.setCollectingPaused(true);
    expect(h.bridge.lastCollectingPaused, isTrue);
    expect(h.bridge.lastScrollPaused, isFalse);
    expect(h.container.read(monitorControllerProvider).scrollPaused, isFalse);

    controller.setScrollPaused(true);
    expect(h.bridge.lastScrollPaused, isTrue);
    expect(h.bridge.lastCollectingPaused, isTrue);
  });

  test('hiding the page freezes the view but never the collection', () {
    final h = _harness(bridge: FakeMonitorBridge(clashApiSupported: true));
    final controller = h.container.read(monitorControllerProvider.notifier);
    controller.syncRuntimeSession(_runningA);
    controller.setPageVisible('logs', true);
    controller.setPageVisible('logs', false);
    expect(h.bridge.pageVisibility['logs'], isFalse);

    h.bridge.emitLogs(<m.LogLineDto>[
      const m.LogLineDto(text: 'hidden', level: 2, truncated: false),
    ]);
    expect(h.container.read(monitorControllerProvider).logs, isEmpty);

    h.bridge.emitTraffic(proxyUp: BigInt.from(7));
    expect(h.container.read(monitorControllerProvider).proxyUp, BigInt.from(7));
  });

  test(
    'closing a connection uses the real bridge and refreshes the list',
    () async {
      final monitor = FakeMonitorBridge(
        clashApiSupported: true,
        connections: <m.ClashConnectionDto>[_conn('a'), _conn('b')],
      );
      final h = _harness(bridge: monitor);
      final controller = h.container.read(monitorControllerProvider.notifier);
      controller.setPageVisible('connections', true);
      await controller.refreshConnections();
      expect(h.container.read(monitorControllerProvider).connections.length, 2);

      final ok = await controller.closeConnection('a');
      expect(ok, isTrue);
      expect(h.bridge.closedConnections, <String>['a']);
      expect(
        h.container
            .read(monitorControllerProvider)
            .connections
            .map((c) => c.id),
        <String>['b'],
      );
    },
  );

  test('close-all failure is not reported as success', () async {
    final monitor = FakeMonitorBridge(
      clashApiSupported: true,
      connections: <m.ClashConnectionDto>[_conn('a')],
      closeAllFails: true,
    );
    final h = _harness(bridge: monitor);
    final controller = h.container.read(monitorControllerProvider.notifier);
    controller.setPageVisible('connections', true);
    await controller.refreshConnections();

    final ok = await controller.closeAllConnections();
    expect(ok, isFalse);
    expect(h.bridge.closeAllCount, 1);
    expect(h.container.read(monitorControllerProvider).connections, isNotEmpty);
  });

  test('log filtering, scroll pause and copy are presentation-only', () {
    final h = _harness(bridge: FakeMonitorBridge(clashApiSupported: true));
    final controller = h.container.read(monitorControllerProvider.notifier);
    controller.syncRuntimeSession(_runningA);
    controller.setPageVisible('logs', true);
    h.bridge.emitLogs(<m.LogLineDto>[
      const m.LogLineDto(text: 'info start', level: 2, truncated: false),
      const m.LogLineDto(text: 'warn disk', level: 3, truncated: false),
      const m.LogLineDto(text: 'error disk full', level: 4, truncated: false),
    ]);

    controller.setMinLevel(3);
    controller.setKeyword('disk');
    final state = h.container.read(monitorControllerProvider);
    expect(state.visibleLogs.length, 2);
    expect(
      formatLogsForCopy(state.visibleLogs),
      'warn disk\nerror disk full\n',
    );

    controller.setScrollPaused(true);
    expect(h.bridge.lastScrollPaused, isTrue);
    expect(h.bridge.lastCollectingPaused, isFalse);
  });

  test('a store bind/load failure is visible and cleared on retry', () {
    final monitor = FakeMonitorBridge(
      clashApiSupported: true,
      statsError: const contract.ErrorDto(
        code: 'E_MONITOR_STORE',
        messageKey: 'error.monitor.store_bind_failed',
        retryable: true,
      ),
    );
    final h = _harness(bridge: monitor);
    final controller = h.container.read(monitorControllerProvider.notifier);
    controller.syncRuntimeSession(_runningA);

    expect(
      h.container.read(monitorControllerProvider).error,
      'E_MONITOR_STORE',
    );

    // A retry after the bind succeeds surfaces no error.
    monitor.statsError = null;
    controller.refreshStats();
    expect(h.container.read(monitorControllerProvider).error, isNull);
  });
}
