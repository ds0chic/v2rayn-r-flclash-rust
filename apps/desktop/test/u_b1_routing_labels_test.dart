import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('status bar selectors carry group captions', (tester) async {
    await pumpApp(tester, rows: 5);
    expect(find.byKey(const ValueKey('label-sysproxy')), findsOneWidget);
    expect(find.byKey(const ValueKey('label-rule-mode')), findsOneWidget);
    expect(find.byKey(const ValueKey('label-routing-scheme')), findsOneWidget);
    expect(find.text('代理'), findsOneWidget);
    expect(find.text('模式'), findsOneWidget);
    expect(find.text('方案'), findsOneWidget);
  });

  testWidgets('builtin rules import folds into advanced by default', (
    tester,
  ) async {
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      const MethodChannel('v2rayn/routing_window'),
      (call) async => call.method == 'open' ? false : null,
    );
    addTearDown(
      () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        const MethodChannel('v2rayn/routing_window'),
        null,
      ),
    );
    await pumpApp(tester, rows: 5);
    await tester.tap(find.byKey(const ValueKey('menu-设置')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('menu-item-路由设置')));
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('routing-setting-window')),
      findsOneWidget,
    );
    expect(find.byKey(const ValueKey('routing-import-builtin')), findsNothing);
    await tester.tap(find.byKey(const ValueKey('routing-advanced-toggle')));
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('routing-import-builtin')),
      findsOneWidget,
    );
    await tester.tap(find.byKey(const ValueKey('routing-advanced-toggle')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('routing-import-builtin')), findsNothing);
    await tester.tap(find.byKey(const ValueKey('routing-close')));
    await tester.pumpAndSettle();
  });
}
