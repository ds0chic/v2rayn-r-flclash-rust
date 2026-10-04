// RE-PROF-14a / P2: `自动列宽` really measures content/header and resizes the
// columns (upstream `AutofitColumnWidth` sets every column to Auto), instead of
// falling through to the default log/echo.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('autofit measures content and persists new column widths', (
    tester,
  ) async {
    final container = await pumpApp(tester, rows: 6, width: 1400, height: 700);
    final before = container.read(profilesControllerProvider).columns;

    await tester.tap(find.byKey(const ValueKey('toolbar-自动列宽')));
    await tester.pump();

    final state = container.read(profilesControllerProvider);
    final beforeWidths = <String, double>{
      for (final c in before) c.key: c.width,
    };
    final afterWidths = <String, double>{
      for (final c in state.columns) c.key: c.width,
    };
    final changed = afterWidths.keys
        .where((k) => afterWidths[k] != beforeWidths[k])
        .toList();

    expect(
      changed,
      isNotEmpty,
      reason: 'autofit must resize at least one column',
    );
    expect(afterWidths.values.every((w) => w > 0), isTrue);
    expect(state.lastEvent?.action, 'autofit-columns');

    final store = container.read(uiStateStoreProvider) as MemoryUiStateStore;
    final persisted =
        store.loadSection('column_layout')!['widths'] as Map<String, double>;
    expect(persisted['ConfigType'], afterWidths['ConfigType']);
  });
}
