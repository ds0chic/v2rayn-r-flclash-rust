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
    expect(
      tester.widget<Text>(find.byKey(const ValueKey('status-counts'))).data,
      contains('total=10000'),
    );
    // Virtualization: far fewer cells materialized than rows * columns.
    expect(find.byType(TableViewCell).evaluate().length, lessThan(1000));
  });
}
