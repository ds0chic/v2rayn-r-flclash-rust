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

    // Esc only stops a running test; it must keep the selection (R3-PROF-10).
    await pressPlain(tester, LogicalKeyboardKey.escape);
    expect(readState(container).selected.length, 4);
    expect(readState(container).lastEvent?.action, 'escape');

    await pressWithCtrl(tester, LogicalKeyboardKey.keyA);
    expect(readState(container).selected.length, 100);
    expect(readState(container).lastEvent?.action, 'select-all');
  });
}
