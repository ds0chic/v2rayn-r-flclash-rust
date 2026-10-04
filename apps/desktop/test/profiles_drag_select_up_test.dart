import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('drag upward selects the reversed range', (tester) async {
    final container = await pumpApp(tester, rows: 100);

    final from = tester.getCenter(
      find.byKey(const ValueKey('cell-syn-000004-Remarks')),
    );
    final to = tester.getCenter(
      find.byKey(const ValueKey('cell-syn-000002-Remarks')),
    );
    final gesture = await tester.startGesture(from);
    await tester.pump();
    await gesture.moveTo(to);
    await tester.pump();
    await gesture.up();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 700));

    expect(readState(container).selected, <String>{
      'syn-000002',
      'syn-000003',
      'syn-000004',
    });
  });
}
