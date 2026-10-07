// Wave B FLD-CFG-136: canonical `ClashUIItem.ConnectionsColumnItem` wiring.
//
// Synthetic-only: stubbed settings controller (persisted settings tree) +
// FakeMonitorBridge for the connection stream. No network/10808/system-proxy/
// user secrets. Covers the Name/Width/Index contract (unknown preserved on
// save, invalid widths fall back) plus the table wiring: canonical applied on
// load/reopen, live resync on an external save while open, and save->reopen
// roundtrip.
//
// Upstream (frozen 7d6a967): `ClashConnectionsView.xaml.cs:RestoreUI` (order
// by Index, only Width > 0 overrides) / `StorageUI` (Name/ActualWidth/
// DisplayIndex write-back); profiles G-15 (`MainColumnItem`) is the mirror
// pattern for unknown preservation.
import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/bridge/api/settings.dart' as settings;
import 'package:v2rayn_desktop/features/monitor/clash_ui_config.dart';
import 'package:v2rayn_desktop/features/monitor/connections_columns.dart';
import 'package:v2rayn_desktop/features/monitor/connections_view.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
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

/// Settings stand-in: fixed document; a successful saveGroup replaces the
/// group and notifies (so the open tab live-resyncs); [failSave] rejects the
/// write (fault injection). Never touches a real bridge.
class _WaveB136Settings extends SettingsController {
  _WaveB136Settings(this.doc);

  final Map<String, dynamic> doc;
  final List<Map<String, Object?>> savedGroups = <Map<String, Object?>>[];

  @override
  SettingsViewState build() =>
      SettingsViewState(loaded: true, revision: 1, document: doc);

  @override
  settings.SaveSettingsResult saveGroup(String group, Object? value) {
    savedGroups.add(<String, Object?>{'group': group, 'value': value});
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
  required _WaveB136Settings seedSettings,
}) async {
  addTearDown(bridge.disposeStreams);
  final container = ProviderContainer(
    overrides: [
      monitorBridgeProvider.overrideWithValue(bridge),
      settingsControllerProvider.overrideWith(() => seedSettings),
      runtimeBridgeProvider.overrideWithValue(
        _FakeRuntimeBridge(const RuntimeView()),
      ),
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
  group('FLD-CFG-136 canonical parse/apply/encode (synthetic)', () {
    test('unknown kept, empty name and bad types refused', () {
      final entries = parseConnectionColumnItems(<Object?>[
        <String, Object?>{'Name': 'Host', 'Width': 310, 'Index': 0},
        <String, Object?>{'Name': 'SynUnknown', 'Width': 99, 'Index': 1},
        <String, Object?>{'Name': '', 'Width': 50, 'Index': 2},
        <String, Object?>{'Name': 'BadWidth', 'Width': 'wide', 'Index': 3},
        <String, Object?>{'Name': 'BadIndex', 'Width': 50, 'Index': 'first'},
        'not-a-row',
      ]);
      expect(entries.map((e) => e.name), ['Host', 'SynUnknown']);
    });

    test('resolve: Index order, invalid widths fall back, unknowns hidden', () {
      final columns = resolveVisibleColumns(<Map<String, dynamic>>[
        <String, dynamic>{'Name': 'Elapsed', 'Width': 120, 'Index': 0},
        <String, dynamic>{'Name': 'Host', 'Width': 0, 'Index': 1},
        <String, dynamic>{'Name': 'Chain', 'Width': -5, 'Index': 2},
        <String, dynamic>{'Name': 'Network', 'Width': 'wide', 'Index': 3},
        <String, dynamic>{'Name': 'SynUnknown', 'Width': 99, 'Index': 0},
      ]);
      // Unknown names never reach the table; known order follows Index.
      expect(columns.any((c) => c.name == 'SynUnknown'), isFalse);
      expect(columns.map((c) => c.name).take(4), [
        'Elapsed',
        'Host',
        'Chain',
        'Network',
      ]);
      expect(columns.firstWhere((c) => c.name == 'Elapsed').width, 120);
      // Width 0 / negative / mistyped fall back to the upstream default.
      expect(columns.firstWhere((c) => c.name == 'Host').width, 300);
      expect(columns.firstWhere((c) => c.name == 'Chain').width, 500);
      expect(columns.firstWhere((c) => c.name == 'Network').width, 80);
      // Missing defaults keep their slot after the ordered ones.
      expect(columns.last.name, 'ProcessPath');
      expect(columns.map((c) => c.index).toList(), [0, 1, 2, 3, 4, 5]);
    });

    test('duplicate names: first Index wins; encode keeps unknowns once', () {
      final columns = resolveVisibleColumns(<Map<String, dynamic>>[
        <String, dynamic>{'Name': 'Host', 'Width': 200, 'Index': 5},
        <String, dynamic>{'Name': 'Host', 'Width': 310, 'Index': 0},
      ]);
      expect(columns.first.name, 'Host');
      expect(columns.first.width, 310);
      final encoded = encodeConnectionColumns(
        columns,
        preserveUnknownFrom: const [
          ConnectionColumnEntry(name: 'SynUnknown', width: 99, index: 7),
          ConnectionColumnEntry(name: 'Host', width: 1, index: 1),
        ],
      );
      expect(encoded.first['Name'], 'Host');
      expect(encoded.where((row) => row['Name'] == 'Host').length, 1);
      final unknown = encoded.where((row) => row['Name'] == 'SynUnknown');
      expect(unknown.length, 1);
      expect(unknown.single['Width'], 99);
    });

    test('clash_ui_config seam exposes entries + unknown-preserving save', () {
      final document = _docWithColumns(<Map<String, dynamic>>[
        <String, dynamic>{'Name': 'Host', 'Width': 310, 'Index': 0},
        <String, dynamic>{'Name': 'SynUnknown', 'Width': 99, 'Index': 1},
      ]);
      final entries = connectionsColumnEntriesFromDocument(document);
      expect(entries.map((e) => e.name), ['Host', 'SynUnknown']);
      final stored = encodeConnectionsColumnStorage(
        defaultConnectionColumns(),
        (document['ClashUIItem']
                as Map<String, dynamic>)['ConnectionsColumnItem']
            as List<Map<String, dynamic>>,
      );
      expect(stored.length, 7);
      expect(stored.last['Name'], 'SynUnknown');
      expect(stored.last['Width'], 99);
    });
  });

  group('FLD-CFG-136 table wiring (synthetic fake bridges)', () {
    testWidgets('load applies canonical order and widths', (tester) async {
      final seedSettings = _WaveB136Settings(
        _docWithColumns(<Map<String, dynamic>>[
          <String, dynamic>{'Name': 'Elapsed', 'Width': 120, 'Index': 0},
          <String, dynamic>{'Name': 'Host', 'Width': 310, 'Index': 1},
          <String, dynamic>{'Name': 'SynUnknown', 'Width': 99, 'Index': 0},
        ]),
      );
      await _pumpConnections(
        tester,
        bridge: FakeMonitorBridge(
          clashApiSupported: true,
          connections: <m.ClashConnectionDto>[_conn('a')],
        ),
        seedSettings: seedSettings,
      );
      expect(find.byKey(const ValueKey('connections-list')), findsOneWidget);
      expect(_headerX(tester, 'Elapsed'), lessThan(_headerX(tester, 'Host')));
      // Unknown canonical rows never reach the table headers.
      expect(find.text('SynUnknown'), findsNothing);
    });

    testWidgets('save preserves unknown rows; reopen applies them back', (
      tester,
    ) async {
      final seedSettings = _WaveB136Settings(
        _docWithColumns(<Map<String, dynamic>>[
          <String, dynamic>{'Name': 'Elapsed', 'Width': 120, 'Index': 0},
          <String, dynamic>{'Name': 'SynUnknown', 'Width': 99, 'Index': 3},
        ]),
      );
      await _pumpConnections(
        tester,
        bridge: FakeMonitorBridge(
          clashApiSupported: true,
          connections: <m.ClashConnectionDto>[_conn('a')],
        ),
        seedSettings: seedSettings,
      );
      await tester.tap(find.byKey(const ValueKey('connections-autofit')));
      await tester.pump();
      final writes = seedSettings.savedGroups
          .where((e) => e['group'] == 'ClashUIItem')
          .toList();
      expect(writes, isNotEmpty);
      final group = writes.last['value'] as Map<String, dynamic>;
      final rows = (group['ConnectionsColumnItem'] as List)
          .map((row) => Map<String, dynamic>.from(row as Map))
          .toList();
      // Autofit resets the six known columns and keeps the unknown row.
      expect(rows.length, 7);
      expect(rows.first['Name'], 'Host');
      expect(rows.first['Width'], 300);
      expect(rows.any((row) => row['Name'] == 'SynUnknown'), isTrue);

      // Independent reopen: the saved rows flow back through the same
      // restore path the view uses on open.
      final reopened = resolveVisibleColumns(rows);
      expect(
        reopened.map((c) => c.name).toList(),
        defaultConnectionColumns().map((c) => c.name).toList(),
      );
      expect(reopened.first.width, 300);
      await tester.pumpWidget(const SizedBox());
    });

    testWidgets('external save while open live-resyncs the table', (
      tester,
    ) async {
      final seedSettings = _WaveB136Settings(
        _docWithColumns(<Map<String, dynamic>>[
          <String, dynamic>{'Name': 'Host', 'Width': 300, 'Index': 0},
          <String, dynamic>{'Name': 'Elapsed', 'Width': 100, 'Index': 1},
        ]),
      );
      await _pumpConnections(
        tester,
        bridge: FakeMonitorBridge(
          clashApiSupported: true,
          connections: <m.ClashConnectionDto>[_conn('a')],
        ),
        seedSettings: seedSettings,
      );
      expect(_headerX(tester, 'Host'), lessThan(_headerX(tester, 'Elapsed')));

      // External canonical save through the real group-save seam.
      seedSettings.saveGroup('ClashUIItem', <String, dynamic>{
        'ConnectionsAutoRefresh': false,
        'ConnectionsRefreshInterval': 0,
        'ConnectionsColumnItem': <Map<String, dynamic>>[
          <String, dynamic>{'Name': 'Elapsed', 'Width': 120, 'Index': 0},
          <String, dynamic>{'Name': 'Host', 'Width': 310, 'Index': 1},
        ],
      });
      await tester.pump();
      expect(_headerX(tester, 'Elapsed'), lessThan(_headerX(tester, 'Host')));
      await tester.pumpWidget(const SizedBox());
    });
  });
}
