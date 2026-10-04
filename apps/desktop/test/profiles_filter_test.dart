import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets(
    'filter applies on Enter/clear (not per keystroke), keeps focus scope',
    (tester) async {
      final container = await pumpApp(tester, rows: 500);
      final filter = find.byKey(const ValueKey('filter-field'));

      await tester.tap(filter);
      await tester.enterText(filter, 'Synthetic-00001');
      await tester.pump();
      // Upstream `ServerFilterChanged` only refreshes when the box is cleared;
      // a non-empty query is committed on Enter, not on every keystroke.
      expect(readState(container).visible.length, 500);

      await tester.testTextInput.receiveAction(TextInputAction.done);
      await tester.pump();
      final filtered = readState(container);
      expect(filtered.visible.length, lessThan(500));
      expect(
        filtered.visible.every((r) => r.remarks.contains('Synthetic-00001')),
        isTrue,
      );
      expect(readState(container).lastEvent?.action, 'refresh');

      // Clearing the box refreshes immediately (upstream empty-query case).
      await tester.enterText(filter, '');
      await tester.pump();
      expect(readState(container).visible.length, 500);

      final beforeSelection = readState(container).selectedCount;
      final beforeEvents = readState(container).events.length;
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.keyD);
      await tester.pump();

      final after = readState(container);
      expect(
        after.selectedCount,
        beforeSelection,
        reason: 'Ctrl+A must stay in the filter',
      );
      expect(
        after.events.length,
        beforeEvents,
        reason: 'no table action should fire',
      );
    },
  );
}
