// Wave B (FLD-CFG-131..135): ClashUI save-failure rollback + poll wiring.
//
// 原版对照（v2rayN 7.25.4 / 7d6a967，只读 work/）：`ConfigItems.ClashUIItem`
// （ProxiesSorting=0 / ProxiesAutoRefresh=false / ProxiesRefreshInterval=2 /
// ConnectionsAutoRefresh=false / ConnectionsRefreshInterval=2），
// `ClashProxiesViewModel` / `ClashConnectionsViewModel` 按 AutoRefresh +
// RefreshInterval 节流轮询（非正间隔停轮询），排序 0=延迟/1=名称。
//
// 本文件用合成数据 + 故障注入 saveGroup（ synthetic only，无 socket/代理/
// 10808 操作），断言：保存失败→可见报错 + 开关回滚到持久值 + 轮询保持
// canonical（未持久值永不驱动 UI 与 timer）；成功→新值持久 + 轮询跟随 +
// 重开保持。
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as contract;
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/bridge/api/settings.dart' as settings;
import 'package:v2rayn_desktop/features/monitor/clash_ui_config.dart';
import 'package:v2rayn_desktop/features/monitor/connections_view.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/monitor/proxies_view.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

import '../support/fake_monitor_bridge.dart';

m.ClashProxyDto _node(String name, int delay) => m.ClashProxyDto(
  name: name,
  proxyType: 'Shadowsocks',
  isGroup: false,
  now: null,
  all: const <String>[],
  delay: delay,
  provider: null,
);

Map<String, dynamic> _clashDoc({
  int sorting = 0,
  bool proxiesAuto = false,
  int proxiesInterval = 2,
  bool connectionsAuto = false,
  int connectionsInterval = 2,
}) => <String, dynamic>{
  'ClashUIItem': <String, dynamic>{
    'ProxiesSorting': sorting,
    'ProxiesAutoRefresh': proxiesAuto,
    'ProxiesRefreshInterval': proxiesInterval,
    'ConnectionsAutoRefresh': connectionsAuto,
    'ConnectionsRefreshInterval': connectionsInterval,
  },
};

/// Settings 替身：固定文档；[failSave] 为 true 时 saveGroup 恒失败（故障
/// 注入），成功时同步更新 state 以便 provider 派生 canonical。
class _WaveBSettings extends SettingsController {
  _WaveBSettings(this.doc, {this.failSave = false});

  final Map<String, dynamic> doc;
  final bool failSave;
  int saveAttempts = 0;

  @override
  SettingsViewState build() =>
      SettingsViewState(loaded: true, revision: 1, document: doc);

  @override
  settings.SaveSettingsResult saveGroup(String group, Object? value) {
    saveAttempts++;
    if (failSave) {
      return const settings.SaveSettingsResult(
        ok: false,
        changes: <settings.SettingsChangeDto>[],
        restartCoreFields: <String>[],
        restartAppFields: <String>[],
        nextLaunchFields: <String>[],
        error: contract.ErrorDto(
          code: 'E_STORAGE_UNAVAILABLE',
          messageKey: 'error.storage_unavailable',
          retryable: true,
        ),
      );
    }
    doc[group] = value;
    state = state.copyWith(document: Map<String, dynamic>.of(doc));
    return const settings.SaveSettingsResult(
      ok: true,
      changes: <settings.SettingsChangeDto>[],
      restartCoreFields: <String>[],
      restartAppFields: <String>[],
      nextLaunchFields: <String>[],
    );
  }
}

Future<ProviderContainer> _pumpProxies(
  WidgetTester tester, {
  required _WaveBSettings seed,
  required FakeMonitorBridge bridge,
}) async {
  addTearDown(bridge.disposeStreams);
  final container = ProviderContainer(
    overrides: [
      monitorBridgeProvider.overrideWithValue(bridge),
      settingsControllerProvider.overrideWith(() => seed),
    ],
  );
  addTearDown(container.dispose);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: ProxiesView())),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 20));
  return container;
}

/// 独立重开：同 container 重新建页，新 state 必须从 canonical 播种。
Future<void> _reopenProxies(
  WidgetTester tester,
  ProviderContainer container,
) async {
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: ProxiesView())),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 20));
}

Future<ProviderContainer> _pumpConnections(
  WidgetTester tester, {
  required _WaveBSettings seed,
  required FakeMonitorBridge bridge,
}) async {
  addTearDown(bridge.disposeStreams);
  final container = ProviderContainer(
    overrides: [
      monitorBridgeProvider.overrideWithValue(bridge),
      settingsControllerProvider.overrideWith(() => seed),
    ],
  );
  addTearDown(container.dispose);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: ConnectionsView())),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 20));
  return container;
}

double _nodeY(WidgetTester tester, String name) =>
    tester.getTopLeft(find.byKey(ValueKey('proxies-node-$name'))).dy;

bool _proxiesSwitch(WidgetTester tester) => tester
    .widget<Switch>(find.byKey(const ValueKey('proxies-auto-refresh')))
    .value;

bool _connectionsSwitch(WidgetTester tester) => tester
    .widget<Switch>(find.byKey(const ValueKey('connections-auto-refresh')))
    .value;

void main() {
  group('FLD-CFG-131 ProxiesSorting 失败回滚', () {
    testWidgets('保存失败→可见报错 + 保持延迟排序（不留存未持久值）', (tester) async {
      final seed = _WaveBSettings(_clashDoc(sorting: 0), failSave: true);
      final bridge = FakeMonitorBridge(
        clashApiSupported: true,
        proxies: <m.ClashProxyDto>[
          _node('A', 300),
          _node('B', 100),
          _node('C', 200),
        ],
        clashMode: 'Rule',
      );
      await _pumpProxies(tester, seed: seed, bridge: bridge);
      expect(find.text('排序: 延迟'), findsOneWidget);

      await tester.tap(find.byKey(const ValueKey('proxies-sort')));
      await tester.pump();

      expect(seed.saveAttempts, 1);
      expect(find.text('排序: 延迟'), findsOneWidget);
      expect(find.text('排序: 名称'), findsNothing);
      expect(find.textContaining('代理选项保存失败'), findsOneWidget);
      // 持久文档未被改写。
      expect(
        (seed.doc['ClashUIItem'] as Map<String, dynamic>)['ProxiesSorting'],
        0,
      );
      // 列表仍按延迟排序（B<C<A），未持久值未驱动展示。
      expect(_nodeY(tester, 'B'), lessThan(_nodeY(tester, 'C')));
      expect(_nodeY(tester, 'C'), lessThan(_nodeY(tester, 'A')));
      await tester.pumpWidget(const SizedBox());
    });

    testWidgets('保存成功→名称排序 + 重开保持', (tester) async {
      final seed = _WaveBSettings(_clashDoc(sorting: 0));
      final bridge = FakeMonitorBridge(
        clashApiSupported: true,
        proxies: <m.ClashProxyDto>[
          _node('C', 10),
          _node('A', 30),
          _node('B', 20),
        ],
        clashMode: 'Rule',
      );
      final container = await _pumpProxies(tester, seed: seed, bridge: bridge);

      await tester.tap(find.byKey(const ValueKey('proxies-sort')));
      await tester.pump();
      expect(find.text('排序: 名称'), findsOneWidget);
      expect(_nodeY(tester, 'A'), lessThan(_nodeY(tester, 'B')));
      expect(_nodeY(tester, 'B'), lessThan(_nodeY(tester, 'C')));

      await _reopenProxies(tester, container);
      expect(find.text('排序: 名称'), findsOneWidget);
      await tester.pumpWidget(const SizedBox());
    });
  });

  group('FLD-CFG-132 ProxiesAutoRefresh 失败回滚 + 轮询跟随 canonical', () {
    testWidgets('保存失败→开关回滚 + 可见报错 + 不启动轮询', (tester) async {
      final seed = _WaveBSettings(
        _clashDoc(proxiesAuto: false, proxiesInterval: 1),
        failSave: true,
      );
      final bridge = FakeMonitorBridge(
        clashApiSupported: true,
        proxies: <m.ClashProxyDto>[_node('A', 10)],
        clashMode: 'Rule',
      );
      await _pumpProxies(tester, seed: seed, bridge: bridge);
      expect(_proxiesSwitch(tester), isFalse);
      final before = bridge.clashProxiesCount;
      expect(before, greaterThanOrEqualTo(1));

      await tester.tap(find.byKey(const ValueKey('proxies-auto-refresh')));
      await tester.pump();

      expect(seed.saveAttempts, 1);
      expect(_proxiesSwitch(tester), isFalse, reason: '失败不得留下未持久的开位');
      expect(find.textContaining('代理选项保存失败'), findsOneWidget);
      expect(
        (seed.doc['ClashUIItem'] as Map<String, dynamic>)['ProxiesAutoRefresh'],
        isFalse,
      );
      // canonical（关）驱动：推进 1s 无新增轮询。
      await tester.pump(const Duration(seconds: 1));
      await tester.pump();
      expect(bridge.clashProxiesCount, before);
      await tester.pumpWidget(const SizedBox());
    });

    testWidgets('保存成功→开轮询 + 重开保持开位与轮询', (tester) async {
      final seed = _WaveBSettings(
        _clashDoc(proxiesAuto: false, proxiesInterval: 1),
      );
      final bridge = FakeMonitorBridge(
        clashApiSupported: true,
        proxies: <m.ClashProxyDto>[_node('A', 10)],
        clashMode: 'Rule',
      );
      final container = await _pumpProxies(tester, seed: seed, bridge: bridge);

      await tester.tap(find.byKey(const ValueKey('proxies-auto-refresh')));
      await tester.pump();
      expect(_proxiesSwitch(tester), isTrue);
      expect(find.textContaining('代理选项保存失败'), findsNothing);

      final before = bridge.clashProxiesCount;
      await tester.pump(const Duration(seconds: 1));
      await tester.pump();
      expect(bridge.clashProxiesCount, greaterThan(before));

      await _reopenProxies(tester, container);
      expect(_proxiesSwitch(tester), isTrue);
      final reopened = bridge.clashProxiesCount;
      await tester.pump(const Duration(seconds: 1));
      await tester.pump();
      expect(bridge.clashProxiesCount, greaterThan(reopened));
      await tester.pumpWidget(const SizedBox());
    });
  });

  group('FLD-CFG-133 ProxiesRefreshInterval 频率跟随 canonical', () {
    test('clashPollPeriod：开+正间隔才轮询，否则停', () {
      expect(
        clashPollPeriod(autoRefresh: true, intervalSeconds: 5),
        const Duration(seconds: 5),
      );
      expect(clashPollPeriod(autoRefresh: true, intervalSeconds: 0), isNull);
      expect(clashPollPeriod(autoRefresh: true, intervalSeconds: -1), isNull);
      expect(clashPollPeriod(autoRefresh: false, intervalSeconds: 5), isNull);
    });

    test('文档间隔解析：5s 落盘值进入 canonical', () {
      final config = clashUiConfigFromDocument(
        _clashDoc(proxiesAuto: true, proxiesInterval: 5),
      );
      expect(config.proxiesRefreshInterval, 5);
      expect(config.proxiesRefreshEnabled, isTrue);
      expect(
        clashPollPeriod(
          autoRefresh: config.proxiesAutoRefresh,
          intervalSeconds: config.proxiesRefreshInterval,
        ),
        const Duration(seconds: 5),
      );
    });

    testWidgets('开但间隔为 0→不轮询（非正间隔停轮询）', (tester) async {
      final seed = _WaveBSettings(
        _clashDoc(proxiesAuto: true, proxiesInterval: 0),
      );
      final bridge = FakeMonitorBridge(
        clashApiSupported: true,
        proxies: <m.ClashProxyDto>[_node('A', 10)],
        clashMode: 'Rule',
      );
      await _pumpProxies(tester, seed: seed, bridge: bridge);
      final before = bridge.clashProxiesCount;
      await tester.pump(const Duration(seconds: 1));
      await tester.pump();
      expect(bridge.clashProxiesCount, before);
      await tester.pumpWidget(const SizedBox());
    });
  });

  group('FLD-CFG-134 ConnectionsAutoRefresh 失败回滚 + 轮询跟随', () {
    testWidgets('保存失败→开关回滚 + 可见报错 + 不启动轮询', (tester) async {
      final seed = _WaveBSettings(
        _clashDoc(connectionsAuto: false, connectionsInterval: 1),
        failSave: true,
      );
      final bridge = FakeMonitorBridge(clashApiSupported: true);
      await _pumpConnections(tester, seed: seed, bridge: bridge);
      expect(_connectionsSwitch(tester), isFalse);
      final before = bridge.clashConnectionsCount;
      expect(before, greaterThanOrEqualTo(1));

      await tester.tap(find.byKey(const ValueKey('connections-auto-refresh')));
      await tester.pump();

      expect(seed.saveAttempts, 1);
      expect(_connectionsSwitch(tester), isFalse, reason: '失败不得留下未持久的开位');
      expect(find.textContaining('连接选项保存失败'), findsOneWidget);
      expect(
        (seed.doc['ClashUIItem']
            as Map<String, dynamic>)['ConnectionsAutoRefresh'],
        isFalse,
      );
      await tester.pump(const Duration(seconds: 1));
      await tester.pump();
      expect(bridge.clashConnectionsCount, before);
      await tester.pumpWidget(const SizedBox());
    });

    testWidgets('保存成功→开轮询', (tester) async {
      final seed = _WaveBSettings(
        _clashDoc(connectionsAuto: false, connectionsInterval: 1),
      );
      final bridge = FakeMonitorBridge(clashApiSupported: true);
      await _pumpConnections(tester, seed: seed, bridge: bridge);

      await tester.tap(find.byKey(const ValueKey('connections-auto-refresh')));
      await tester.pump();
      expect(_connectionsSwitch(tester), isTrue);

      final before = bridge.clashConnectionsCount;
      await tester.pump(const Duration(seconds: 1));
      await tester.pump();
      expect(bridge.clashConnectionsCount, greaterThan(before));
      await tester.pumpWidget(const SizedBox());
    });
  });

  group('FLD-CFG-135 ConnectionsRefreshInterval 频率跟随 canonical', () {
    test('文档间隔解析 + 非正间隔停轮询', () {
      final config = clashUiConfigFromDocument(
        _clashDoc(connectionsAuto: true, connectionsInterval: 5),
      );
      expect(config.connectionsRefreshInterval, 5);
      expect(
        clashPollPeriod(
          autoRefresh: config.connectionsAutoRefresh,
          intervalSeconds: config.connectionsRefreshInterval,
        ),
        const Duration(seconds: 5),
      );
      expect(clashPollPeriod(autoRefresh: true, intervalSeconds: 0), isNull);
    });

    testWidgets('开但间隔为 0→不轮询', (tester) async {
      final seed = _WaveBSettings(
        _clashDoc(connectionsAuto: true, connectionsInterval: 0),
      );
      final bridge = FakeMonitorBridge(clashApiSupported: true);
      await _pumpConnections(tester, seed: seed, bridge: bridge);
      final before = bridge.clashConnectionsCount;
      await tester.pump(const Duration(seconds: 1));
      await tester.pump();
      expect(bridge.clashConnectionsCount, before);
      await tester.pumpWidget(const SizedBox());
    });
  });
}
