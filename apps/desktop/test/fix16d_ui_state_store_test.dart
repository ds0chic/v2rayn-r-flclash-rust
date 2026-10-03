// FIX-16D: window geometry (by upstream TypeName) and table column widths
// (node list + Clash connections) must share one consistent, migratable state
// source so a backup/restore never loses them.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

void main() {
  group('WindowGeometry', () {
    test('round-trips per TypeName and restores all rows', () {
      final store = MemoryUiStateStore();
      store.saveWindowGeometry(
        const WindowGeometry(
          typeName: WindowTypeNames.main,
          width: 1280,
          height: 800,
          mainGridHeight1: 300,
          mainGridHeight2: 500,
          orientation: 2,
        ),
      );
      store.saveWindowGeometry(
        const WindowGeometry(
          typeName: WindowTypeNames.dnsSetting,
          width: 720,
          height: 540,
        ),
      );

      final main = store.loadWindowGeometry(WindowTypeNames.main)!;
      expect(main.width, 1280);
      expect(main.height, 800);
      expect(main.mainGridHeight1, 300);
      expect(main.mainGridHeight2, 500);
      expect(main.orientation, 2);
      expect(main.isValid, isTrue);

      final all = store.loadWindowGeometries();
      expect(all.keys, containsAll(<String>['MainWindow', 'DNSSettingWindow']));
      expect(all[WindowTypeNames.dnsSetting]!.height, 540);

      // Saving the same TypeName updates in place instead of duplicating.
      store.saveWindowGeometry(main.copyWith(width: 1600, height: 900));
      expect(store.loadWindowGeometries().length, 2);
      expect(store.loadWindowGeometry(WindowTypeNames.main)!.width, 1600);
    });

    test('invalid rows are not restored', () {
      final store = MemoryUiStateStore();
      store.saveWindowGeometry(
        const WindowGeometry(
          typeName: WindowTypeNames.main,
          width: 0,
          height: 0,
        ),
      );
      expect(store.loadWindowGeometry(WindowTypeNames.main)!.isValid, isFalse);
    });

    test('migrates the legacy flat window blob to MainWindow', () {
      final store = MemoryUiStateStore()
        ..saveWindowState(<String, dynamic>{'width': 1024, 'height': 768});
      final main = store.loadWindowGeometry(WindowTypeNames.main);
      expect(main, isNotNull);
      expect(main!.width, 1024);
      expect(main.height, 768);
    });

    test('migrates a TypeName-keyed legacy blob', () {
      final store = MemoryUiStateStore()
        ..saveWindowState(<String, dynamic>{
          'OptionSettingWindow': <String, dynamic>{
            'Width': 900,
            'Height': 600,
            'TypeName': 'OptionSettingWindow',
          },
        });
      final option = store.loadWindowGeometry(WindowTypeNames.optionSetting);
      expect(option, isNotNull);
      expect(option!.width, 900);
      expect(option.height, 600);
    });
  });

  group('ColumnLayout', () {
    test('node table and Clash connections share one shape and source', () {
      final store = MemoryUiStateStore();
      store.saveColumnLayout(
        ColumnTable.profiles,
        const ColumnLayout(
          order: <String>['Remarks', 'Address', 'Port'],
          visible: <String, bool>{'Remarks': true, 'Port': false},
          widths: <String, double>{'Remarks': 200, 'Port': 80},
        ),
      );
      store.saveColumnLayout(
        ColumnTable.clashConnections,
        const ColumnLayout(
          order: <String>['Host', 'Chain', 'Elapsed'],
          visible: <String, bool>{'Host': true},
          widths: <String, double>{'Host': 180},
        ),
      );

      final profiles = store.loadColumnLayout(ColumnTable.profiles);
      expect(profiles.order, <String>['Remarks', 'Address', 'Port']);
      expect(profiles.visible['Port'], isFalse);
      expect(profiles.widths['Remarks'], 200);

      final connections = store.loadColumnLayout(ColumnTable.clashConnections);
      expect(connections.order.first, 'Host');
      expect(connections.widths['Host'], 180);

      // Distinct sections so the two tables never overwrite each other.
      expect(
        UiStateStore.columnSectionFor(ColumnTable.profiles),
        isNot(UiStateStore.columnSectionFor(ColumnTable.clashConnections)),
      );
    });

    test('migrates legacy flat column_widths into the profiles layout', () {
      final store = MemoryUiStateStore()
        ..saveColumnWidths(<String, double>{'Remarks': 180, 'Port': 90});
      final profiles = store.loadColumnLayout(ColumnTable.profiles);
      expect(profiles.widths['Remarks'], 180);
      expect(profiles.widths['Port'], 90);
    });

    test('unknown table falls back to an empty layout', () {
      final layout = MemoryUiStateStore().loadColumnLayout('nope');
      expect(layout.isEmpty, isTrue);
    });
  });

  group('migrateLegacyUiState', () {
    test('folds legacy blobs and records the schema version', () {
      final store = MemoryUiStateStore()
        ..saveColumnWidths(<String, double>{'Remarks': 180})
        ..saveWindowState(<String, dynamic>{'width': 1024, 'height': 768});

      store.migrateLegacyUiState();

      expect(
        store.loadColumnLayout(ColumnTable.profiles).widths['Remarks'],
        180,
      );
      expect(store.loadWindowGeometry(WindowTypeNames.main)!.width, 1024);
      expect(
        store.loadSection(UiStateStore.metaSection)!['schema_version'],
        UiStateStore.schemaVersion,
      );

      // Idempotent: a second run does not clobber canonical values.
      store.saveWindowGeometry(
        const WindowGeometry(
          typeName: WindowTypeNames.main,
          width: 1440,
          height: 900,
        ),
      );
      store.migrateLegacyUiState();
      expect(store.loadWindowGeometry(WindowTypeNames.main)!.width, 1440);
    });
  });

  test('backup document round-trips windows and both column tables', () {
    final source = MemoryUiStateStore()
      ..saveWindowGeometry(
        const WindowGeometry(
          typeName: WindowTypeNames.main,
          width: 1366,
          height: 768,
        ),
      )
      ..saveColumnLayout(
        ColumnTable.profiles,
        const ColumnLayout(widths: <String, double>{'Remarks': 200}),
      )
      ..saveColumnLayout(
        ColumnTable.clashConnections,
        const ColumnLayout(widths: <String, double>{'Host': 180}),
      );

    final backup = source.loadDocument();
    final restored = MemoryUiStateStore()..saveDocument(backup);

    expect(restored.loadWindowGeometry(WindowTypeNames.main)!.width, 1366);
    expect(
      restored.loadColumnLayout(ColumnTable.profiles).widths['Remarks'],
      200,
    );
    expect(
      restored.loadColumnLayout(ColumnTable.clashConnections).widths['Host'],
      180,
    );
  });
}
