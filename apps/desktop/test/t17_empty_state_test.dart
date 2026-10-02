import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/profiles_harness.dart';

/// T17 empty state: an empty node table shows the shared EmptyState (icon +
/// message) instead of a blank grid or a fabricated row.
void main() {
  testWidgets('empty profile table shows the shared empty state', (
    tester,
  ) async {
    await pumpApp(tester, rows: 0);
    expect(find.byKey(const ValueKey('profiles-empty')), findsOneWidget);
    expect(find.text('暂无节点'), findsOneWidget);
  });
}
