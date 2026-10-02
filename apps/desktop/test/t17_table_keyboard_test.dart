import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/profiles_harness.dart';

/// T17 table navigation: the arrow keys move the single-row selection without
/// touching the reorder (T/U/D/B) actions.
void main() {
  testWidgets('arrow keys navigate the profile table selection', (
    tester,
  ) async {
    final container = await pumpApp(tester, rows: 5);
    await tapRow(tester, const ValueKey('cell-syn-000000-Remarks'));

    final first = readState(container).selected.first;
    await pressPlain(tester, LogicalKeyboardKey.arrowDown);
    final down = readState(container);
    expect(down.selected.length, 1);
    expect(down.selected.first, isNot(first));
    expect(down.lastEvent?.action, 'navigate-down');

    final second = down.selected.first;
    await pressPlain(tester, LogicalKeyboardKey.arrowUp);
    final up = readState(container);
    expect(up.selected.single, first);
    expect(up.selected.first, isNot(second));
    expect(up.lastEvent?.action, 'navigate-up');
  });
}
