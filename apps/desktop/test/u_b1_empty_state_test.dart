import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/shared/widgets/empty_state.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('profiles empty state exposes import + subscription actions', (
    tester,
  ) async {
    await pumpApp(tester, rows: 0);
    expect(find.byKey(const ValueKey('profiles-empty')), findsOneWidget);
    expect(find.byKey(const ValueKey('empty-action-import')), findsOneWidget);
    expect(find.byKey(const ValueKey('empty-action-sub')), findsOneWidget);
  });

  testWidgets('EmptyState without actions renders no buttons', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(body: EmptyState(message: 'nothing here')),
      ),
    );
    await tester.pump();
    expect(find.byType(EmptyState), findsOneWidget);
    expect(find.byType(FilledButton), findsNothing);
    expect(find.byType(OutlinedButton), findsNothing);
    expect(find.byType(TextButton), findsNothing);
  });

  testWidgets('proxies unsupported state exposes kernel update action', (
    tester,
  ) async {
    await pumpApp(tester, rows: 0);
    await tester.tap(find.byKey(const ValueKey('main-tab-proxies')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));
    expect(find.byKey(const ValueKey('proxies-unsupported')), findsOneWidget);
    expect(find.byKey(const ValueKey('empty-action-update')), findsOneWidget);
  });
}
