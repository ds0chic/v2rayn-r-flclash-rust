// SP-20 完整卡：连接列排列/宽度持久化重开保持 + 右键关闭 + 真实连接管理。
//
// 原版对照（v2rayN 7.25.4 / 7d6a967，只读 work/）：
// `v2rayN/Views/ClashConnectionsView.xaml` 默认列宽 Host=300 / Chain=500 /
// Network=80 / Type=160 / ProcessPath=100 / Elapsed=100（ExName 为稳定列键）；
// `ClashConnectionsView.xaml.cs` RestoreUI 按 ConnectionsColumnItem.Index 排序
// 恢复（Width>0 才覆盖），StorageUI 退出时按 Name/ActualWidth/DisplayIndex 回写，
// 另有列宽自适应按钮；`ServiceLib/ViewModels/ClashConnectionsViewModel.cs`
// 右键关闭冻结 SelectedSource.Id（空 Id 不可执行，对应 canEditRemove），关闭
// 全部走独立空 id 路径，关闭后重查连接。
//
// 本文件覆盖完整用户流程：列布局恢复/排列/回写重开、右键菜单冻结目标、关闭
// 失败不假成功、换 session 迟到不误关。合成数据（*.example.invalid），无
// socket/代理/10808 操作；Fake 仅做故障注入（关闭失败、关闭中切换会话）。
import 'dart:async';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/bridge/api/settings.dart' as settings;
import 'package:v2rayn_desktop/features/monitor/clash_ui_config.dart';
import 'package:v2rayn_desktop/features/monitor/connections_columns.dart';
import 'package:v2rayn_desktop/features/monitor/connections_view.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

import '../support/fake_monitor_bridge.dart';

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

/// Settings 替身：固定文档 + 记录 saveGroup（列回写断言用），不触真实桥。
class _Sp20Settings extends SettingsController {
  _Sp20Settings(this.doc);

  final Map<String, dynamic> doc;
  final List<Map<String, Object?>> savedGroups = <Map<String, Object?>>[];

  @override
  SettingsViewState build() =>
      SettingsViewState(loaded: true, revision: 1, document: doc);

  @override
  settings.SaveSettingsResult saveGroup(String group, Object? value) {
    savedGroups.add(<String, Object?>{'group': group, 'value': value});
    doc[group] = value;
    return const settings.SaveSettingsResult(
      ok: true,
      changes: <settings.SettingsChangeDto>[],
      restartCoreFields: <String>[],
      restartAppFields: <String>[],
      nextLaunchFields: <String>[],
    );
  }
}

/// 单条关闭恒失败的故障注入（不断言假成功）。
class _FailCloseBridge extends FakeMonitorBridge {
  _FailCloseBridge({required List<m.ClashConnectionDto> connections})
    : super(clashApiSupported: true, connections: connections);

  @override
  Future<m.MonitorActionResult> closeClashConnection(String id) async =>
      const m.MonitorActionResult(ok: false, supported: true, message: 'boom');
}

/// 关闭在途可挂起的桥（换 session 迟到场景用）。
class _GatedCloseBridge extends FakeMonitorBridge {
  _GatedCloseBridge({required List<m.ClashConnectionDto> connections})
    : super(clashApiSupported: true, connections: connections);

  Completer<void>? closeGate;

  @override
  Future<m.MonitorActionResult> closeClashConnection(String id) async {
    final gate = closeGate;
    if (gate != null) await gate.future;
    return super.closeClashConnection(id);
  }
}

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

Map<String, dynamic> _docWithColumns(List<Map<String, dynamic>> rows) =>
    <String, dynamic>{
      'ClashUIItem': <String, dynamic>{
        'ConnectionsAutoRefresh': false,
        'ConnectionsRefreshInterval': 0,
        'ConnectionsColumnItem': rows,
      },
    };

Future<ProviderContainer> _pumpConnections(
  WidgetTester tester, {
  required FakeMonitorBridge bridge,
  required _Sp20Settings seedSettings,
}) async {
  addTearDown(bridge.disposeStreams);
  final container = ProviderContainer(
    overrides: [
      monitorBridgeProvider.overrideWithValue(bridge),
      settingsControllerProvider.overrideWith(() => seedSettings),
      runtimeBridgeProvider.overrideWithValue(_FakeRuntimeBridge(_runningA)),
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

double _headerX(WidgetTester tester, String name) =>
    tester.getTopLeft(find.text(name)).dx;

void main() {
  group('SP-20: column reorder is pure and re-indexed', () {
    test('moveConnectionColumn moves and re-numbers 0..n-1', () {
      final cols = defaultConnectionColumns();
      final moved = moveConnectionColumn(cols, 0, 2);
      expect(moved.map((c) => c.name).take(3).toList(), <String>[
        'Chain',
        'Network',
        'Host',
      ]);
      expect(moved.map((c) => c.index).toList(), <int>[0, 1, 2, 3, 4, 5]);
      // Widths travel with their column.
      expect(moved.firstWhere((c) => c.name == 'Host').width, 300);
      // Out-of-range targets clamp instead of throwing.
      final clamped = moveConnectionColumn(cols, 5, 99);
      expect(clamped.last.name, 'Elapsed');
    });

    test('config keeps persisted ConnectionsColumnItem rows', () {
      final config = clashUiConfigFromDocument(
        _docWithColumns(<Map<String, dynamic>>[
          <String, dynamic>{'Name': 'Elapsed', 'Width': 120, 'Index': 0},
        ]),
      );
      expect(config.connectionsColumns.length, 1);
      expect(config.connectionsColumns.first['Name'], 'Elapsed');
      expect(
        clashUiConfigFromDocument(<String, dynamic>{}).connectionsColumns,
        isEmpty,
      );
    });
  });

  group('SP-20: close target freeze and generation guard', () {
    test('empty id is never sent (canEditRemove parity)', () async {
      final bridge = FakeMonitorBridge(
        clashApiSupported: true,
        connections: <m.ClashConnectionDto>[_conn('a')],
      );
      final runtime = _FakeRuntimeBridge(_runningA);
      final container = ProviderContainer(
        overrides: [
          monitorBridgeProvider.overrideWithValue(bridge),
          runtimeBridgeProvider.overrideWithValue(runtime),
        ],
      );
      addTearDown(container.dispose);
      addTearDown(bridge.disposeStreams);
      final controller = container.read(monitorControllerProvider.notifier);

      expect(await controller.closeConnection(''), isFalse);
      expect(bridge.closedConnections, isEmpty);
    });

    test('close failure is not reported as success', () async {
      final bridge = _FailCloseBridge(
        connections: <m.ClashConnectionDto>[_conn('a'), _conn('b')],
      );
      final container = ProviderContainer(
        overrides: [
          monitorBridgeProvider.overrideWithValue(bridge),
          runtimeBridgeProvider.overrideWithValue(
            _FakeRuntimeBridge(_runningA),
          ),
        ],
      );
      addTearDown(container.dispose);
      addTearDown(bridge.disposeStreams);
      final controller = container.read(monitorControllerProvider.notifier);
      await controller.refreshConnections();
      expect(container.read(monitorControllerProvider).connections.length, 2);

      expect(await controller.closeConnection('a'), isFalse);
      // 失败不重查、不丢行：列表保持原样。
      expect(
        container
            .read(monitorControllerProvider)
            .connections
            .map((c) => c.id)
            .toList(),
        <String>['a', 'b'],
      );
    });

    test('late close after a session switch claims no success', () async {
      final bridge = _GatedCloseBridge(
        connections: <m.ClashConnectionDto>[_conn('a')],
      )..closeGate = Completer<void>();
      final container = ProviderContainer(
        overrides: [
          monitorBridgeProvider.overrideWithValue(bridge),
          runtimeBridgeProvider.overrideWithValue(
            _FakeRuntimeBridge(_runningA),
          ),
        ],
      );
      addTearDown(container.dispose);
      addTearDown(bridge.disposeStreams);
      final controller = container.read(monitorControllerProvider.notifier);
      controller.syncRuntimeSession(_runningA);

      final pending = controller.closeConnection('a');
      // 关闭在途时会话切换：迟到回包不得作用于新会话。
      controller.syncRuntimeSession(_runningB);
      bridge.closeGate!.complete();
      expect(await pending, isFalse);
      expect(container.read(monitorControllerProvider).connections, isEmpty);
    });
  });

  group('SP-20: connections view layout and context menu', () {
    testWidgets('persisted order is restored (Elapsed first)', (tester) async {
      final settings = _Sp20Settings(
        _docWithColumns(<Map<String, dynamic>>[
          <String, dynamic>{'Name': 'Elapsed', 'Width': 120, 'Index': 0},
          <String, dynamic>{'Name': 'Host', 'Width': 310, 'Index': 1},
        ]),
      );
      await _pumpConnections(
        tester,
        bridge: FakeMonitorBridge(
          clashApiSupported: true,
          connections: <m.ClashConnectionDto>[_conn('a')],
        ),
        seedSettings: settings,
      );
      expect(find.byKey(const ValueKey('connections-list')), findsOneWidget);
      expect(_headerX(tester, 'Elapsed'), lessThan(_headerX(tester, 'Host')));
    });

    testWidgets('right-click closes the frozen row id', (tester) async {
      final settings = _Sp20Settings(
        _docWithColumns(const <Map<String, dynamic>>[]),
      );
      final bridge = FakeMonitorBridge(
        clashApiSupported: true,
        connections: <m.ClashConnectionDto>[_conn('a'), _conn('b')],
      );
      await _pumpConnections(tester, bridge: bridge, seedSettings: settings);

      await tester.tap(
        find.text('host-b.example.invalid:443'),
        buttons: kSecondaryButton,
      );
      await tester.pump();
      await tester.pumpAndSettle();
      await tester.tap(find.text('关闭连接'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 20));
      expect(bridge.closedConnections, <String>['b']);
    });

    testWidgets('right-click menu also offers close-all', (tester) async {
      final settings = _Sp20Settings(
        _docWithColumns(const <Map<String, dynamic>>[]),
      );
      final bridge = FakeMonitorBridge(
        clashApiSupported: true,
        connections: <m.ClashConnectionDto>[_conn('a')],
      );
      await _pumpConnections(tester, bridge: bridge, seedSettings: settings);

      await tester.tap(
        find.text('host-a.example.invalid:443'),
        buttons: kSecondaryButton,
      );
      await tester.pump();
      await tester.pumpAndSettle();
      // The toolbar button and the menu item share the text; the overlay menu
      // paints last.
      await tester.tap(find.text('关闭全部').last);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 20));
      expect(bridge.closeAllCount, 1);
    });

    testWidgets('autofit resets to upstream defaults and persists', (
      tester,
    ) async {
      final settings = _Sp20Settings(
        _docWithColumns(<Map<String, dynamic>>[
          <String, dynamic>{'Name': 'Elapsed', 'Width': 120, 'Index': 0},
          <String, dynamic>{'Name': 'Host', 'Width': 310, 'Index': 1},
        ]),
      );
      await _pumpConnections(
        tester,
        bridge: FakeMonitorBridge(
          clashApiSupported: true,
          connections: <m.ClashConnectionDto>[_conn('a')],
        ),
        seedSettings: settings,
      );
      await tester.tap(find.byKey(const ValueKey('connections-autofit')));
      await tester.pump();
      expect(_headerX(tester, 'Host'), lessThan(_headerX(tester, 'Chain')));
      final writes = settings.savedGroups
          .where((e) => e['group'] == 'ClashUIItem')
          .toList();
      expect(writes, isNotEmpty);
      final group = writes.last['value'] as Map<String, dynamic>;
      final rows = group['ConnectionsColumnItem'] as List;
      expect(rows.length, 6);
      expect(rows.first['Name'], 'Host');
      expect(rows.first['Width'], 300);
    });

    testWidgets('autofit write survives an independent reopen', (tester) async {
      final settings = _Sp20Settings(
        _docWithColumns(const <Map<String, dynamic>>[]),
      );
      await _pumpConnections(
        tester,
        bridge: FakeMonitorBridge(
          clashApiSupported: true,
          connections: <m.ClashConnectionDto>[_conn('a')],
        ),
        seedSettings: settings,
      );
      await tester.tap(find.byKey(const ValueKey('connections-autofit')));
      await tester.pump();
      expect(_headerX(tester, 'Host'), lessThan(_headerX(tester, 'Chain')));
      final writes = settings.savedGroups
          .where((e) => e['group'] == 'ClashUIItem')
          .toList();
      expect(writes, isNotEmpty);
      final group = writes.last['value'] as Map<String, dynamic>;
      // Independent reopen: feed the stored document rows back through the
      // same restore path the view uses on open.
      final reopened = resolveVisibleColumns(
        (group['ConnectionsColumnItem'] as List)
            .map((row) => Map<String, dynamic>.from(row as Map))
            .toList(),
      );
      expect(
        reopened.map((c) => c.name).toList(),
        defaultConnectionColumns().map((c) => c.name).toList(),
      );
      expect(reopened.first.width, 300);
    });
  });
}
