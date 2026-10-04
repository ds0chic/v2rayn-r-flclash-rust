// RE-PROF-13 / P2: the active node must carry its own visual marker,
// independent from the multi-select highlight (upstream `ProfilesView.xaml`
// `IsActive` DataTrigger on every cell). One page build per process because the
// locked flutter_tester leaks native resources.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_table.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('active row keeps its own fill while another row is selected', (
    tester,
  ) async {
    final container = await pumpApp(tester, rows: 40, width: 1200, height: 700);
    final controller = container.read(profilesControllerProvider.notifier);
    final rows = container.read(profilesControllerProvider).all;
    final activeId = rows[0].id;
    final selectedId = rows[1].id;

    controller.setActive(activeId);
    await tester.pump();
    controller.selectRow(selectedId);
    await tester.pump();

    final context = tester.element(find.byType(ProfilesTable));
    final selectedFill = Theme.of(context).colorScheme.primaryContainer;

    final activeCell = tester.widget<Container>(
      find
          .descendant(
            of: find.byKey(ValueKey('cell-$activeId-Remarks')),
            matching: find.byType(Container),
          )
          .first,
    );
    final selectedCell = tester.widget<Container>(
      find
          .descendant(
            of: find.byKey(ValueKey('cell-$selectedId-Remarks')),
            matching: find.byType(Container),
          )
          .first,
    );

    expect(
      activeCell.color,
      activeRowFill(context),
      reason: 'active row keeps its own fill',
    );
    expect(
      selectedCell.color,
      selectedFill,
      reason: 'selected row keeps its fill',
    );
    expect(activeCell.color, isNot(selectedCell.color));

    // The leading marker stays on the active row even when it is also selected.
    controller.selectRow(activeId);
    await tester.pump();
    final marker = find.byKey(ValueKey('active-marker-$activeId'));
    expect(marker, findsOneWidget);
    expect(
      tester.widget<ColoredBox>(marker).color,
      activeRowMarkerColor(context),
    );
  });
}
