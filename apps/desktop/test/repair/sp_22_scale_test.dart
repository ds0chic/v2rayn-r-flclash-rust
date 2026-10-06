// SP-22 continuation: real-scale behavior — 10k nodes + 10k connections +
// massive log flood: memory trimming, throughput, connection generation
// switch / cancel recovery, dropped-line accounting.
//
// Synthetic data only (`*.example.invalid`, `flood N`); FakeMonitorBridge is
// the only bridge stand-in, no sockets, no host network.
import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/connections_columns.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_incremental.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';

import '../support/fake_monitor_bridge.dart';

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

RuntimeView _view(String session, int port) => RuntimeView(
  state: 'Running',
  ports: <int>[port],
  sessionId: session,
  desiredRevision: BigInt.one,
  appliedRevision: BigInt.one,
);

m.LogLineDto _line(String text) =>
    m.LogLineDto(text: text, level: 2, truncated: false);

m.ClashConnectionDto _conn(String id) => m.ClashConnectionDto(
  id: id,
  host: 'host-$id.example.invalid:443',
  network: 'tcp',
  connectionType: 'Shadowsocks',
  chains: <String>['RULE', 'proxy-$id'],
  rule: 'RULE',
  processPath: 'C:\\synthetic\\app.exe',
  upload: BigInt.zero,
  download: BigInt.zero,
);

m.ClashProxyDto _node(String name, int delay) => m.ClashProxyDto(
  name: name,
  proxyType: 'SS',
  isGroup: false,
  now: null,
  all: const <String>[],
  delay: delay,
);

void main() {
  group('SP-22 scale: 10k overlays stay bounded and fast', () {
    test('mergeDelayMap overlays 10k nodes completely', () {
      final items = List<m.DelayResultDto>.generate(
        10000,
        (i) => m.DelayResultDto(name: 'node-$i', delay: i % 900),
      );
      final sw = Stopwatch()..start();
      final merged = mergeDelayMap(const <String, int>{}, items);
      sw.stop();
      // ignore: avoid_print
      print('sp22-scale mergeDelayMap-10k: ${sw.elapsedMilliseconds}ms');
      expect(merged.length, 10000);
      expect(merged['node-0'], 0);
      expect(merged['node-9999'], 99);
      expect(sw.elapsed, lessThan(const Duration(seconds: 10)));
    });

    test('sortProxiesForDisplay sorts 10k nodes without mutating input', () {
      final nodes = List<m.ClashProxyDto>.generate(
        10000,
        (i) => _node('node-$i', (9999 - i) % 900),
      );
      final before = nodes.map((p) => p.name).toList();
      final sw = Stopwatch()..start();
      final sorted = sortProxiesForDisplay(
        const <m.ClashProxyDto>[],
        nodes,
        const <String, int>{},
        0,
      );
      sw.stop();
      // ignore: avoid_print
      print('sp22-scale sortProxies-10k: ${sw.elapsedMilliseconds}ms');
      expect(sorted.nodes.length, 10000);
      expect(nodes.map((p) => p.name).toList(), before);
      for (var i = 1; i < sorted.nodes.length; i++) {
        final prev = sorted.nodes[i - 1].delay < 0
            ? 1 << 30
            : sorted.nodes[i - 1].delay;
        final cur = sorted.nodes[i].delay < 0 ? 1 << 30 : sorted.nodes[i].delay;
        expect(cur, greaterThanOrEqualTo(prev));
      }
      expect(sw.elapsed, lessThan(const Duration(seconds: 10)));
    });

    test('mergeConnectionsById merges a 10k snapshot', () {
      final existing = List<m.ClashConnectionDto>.generate(
        10000,
        (i) => _conn('c$i'),
      );
      final incoming = List<m.ClashConnectionDto>.generate(
        10000,
        (i) => _conn('c${i + 5000}'),
      );
      final sw = Stopwatch()..start();
      final merged = mergeConnectionsById(existing, incoming);
      sw.stop();
      // ignore: avoid_print
      print('sp22-scale mergeConnections-10k: ${sw.elapsedMilliseconds}ms');
      // 5000 survivors stay in place, 5000 new append, 5000 gone dropped.
      expect(merged.length, 10000);
      expect(merged.first.id, 'c5000');
      expect(merged.last.id, 'c14999');
      expect(sw.elapsed, lessThan(const Duration(seconds: 10)));
    });

    test('filterConnections scans 10k rows bounded', () {
      final items = List<m.ClashConnectionDto>.generate(
        10000,
        (i) => _conn('n$i'),
      );
      final sw = Stopwatch()..start();
      final result = filterConnections(items, 'example.invalid');
      sw.stop();
      // ignore: avoid_print
      print('sp22-scale filterConnections-10k: ${sw.elapsedMilliseconds}ms');
      expect(result.totalMatches, 10000);
      expect(result.items.length, maxFilteredConnections);
      expect(result.truncated, isTrue);
      expect(sw.elapsed, lessThan(const Duration(seconds: 10)));
    });

    test('mergeLogTail bounds a 10k flood and keeps the final marker', () {
      final flood = List<m.LogLineDto>.generate(
        10000,
        (i) => _line('flood $i'),
      );
      final sw = Stopwatch()..start();
      final merged = mergeLogTail(
        const <m.LogLineDto>[],
        flood,
        maxDisplayedLogs,
      );
      sw.stop();
      // ignore: avoid_print
      print('sp22-scale mergeLogTail-10k: ${sw.elapsedMilliseconds}ms');
      expect(merged.length, maxDisplayedLogs);
      expect(merged.first.text, 'flood ${10000 - maxDisplayedLogs}');
      expect(merged.last.text, 'flood 9999');
      expect(sw.elapsed, lessThan(const Duration(seconds: 10)));
    });
  });

  group('SP-22 scale: generation switch / cancel recovery / accounting', () {
    test(
      'dropped-line counters apply immediately while rows coalesce',
      () async {
        final monitor = FakeMonitorBridge();
        final container = ProviderContainer(
          overrides: [
            monitorBridgeProvider.overrideWithValue(monitor),
            runtimeBridgeProvider.overrideWithValue(
              _FakeRuntimeBridge(_view('s-1', 11810)),
            ),
          ],
        );
        addTearDown(container.dispose);
        addTearDown(monitor.disposeStreams);
        final controller = container.read(monitorControllerProvider.notifier);
        controller.setPageVisible('logs', true);
        await Future<void>.delayed(Duration.zero);

        monitor.emitLogs(
          List<m.LogLineDto>.generate(100, (i) => _line('flood $i')),
          droppedLines: BigInt.from(8000),
          truncatedLines: BigInt.from(12),
        );
        // Rows wait for the 150ms coalesce window; accounting is already live.
        expect(container.read(monitorControllerProvider).logs, isEmpty);
        expect(
          container.read(monitorControllerProvider).droppedLines,
          BigInt.from(8000),
        );
        expect(
          container.read(monitorControllerProvider).truncatedLines,
          BigInt.from(12),
        );
        await Future<void>.delayed(const Duration(milliseconds: 300));
        expect(container.read(monitorControllerProvider).logs.length, 100);
        // Counters survive the row flush.
        expect(
          container.read(monitorControllerProvider).droppedLines,
          BigInt.from(8000),
        );
      },
    );

    test(
      'session switch clears stale rows; in-flight reads never clobber',
      () async {
        final monitor = FakeMonitorBridge(clashApiSupported: true);
        monitor.connections = List<m.ClashConnectionDto>.generate(
          10000,
          (i) => _conn('c$i'),
        );
        final container = ProviderContainer(
          overrides: [
            monitorBridgeProvider.overrideWithValue(monitor),
            runtimeBridgeProvider.overrideWithValue(
              _FakeRuntimeBridge(_view('s-1', 11810)),
            ),
          ],
        );
        addTearDown(container.dispose);
        addTearDown(monitor.disposeStreams);
        final controller = container.read(monitorControllerProvider.notifier);

        await controller.refreshConnections();
        expect(
          container.read(monitorControllerProvider).connections.length,
          10000,
        );

        // A slow read starts under generation N; the session switches; the
        // late response must be discarded instead of clobbering session N+1.
        final gate = Completer<void>();
        monitor.connectionsGate = gate.future;
        final pending = controller.refreshConnections();
        controller.syncRuntimeSession(_view('s-2', 11811));
        // Switch clears the previous session's rows immediately.
        expect(container.read(monitorControllerProvider).connections, isEmpty);
        expect(container.read(monitorControllerProvider).proxyDelays, isEmpty);
        gate.complete();
        await pending;
        await Future<void>.delayed(Duration.zero);
        expect(container.read(monitorControllerProvider).connections, isEmpty);

        // Recovery: the new session reads its own rows again.
        monitor.connectionsGate = null;
        monitor.connections = List<m.ClashConnectionDto>.generate(
          10,
          (i) => _conn('new-$i'),
        );
        await controller.refreshConnections();
        expect(
          container
              .read(monitorControllerProvider)
              .connections
              .map((c) => c.id),
          List<String>.generate(10, (i) => 'new-$i'),
        );
      },
    );

    test('close targets freeze; empty ids never reach the bridge', () async {
      final monitor = FakeMonitorBridge(clashApiSupported: true);
      monitor.connections = <m.ClashConnectionDto>[_conn('c1')];
      final container = ProviderContainer(
        overrides: [
          monitorBridgeProvider.overrideWithValue(monitor),
          runtimeBridgeProvider.overrideWithValue(
            _FakeRuntimeBridge(_view('s-1', 11810)),
          ),
        ],
      );
      addTearDown(container.dispose);
      addTearDown(monitor.disposeStreams);
      final controller = container.read(monitorControllerProvider.notifier);

      expect(await controller.closeConnection(''), isFalse);
      expect(monitor.closedConnections, isEmpty);

      final target = freezeConnectionClose('c1', 7);
      expect(target, isNotNull);
      expect(closeResponseIsCurrent(target!.generation, 7), isTrue);
      expect(closeResponseIsCurrent(target.generation, 8), isFalse);

      expect(await controller.closeConnection('c1'), isTrue);
      expect(monitor.closedConnections, <String>['c1']);
    });

    test('testGroup overlays 10k probe results by id without loss', () async {
      final monitor = FakeMonitorBridge(clashApiSupported: true);
      final names = List<String>.generate(10000, (i) => 'node-$i');
      monitor.proxies = <m.ClashProxyDto>[
        m.ClashProxyDto(
          name: 'g',
          proxyType: 'Selector',
          isGroup: true,
          now: 'node-0',
          all: names,
          delay: -1,
        ),
      ];
      final container = ProviderContainer(
        overrides: [
          monitorBridgeProvider.overrideWithValue(monitor),
          runtimeBridgeProvider.overrideWithValue(
            _FakeRuntimeBridge(_view('s-1', 11810)),
          ),
        ],
      );
      addTearDown(container.dispose);
      addTearDown(monitor.disposeStreams);
      final controller = container.read(monitorControllerProvider.notifier);

      final sw = Stopwatch()..start();
      await controller.testGroup('g');
      sw.stop();
      // ignore: avoid_print
      print('sp22-scale testGroup-10k: ${sw.elapsedMilliseconds}ms');
      final delays = container.read(monitorControllerProvider).proxyDelays;
      expect(delays.length, 10000);
      expect(delays['node-9999'], 42);
      expect(monitor.testedGroups, <String>['g']);
    });
  });
}
