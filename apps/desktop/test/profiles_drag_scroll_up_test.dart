import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('drag back above the top edge scrolls up to the first row', (
    tester,
  ) async {
    final container = await pumpApp(tester, rows: 100);

    final from = tester.getCenter(
      find.byKey(const ValueKey('cell-syn-000001-Remarks')),
    );
    final gesture = await tester.startGesture(from);
    await tester.pump();
    // Scroll down first so there is room to scroll back up.
    await gesture.moveTo(Offset(from.dx, 700));
    await tester.pump(const Duration(milliseconds: 320));
    expect(readState(container).selected.length, greaterThan(2));
    // Hold above the header: the range shrinks back to the anchor and row 0.
    await gesture.moveTo(Offset(from.dx, 0));
    await tester.pump(const Duration(milliseconds: 640));
    expect(readState(container).selected, <String>{'syn-000000', 'syn-000001'});

    await gesture.up();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 700));
  });
}
