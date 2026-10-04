import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('drag beyond the bottom edge auto-scrolls the range', (
    tester,
  ) async {
    final container = await pumpApp(tester, rows: 100);

    final from = tester.getCenter(
      find.byKey(const ValueKey('cell-syn-000001-Remarks')),
    );
    final gesture = await tester.startGesture(from);
    await tester.pump();
    await gesture.moveTo(Offset(from.dx, 700));
    await tester.pump();
    final before = readState(container).selected.length;
    await tester.pump(const Duration(milliseconds: 240));
    final after = readState(container).selected.length;
    expect(after, greaterThan(before));
    expect(readState(container).selected.contains('syn-000001'), isTrue);

    await gesture.up();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 700));
  });
}
