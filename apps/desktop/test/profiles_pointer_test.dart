import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('double click, right click, drag/drop and header sort', (
    tester,
  ) async {
    final container = await pumpApp(tester, rows: 20);
    final controller = container.read(profilesControllerProvider.notifier);
    final cell = find.byKey(const ValueKey('cell-syn-000000-Remarks'));

    await doubleTap(tester, cell);
    expect(readState(container).lastEvent?.action, 'edit');

    controller.toggleDoubleClick2Activate();
    await tester.pump();
    await doubleTap(tester, cell);
    expect(readState(container).lastEvent?.action, 'activate');

    // Right click keeps the existing multi-selection.
    await tapRow(tester, const ValueKey('cell-syn-000000-Remarks'));
    await pressWithCtrlTap(tester, const ValueKey('cell-syn-000001-Remarks'));
    final before = readState(container).selected;
    await tester.tapAt(
      tester.getCenter(find.byKey(const ValueKey('cell-syn-000001-Remarks'))),
      buttons: kSecondaryButton,
    );
    await tester.pump();
    expect(readState(container).selected, before);
    expect(readState(container).lastEvent?.action, 'context-menu');

    // Drag start/end are captured.
    final source = find.byKey(const ValueKey('handle-syn-000000'));
    final target = find.byKey(const ValueKey('drop-syn-000005'));
    final gesture = await tester.startGesture(tester.getCenter(source));
    await tester.pump(const Duration(milliseconds: 120));
    await gesture.moveTo(tester.getCenter(target));
    await tester.pump(const Duration(milliseconds: 120));
    await gesture.up();
    await tester.pump(const Duration(milliseconds: 200));
    final actions = readState(container).events.map((e) => e.action).toList();
    expect(actions.contains('drag-start'), isTrue);
    expect(actions.contains('drop'), isTrue);

    // Header click sorts locally.
    await tester.tap(find.byKey(const ValueKey('header-Port')));
    await tester.pump();
    expect(readState(container).sort.columnKey, 'Port');
    expect(readState(container).sort.direction.name, 'ascending');
  });
}
