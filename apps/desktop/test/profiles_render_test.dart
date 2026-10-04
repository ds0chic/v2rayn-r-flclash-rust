import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:two_dimensional_scrollables/two_dimensional_scrollables.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('renders 10k rows in a real virtualized table', (tester) async {
    final container = await pumpApp(tester, rows: 10000);
    final state = readState(container);
    expect(state.totalCount, 10000);
    expect(state.visible.length, 10000);
    expect(find.byType(TableView), findsOneWidget);
    // R4-06: technical counters live in the "详情" popup, not the action row.
    // The narrow test surface scrolls the status bar horizontally; bring the
    // details control into view before tapping it.
    final details = find.byKey(const ValueKey('status-details'));
    await tester.ensureVisible(details);
    await tester.pumpAndSettle();
    await tester.tap(details);
    await tester.pumpAndSettle();
    expect(
      tester.widget<Text>(find.byKey(const ValueKey('status-counts'))).data,
      contains('total=10000'),
    );
    // Virtualization: far fewer cells materialized than rows * columns.
    expect(find.byType(TableViewCell).evaluate().length, lessThan(1000));
  });
}
