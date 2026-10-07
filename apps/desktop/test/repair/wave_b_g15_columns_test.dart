// Wave B G-15 / FLD-CFG-117/118/119: canonical `UiItem.MainColumnItem` wiring.
//
// Synthetic-only: the fake bridge stands in for the persisted settings tree,
// no network/10808/system-proxy/user secrets. Covers the pure Name/Width/Index
// contract (unknown preserved, invalid widths fall back) plus the controller
// wiring: canonical applied on load/reopen and after save, UI-cache fallback
// when no canonical layout is persisted.
//
// Upstream (frozen 7d6a967): `ProfilesView.xaml.cs:RestoreUI/StorageUI` —
// order by Index, Width < 0 hides, `to*` follows `GuiItem.EnableStatistics`,
// `IpInfo` follows `SpeedTestItem.IPAPIUrl` + `UiItem.HideColumnIpInfo`.
import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/settings.dart' as settings;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/main_column_layout.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';
import 'package:v2rayn_desktop/features/profiles/sub_entry.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

/// Settings-group write always rejected (canonical-save failure path).
class G15FailingBridge extends SyntheticBridgePort {
  @override
  settings.SaveSettingsResult saveSettingsGroup(
    String group,
    String patchJson,
    int expectedRevision,
  ) {
    return settings.SaveSettingsResult(
      ok: false,
      changes: const [],
      restartCoreFields: const [],
      restartAppFields: const [],
      nextLaunchFields: const [],
      error: const c.ErrorDto(
        code: 'E_REVISION_STALE',
        messageKey: 'error.revision_stale',
        retryable: false,
      ),
    );
  }
}

ProviderContainer g15Container(BridgePort bridge, UiStateStore store) =>
    ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(bridge),
        uiStateStoreProvider.overrideWithValue(store),
        profileRowCountProvider.overrideWithValue(10),
      ],
    );

/// Seed the canonical `UiItem.MainColumnItem` rows on a fake bridge through
/// the real group-save seam (full-group replace, like the engine).
void seedCanonical(
  BridgePort bridge,
  List<Map<String, Object?>> rows, {
  bool enableStatistics = true,
  String? ipApiUrl = 'https://example.com/ip',
  bool hideColumnIpInfo = false,
}) {
  final load = bridge.getSettings();
  expect(load.ok, isTrue);
  final document = jsonDecode(load.settingsJson) as Map<String, dynamic>;
  final ui = Map<String, dynamic>.from(document['UiItem'] as Map);
  ui['MainColumnItem'] = rows;
  ui['HideColumnIpInfo'] = hideColumnIpInfo;
  final gui = Map<String, dynamic>.from(document['GuiItem'] as Map);
  gui['EnableStatistics'] = enableStatistics;
  final speed = Map<String, dynamic>.from(document['SpeedTestItem'] as Map);
  speed['IPAPIUrl'] = ipApiUrl;
  final revision = decodeGroupRevisions(load.groupRevisionsJson)['UiItem'] ?? 0;
  expect(
    bridge.saveSettingsGroup('UiItem', jsonEncode(ui), revision).ok,
    isTrue,
  );
  final guiRevision =
      decodeGroupRevisions(
        bridge.getSettings().groupRevisionsJson,
      )['GuiItem'] ??
      0;
  expect(
    bridge.saveSettingsGroup('GuiItem', jsonEncode(gui), guiRevision).ok,
    isTrue,
  );
  final speedLoad = bridge.getSettings();
  final speedRevision =
      decodeGroupRevisions(speedLoad.groupRevisionsJson)['SpeedTestItem'] ?? 0;
  expect(
    bridge
        .saveSettingsGroup('SpeedTestItem', jsonEncode(speed), speedRevision)
        .ok,
    isTrue,
  );
}

List<Map<String, dynamic>> readCanonical(BridgePort bridge) {
  final load = bridge.getSettings();
  final document = jsonDecode(load.settingsJson) as Map<String, dynamic>;
  final ui = document['UiItem'] as Map<String, dynamic>;
  return (ui['MainColumnItem'] as List).cast<Map<String, dynamic>>();
}

Map<String, dynamic> byName(List<Map<String, dynamic>> rows, String name) =>
    rows.firstWhere((row) => row['Name'] == name);

void main() {
  group('G-15 canonical parse/apply/encode (synthetic)', () {
    test('FLD-CFG-117: unknown kept, empty name and bad types refused', () {
      final entries = parseMainColumnItems(<Object?>[
        <String, Object?>{'Name': 'Remarks', 'Width': 220, 'Index': 0},
        <String, Object?>{'Name': 'SynUnknown', 'Width': 99, 'Index': 1},
        <String, Object?>{'Name': '', 'Width': 50, 'Index': 2},
        <String, Object?>{'Name': 'BadWidth', 'Width': 'wide', 'Index': 3},
        <String, Object?>{'Name': 'BadIndex', 'Width': 50, 'Index': 'first'},
        'not-a-row',
      ]);
      expect(entries.map((e) => e.name), ['Remarks', 'SynUnknown']);
    });

    test('FLD-CFG-118/119: order by Index, hidden sentinel, auto+clamp', () {
      final columns = applyMainColumnLayout(
        defaultProfileColumns(),
        const [
          MainColumnEntry(name: 'Port', width: 300, index: 0),
          MainColumnEntry(name: 'Remarks', width: -1, index: 1),
          MainColumnEntry(name: 'Address', width: 0, index: 2),
          MainColumnEntry(name: 'Network', width: 5000, index: 3),
          MainColumnEntry(name: 'StreamSecurity', width: 5, index: 4),
          MainColumnEntry(name: 'SynUnknown', width: 99, index: 0),
        ],
        showStatistics: true,
        showIpInfo: true,
      );
      // Unknown names never reach the table; known order follows Index.
      expect(columns.any((c) => c.key == 'SynUnknown'), isFalse);
      expect(columns.map((c) => c.key).take(5), [
        'Port',
        'Remarks',
        'Address',
        'Network',
        'StreamSecurity',
      ]);
      expect(columns.firstWhere((c) => c.key == 'Port').width, 300);
      final remarks = columns.firstWhere((c) => c.key == 'Remarks');
      expect(remarks.visible, isFalse);
      expect(remarks.width, 150);
      // Width 0 (auto) falls back to the table default, stays visible.
      final address = columns.firstWhere((c) => c.key == 'Address');
      expect(address.visible, isTrue);
      expect(address.width, 120);
      // Out-of-range widths clamp to the table bounds.
      expect(columns.firstWhere((c) => c.key == 'Network').width, 600);
      expect(columns.firstWhere((c) => c.key == 'StreamSecurity').width, 40);
      // Missing known keys keep their default slot after the ordered ones.
      expect(columns.last.key, 'TotalDown');
    });

    test('duplicate names: first Index wins, encode keeps unknowns once', () {
      final columns = applyMainColumnLayout(
        defaultProfileColumns(),
        const [
          MainColumnEntry(name: 'Port', width: 200, index: 5),
          MainColumnEntry(name: 'Port', width: 300, index: 0),
        ],
        showStatistics: true,
        showIpInfo: true,
      );
      expect(columns.first.key, 'Port');
      expect(columns.first.width, 300);
      final encoded = encodeMainColumnItems(
        columns,
        preserveUnknownFrom: const [
          MainColumnEntry(name: 'SynUnknown', width: 99, index: 7),
          MainColumnEntry(name: 'Port', width: 1, index: 1),
        ],
      );
      expect(encoded.first['Name'], 'Port');
      expect(encoded.first['Width'], 300);
      expect(encoded.first['Index'], 0);
      final unknown = encoded.where((row) => row['Name'] == 'SynUnknown');
      expect(unknown.length, 1);
      expect(unknown.single['Width'], 99);
      expect(encoded.where((row) => row['Name'] == 'Port').length, 1);
    });

    test('hidden columns encode the -1 sentinel with display position', () {
      final columns = applyMainColumnLayout(
        defaultProfileColumns(),
        const [MainColumnEntry(name: 'Remarks', width: -1, index: 0)],
        showStatistics: true,
        showIpInfo: true,
      );
      final encoded = encodeMainColumnItems(columns);
      expect(encoded.first['Name'], 'Remarks');
      expect(encoded.first['Width'], -1);
      expect(encoded.first['Index'], 0);
      expect(encoded[1]['Index'], 1);
    });
  });

  group('G-15 table wiring (synthetic fake bridge)', () {
    test('FLD-CFG-117/118/119: build applies canonical over the UI cache', () {
      final bridge = SyntheticBridgePort();
      final store = MemoryUiStateStore();
      seedCanonical(bridge, [
        {'Name': 'Port', 'Width': 300, 'Index': 0},
        {'Name': 'Remarks', 'Width': 220, 'Index': 1},
        {'Name': 'StreamSecurity', 'Width': -1, 'Index': 2},
        {'Name': 'SynUnknown', 'Width': 99, 'Index': 1},
      ]);
      // Conflicting local cache must lose to the canonical layout.
      store.saveSection(ProfilesController.columnSection, {
        'order': ['Remarks', 'Port'],
        'visible': {'Remarks': true, 'Port': true, 'StreamSecurity': true},
        'widths': {'Remarks': 111.0, 'Port': 111.0},
      });

      final container = g15Container(bridge, store);
      addTearDown(container.dispose);
      final columns = container.read(profilesControllerProvider).columns;
      expect(columns.map((c) => c.key).take(3), [
        'Port',
        'Remarks',
        'StreamSecurity',
      ]);
      expect(columns.firstWhere((c) => c.key == 'Port').width, 300);
      expect(columns.firstWhere((c) => c.key == 'Remarks').width, 220);
      final hidden = columns.firstWhere((c) => c.key == 'StreamSecurity');
      expect(hidden.visible, isFalse);
      expect(columns.any((c) => c.key == 'SynUnknown'), isFalse);
    });

    test('no canonical persisted: UI cache still applies (legacy path)', () {
      final bridge = SyntheticBridgePort();
      final store = MemoryUiStateStore();
      store.saveSection(ProfilesController.columnSection, {
        'order': ['Port', 'Remarks'],
        'visible': {'Port': true, 'Remarks': false},
        'widths': {'Port': 222.0},
      });
      final container = g15Container(bridge, store);
      addTearDown(container.dispose);
      final columns = container.read(profilesControllerProvider).columns;
      expect(columns.map((c) => c.key).take(2), ['Port', 'Remarks']);
      expect(columns.firstWhere((c) => c.key == 'Port').width, 222);
      expect(columns.firstWhere((c) => c.key == 'Remarks').visible, isFalse);
    });

    test('resize writes canonical, preserves unknown, reopen applies', () {
      final bridge = SyntheticBridgePort();
      final store = MemoryUiStateStore();
      seedCanonical(bridge, [
        {'Name': 'Remarks', 'Width': 150, 'Index': 0},
        {'Name': 'SynUnknown', 'Width': 99, 'Index': 5},
      ]);
      final first = g15Container(bridge, store);
      addTearDown(first.dispose);
      first
          .read(profilesControllerProvider.notifier)
          .resizeColumn('Remarks', 40);

      final saved = readCanonical(bridge);
      expect(byName(saved, 'Remarks')['Width'], 190);
      // Unknown rows survive the rewrite (FLD-CFG-117).
      expect(byName(saved, 'SynUnknown')['Width'], 99);
      // Hidden columns persist the -1 sentinel (upstream StorageUI).
      first
          .read(profilesControllerProvider.notifier)
          .toggleColumnVisibility('Port');
      expect(byName(readCanonical(bridge), 'Port')['Width'], -1);

      final second = g15Container(bridge, store);
      addTearDown(second.dispose);
      final reopened = second.read(profilesControllerProvider).columns;
      expect(reopened.firstWhere((c) => c.key == 'Remarks').width, 190);
      expect(reopened.firstWhere((c) => c.key == 'Port').visible, isFalse);
    });

    test('move order round-trips through canonical to reopen', () {
      final bridge = SyntheticBridgePort();
      final store = MemoryUiStateStore();
      seedCanonical(bridge, [
        {'Name': 'ConfigType', 'Width': 80, 'Index': 0},
        {'Name': 'Remarks', 'Width': 150, 'Index': 1},
        {'Name': 'Address', 'Width': 120, 'Index': 2},
      ]);
      final first = g15Container(bridge, store);
      addTearDown(first.dispose);
      first.read(profilesControllerProvider.notifier).moveColumn('Address', -2);
      final order = (first.read(profilesControllerProvider).columns)
          .map((c) => c.key)
          .take(3)
          .toList();
      expect(order, ['Address', 'ConfigType', 'Remarks']);
      final saved = readCanonical(bridge);
      expect(byName(saved, 'Address')['Index'], 0);

      final second = g15Container(bridge, store);
      addTearDown(second.dispose);
      final reopened = second
          .read(profilesControllerProvider)
          .columns
          .map((c) => c.key)
          .take(3)
          .toList();
      expect(reopened, ['Address', 'ConfigType', 'Remarks']);
    });

    test('resync picks up an external canonical save; false when absent', () {
      final bridge = SyntheticBridgePort();
      final store = MemoryUiStateStore();
      final container = g15Container(bridge, store);
      addTearDown(container.dispose);
      final notifier = container.read(profilesControllerProvider.notifier);
      // No canonical rows persisted yet: untouched, false.
      expect(notifier.resyncColumnsFromCanonical(), isFalse);

      seedCanonical(bridge, [
        {'Name': 'Port', 'Width': 321, 'Index': 0},
      ]);
      expect(notifier.resyncColumnsFromCanonical(), isTrue);
      final columns = container.read(profilesControllerProvider).columns;
      expect(columns.first.key, 'Port');
      expect(columns.first.width, 321);
    });

    test('canonical save failure keeps optimistic state + mirror + log', () {
      final bridge = G15FailingBridge();
      final store = MemoryUiStateStore();
      final container = g15Container(bridge, store);
      addTearDown(container.dispose);
      final notifier = container.read(profilesControllerProvider.notifier);
      notifier.resizeColumn('Remarks', 40);
      final state = container.read(profilesControllerProvider);
      expect(
        state.columns.firstWhere((c) => c.key == 'Remarks').width,
        greaterThan(150),
      );
      // Local mirror still written; failure is logged, never a silent revert.
      final widths =
          store.loadSection(ProfilesController.columnSection)!['widths'] as Map;
      expect((widths['Remarks'] as num).toDouble(), greaterThan(150));
      expect(state.events.any((e) => e.action == 'column-save-failed'), isTrue);
    });

    test('statistics/IpInfo gates force visibility like upstream', () {
      final bridge = SyntheticBridgePort();
      final store = MemoryUiStateStore();
      seedCanonical(
        bridge,
        [
          {'Name': 'TodayUp', 'Width': 120, 'Index': 0},
          {'Name': 'IpInfo', 'Width': 120, 'Index': 1},
        ],
        enableStatistics: false,
        ipApiUrl: null,
      );
      final container = g15Container(bridge, store);
      addTearDown(container.dispose);
      final columns = container.read(profilesControllerProvider).columns;
      expect(columns.first.key, 'TodayUp');
      expect(columns.firstWhere((c) => c.key == 'TodayUp').visible, isFalse);
      expect(columns.firstWhere((c) => c.key == 'IpInfo').visible, isFalse);
    });
  });
}
