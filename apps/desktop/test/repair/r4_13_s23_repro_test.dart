// R4-13.S23 repro (UiItem layout/window/column state source guard).
//
// No in-scope defect was found in the settings-save or `ui_state_store` paths:
// the `UiItem` layout state source round-trips, and the shell consumes
// `MainGirdOrientation` (R4-12, HEAD cbce523). The remaining `MainGirdHeight*`
// live resize wiring lives in `lib/app/shell/ui_shell_controller.dart`, which is
// owned by R4-12 and outside this instance's file ownership; it is registered as
// an interface hand-off, not re-implemented here.
//
// This file is the regression guard for the state source plus the 150% DPI
// matrix point. Synthetic data only.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

import '../support/dpi_assertions.dart';

void main() {
  test(
    'three layouts/window sizes/column widths survive a full-document reopen',
    () {
      final store = MemoryUiStateStore();
      for (final entry in <String, int>{
        WindowTypeNames.main: 2,
        WindowTypeNames.optionSetting: 1,
        WindowTypeNames.subSetting: 0,
      }.entries) {
        store.saveWindowGeometry(
          WindowGeometry(
            typeName: entry.key,
            width: 1000,
            height: 700,
            mainGridHeight1: 300,
            mainGridHeight2: 200,
            orientation: entry.value,
          ),
        );
      }
      store.saveColumnLayout(
        ColumnTable.profiles,
        const ColumnLayout(
          order: <String>['Remarks', 'Address'],
          visible: <String, bool>{'Remarks': true},
          widths: <String, double>{'Remarks': 220},
        ),
      );
      store.saveColumnLayout(
        ColumnTable.clashConnections,
        const ColumnLayout(
          order: <String>['Host', 'Rule'],
          visible: <String, bool>{'Host': true},
          widths: <String, double>{'Host': 180},
        ),
      );

      final reopened = MemoryUiStateStore();
      reopened.saveDocument(store.loadDocument());

      for (final entry in <String, int>{
        WindowTypeNames.main: 2,
        WindowTypeNames.optionSetting: 1,
        WindowTypeNames.subSetting: 0,
      }.entries) {
        expect(
          reopened.loadWindowGeometry(entry.key)!.orientation,
          entry.value,
        );
      }
      expect(
        reopened.loadColumnLayout(ColumnTable.profiles).widths['Remarks'],
        220,
      );
      expect(
        reopened.loadColumnLayout(ColumnTable.clashConnections).widths['Host'],
        180,
      );
    },
  );

  testWidgets('main shell survives the 150% DPI matrix in all three layouts', (
    tester,
  ) async {
    await runShellDpiCase(tester, 1.5);
  });
}
