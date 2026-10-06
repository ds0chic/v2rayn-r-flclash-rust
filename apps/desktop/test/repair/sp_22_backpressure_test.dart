// SP-22 红合同：测速同时浏览日志/连接并筛选节点，界面不被全量 overlay 与 IO 阻塞。
//
// 增量可合并：日志尾部合并有界、测速延迟按 ID 覆盖、连接快照按 ID 合并、
// 筛选有界可截断、代理排序不 mutated 原列表。
// 高频日志合并刷新（150ms）：计数器/暂停标记立即生效，行刷新合并一次；
// 生命周期（traffic）更新不被日志刷新挤掉。
// 合成数据，不触宿主网络；Fake 仅做 bridge 替身。
import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
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

final _running = RuntimeView(
  state: 'Running',
  ports: <int>[11810],
  sessionId: 's-1',
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

void main() {
  group('SP-22: incremental overlay helpers', () {
    test('mergeLogTail appends and keeps only the bounded tail', () {
      final existing = List<m.LogLineDto>.generate(5, (i) => _line('old $i'));
      final merged = mergeLogTail(existing, <m.LogLineDto>[
        _line('new a'),
        _line('new b'),
      ], 6);
      expect(merged.map((e) => e.text).toList(), <String>[
        'old 1',
        'old 2',
        'old 3',
        'old 4',
        'new a',
        'new b',
      ]);
      // 原列表不被 mutated。
      expect(existing.length, 5);
    });

    test('mergeDelayMap overlays speedtest results by id', () {
      final existing = <String, int>{'a': 10, 'b': -1};
      final merged = mergeDelayMap(existing, <m.DelayResultDto>[
        const m.DelayResultDto(name: 'b', delay: 42),
        const m.DelayResultDto(name: 'c', delay: 7),
      ]);
      expect(merged, <String, int>{'a': 10, 'b': 42, 'c': 7});
      // 原 map 不被 mutated。
      expect(existing, <String, int>{'a': 10, 'b': -1});
    });

    test('mergeConnectionsById updates in place, appends new, drops gone', () {
      final existing = <m.ClashConnectionDto>[_conn('a'), _conn('b')];
      final updated = _conn('b');
      final merged = mergeConnectionsById(existing, <m.ClashConnectionDto>[
        updated,
        _conn('c'),
      ]);
      // 'a' 快照中消失即丢弃；'b' 原位更新（同一实例），'c' 追加。
      expect(merged.map((c) => c.id).toList(), <String>['b', 'c']);
      expect(identical(merged[0], updated), isTrue);
      // 已消失的连接被丢弃，不残留。
      final pruned = mergeConnectionsById(existing, <m.ClashConnectionDto>[
        _conn('b'),
      ]);
      expect(pruned.map((c) => c.id).toList(), <String>['b']);
    });

    test('filterConnections is bounded and reports truncation', () {
      final items = List<m.ClashConnectionDto>.generate(
        10000,
        (i) => _conn('n$i'),
      );
      final result = filterConnections(items, 'example.invalid', limit: 300);
      expect(result.totalMatches, 10000);
      expect(result.items.length, 300);
      expect(result.truncated, isTrue);
      final none = filterConnections(items, 'no-such-needle-xyz');
      expect(none.totalMatches, 0);
      expect(none.truncated, isFalse);
    });

    test('sortProxiesForDisplay copies instead of mutating state lists', () {
      final groups = <m.ClashProxyDto>[
        const m.ClashProxyDto(
          name: 'g',
          proxyType: 'Selector',
          isGroup: true,
          now: 'n2',
          all: <String>['n1', 'n2'],
          delay: -1,
        ),
      ];
      final nodes = <m.ClashProxyDto>[
        const m.ClashProxyDto(
          name: 'n2',
          proxyType: 'SS',
          isGroup: false,
          now: null,
          all: <String>[],
          delay: 90,
        ),
        const m.ClashProxyDto(
          name: 'n1',
          proxyType: 'SS',
          isGroup: false,
          now: null,
          all: <String>[],
          delay: 10,
        ),
      ];
      final before = nodes.map((p) => p.name).toList();
      final sorted = sortProxiesForDisplay(
        groups,
        nodes,
        const <String, int>{},
        0,
      );
      expect(sorted.nodes.map((p) => p.name).toList(), <String>['n1', 'n2']);
      expect(nodes.map((p) => p.name).toList(), before);
    });
  });

  group('SP-22: log batches coalesce, lifecycle is never squeezed out', () {
    test('rapid batches flush once; counters apply immediately', () async {
      final monitor = FakeMonitorBridge();
      final container = ProviderContainer(
        overrides: [
          monitorBridgeProvider.overrideWithValue(monitor),
          runtimeBridgeProvider.overrideWithValue(_FakeRuntimeBridge(_running)),
        ],
      );
      addTearDown(container.dispose);
      addTearDown(monitor.disposeStreams);
      final controller = container.read(monitorControllerProvider.notifier);
      controller.setPageVisible('logs', true);
      await Future<void>.delayed(Duration.zero);

      // 5 个高频批次连续到达：行刷新合并，计数器立即生效。
      for (var b = 0; b < 5; b++) {
        monitor.emitLogs(
          List<m.LogLineDto>.generate(10, (i) => _line('batch$b line$i')),
          droppedLines: BigInt.from(b),
        );
      }
      // 合并窗口内（150ms）不逐批全量重刷。
      expect(container.read(monitorControllerProvider).logs, isEmpty);
      expect(
        container.read(monitorControllerProvider).droppedLines,
        BigInt.from(4),
      );

      // 生命周期更新在日志合并等待期间仍然直达。
      monitor.emitTraffic(proxyUp: BigInt.from(1024));
      expect(
        container.read(monitorControllerProvider).proxyUp,
        BigInt.from(1024),
      );

      // 合并窗口到期后一次刷新，5 批共 50 行全部进入有界尾部。
      await Future<void>.delayed(const Duration(milliseconds: 300));
      final logs = container.read(monitorControllerProvider).logs;
      expect(logs.length, 50);
      expect(logs.first.text, 'batch0 line0');
      expect(logs.last.text, 'batch4 line9');
    });

    test('a 10k burst stays bounded and keeps the final marker', () async {
      final monitor = FakeMonitorBridge();
      final container = ProviderContainer(
        overrides: [
          monitorBridgeProvider.overrideWithValue(monitor),
          runtimeBridgeProvider.overrideWithValue(_FakeRuntimeBridge(_running)),
        ],
      );
      addTearDown(container.dispose);
      addTearDown(monitor.disposeStreams);
      final controller = container.read(monitorControllerProvider.notifier);
      controller.setPageVisible('logs', true);
      await Future<void>.delayed(Duration.zero);

      monitor.emitLogs(
        List<m.LogLineDto>.generate(10000, (i) => _line('flood $i')),
      );
      await Future<void>.delayed(const Duration(milliseconds: 300));
      final logs = container.read(monitorControllerProvider).logs;
      expect(logs.length, lessThanOrEqualTo(maxDisplayedLogs));
      // 最终结果（尾部）不被高频日志挤掉。
      expect(logs.last.text, 'flood 9999');
    });
  });
}
