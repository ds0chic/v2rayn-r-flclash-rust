import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/features/subs/sub_edit_window.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

import 'support/subs_harness.dart';

Future<void> _pump(
  WidgetTester tester,
  ProviderContainer container,
  c.SubItemDto draft,
) async {
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(
        home: Scaffold(body: SubEditWindow(initial: draft)),
      ),
    ),
  );
  await tester.pump();
}

void main() {
  testWidgets('UB2-03: editor groups rows into basic/advanced/expert', (
    tester,
  ) async {
    final container = makeSubsContainer();
    addTearDown(container.dispose);
    final draft = container.read(subsControllerProvider.notifier).newDraft();
    await _pump(tester, container, draft);

    expect(find.text('基础'), findsOneWidget);
    expect(find.text('进阶'), findsOneWidget);
    expect(find.text('专家'), findsOneWidget);
  });

  testWidgets('UB2-03: expert rows hide until the section opens', (
    tester,
  ) async {
    final container = makeSubsContainer();
    addTearDown(container.dispose);
    final draft = container.read(subsControllerProvider.notifier).newDraft();
    await _pump(tester, container, draft);

    expect(find.byKey(const ValueKey('sub-field-presocks')), findsNothing);
    await tester.ensureVisible(find.text('专家'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('专家'));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('sub-field-presocks')), findsOneWidget);
  });

  testWidgets('UB2-03: rows carry helper captions and gap notes', (
    tester,
  ) async {
    final container = makeSubsContainer();
    addTearDown(container.dispose);
    final draft = container.read(subsControllerProvider.notifier).newDraft();
    await _pump(tester, container, draft);

    expect(find.textContaining('只分类不下载'), findsOneWidget);
    await tester.ensureVisible(find.text('专家'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('专家'));
    await tester.pumpAndSettle();
    expect(find.textContaining('原版有节点选择按钮'), findsWidgets);
  });
}
