// R4-13.S23 settings-save-to-effect contract (UiItem instance).
//
// Storage chain: the three-layout / window-size / column-width state source
// persists and is visible after reopen (R4-12 hand-off), plus the shell DPI
// matrix. No native library, real window, proxy/route/TUN or user data.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

import 'support/dpi_assertions.dart';
import 'support/r4_13_settings_support.dart';

void main() {
  test('UiItem layout/window/column fields save -> reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      final g = draft['UiItem'] as Map<String, dynamic>;
      g['MainGirdOrientation'] = 2;
      g['MainGirdHeight1'] = 320;
      g['MainGirdHeight2'] = 210;
      g['EnableAutoAdjustMainLvColWidth'] = true;
      g['HideColumnIpInfo'] = true;
      g['CurrentFontSize'] = 15;
      g['CurrentFontFamily'] = 'Segoe UI';
      g['CurrentTheme'] = 'Dark';
      g['ColorPrimaryName'] = 'Blue';
      g['CurrentLanguage'] = 'en';
      g['EnableDragDropSort'] = true;
      g['MainColumnItem'] = <Map<String, dynamic>>[
        <String, dynamic>{'Name': 'Remarks', 'Width': 220, 'Index': 0},
      ];
      g['WindowSizeItem'] = <Map<String, dynamic>>[
        <String, dynamic>{
          'TypeName': 'MainWindow',
          'Width': 1280,
          'Height': 800,
        },
      ];
    });
    final g = doc['UiItem'] as Map<String, dynamic>;
    expect(g['MainGirdOrientation'], 2);
    expect(g['MainGirdHeight1'], 320);
    expect(g['MainGirdHeight2'], 210);
    expect(g['EnableAutoAdjustMainLvColWidth'], isTrue);
    expect(g['HideColumnIpInfo'], isTrue);
    expect(g['CurrentFontSize'], 15);
    expect(g['CurrentFontFamily'], 'Segoe UI');
    expect(g['CurrentTheme'], 'Dark');
    expect(g['ColorPrimaryName'], 'Blue');
    expect(g['CurrentLanguage'], 'en');
    expect(g['EnableDragDropSort'], isTrue);
    expect((g['MainColumnItem'] as List).length, 1);
    expect((g['WindowSizeItem'] as List).length, 1);
  });

  test('window geometry and column layout round-trip through one store', () {
    final store = MemoryUiStateStore();
    store.saveWindowGeometry(
      const WindowGeometry(
        typeName: WindowTypeNames.main,
        width: 1280,
        height: 800,
        mainGridHeight1: 320,
        mainGridHeight2: 210,
        orientation: 2,
      ),
    );
    const layout = ColumnLayout(
      order: <String>['Remarks', 'Address', 'Delay'],
      visible: <String, bool>{'Remarks': true, 'Delay': false},
      widths: <String, double>{'Remarks': 220, 'Address': 260},
    );
    store.saveColumnLayout(ColumnTable.profiles, layout);

    final reopened = MemoryUiStateStore();
    reopened.saveDocument(store.loadDocument());
    final geometry = reopened.loadWindowGeometry(WindowTypeNames.main);
    expect(geometry, isNotNull);
    expect(geometry!.width, 1280);
    expect(geometry.height, 800);
    expect(geometry.orientation, 2);
    expect(geometry.mainGridHeight1, 320);
    expect(geometry.mainGridHeight2, 210);
    final reloaded = reopened.loadColumnLayout(ColumnTable.profiles);
    expect(reloaded.order, layout.order);
    expect(reloaded.visible, layout.visible);
    expect(reloaded.widths, layout.widths);
  });

  testWidgets('main shell survives the DPI matrix in all three layouts', (
    tester,
  ) async {
    await runShellDpiCase(tester, 1.0);
  });
}
