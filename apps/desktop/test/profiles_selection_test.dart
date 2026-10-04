import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('single, ctrl, shift and select-all selection', (tester) async {
    final container = await pumpApp(tester, rows: 100);

    await tapRow(tester, const ValueKey('cell-syn-000000-Remarks'));
    expect(readState(container).selected, <String>{'syn-000000'});

    await pressWithCtrlTap(tester, const ValueKey('cell-syn-000002-Remarks'));
    expect(
      readState(container).selected
          .intersection(<String>{'syn-000000', 'syn-000002'})
          .length,
      2,
    );

    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tapRow(tester, const ValueKey('cell-syn-000005-Remarks'));
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.pump();
    expect(readState(container).selected.length, 4);

    await pressPlain(tester, LogicalKeyboardKey.escape);
    expect(readState(container).selected, isEmpty);

    await pressWithCtrl(tester, LogicalKeyboardKey.keyA);
    expect(readState(container).selected.length, 100);
    expect(readState(container).lastEvent?.action, 'select-all');
  });

  testWidgets('press-and-drag selects an inclusive row range', (tester) async {
    final container = await pumpApp(tester, rows: 100);

    final from = tester.getCenter(
      find.byKey(const ValueKey('cell-syn-000001-Remarks')),
    );
    final to = tester.getCenter(
      find.byKey(const ValueKey('cell-syn-000004-Remarks')),
    );
    final gesture = await tester.startGesture(from);
    await tester.pump();
    await gesture.moveTo(to);
    await tester.pump();
    await gesture.up();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 700));

    expect(readState(container).selected, <String>{
      'syn-000001',
      'syn-000002',
      'syn-000003',
      'syn-000004',
    });
  });

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
