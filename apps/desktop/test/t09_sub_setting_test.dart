import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';
import 'package:v2rayn_desktop/features/subs/sub_setting_window.dart';

import 'support/subs_harness.dart';

void main() {
  testWidgets('sub setting window renders the seeded list', (tester) async {
    final container = makeSubsContainer();
    addTearDown(container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: SubSettingWindow())),
      ),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 20));

    expect(find.byKey(const ValueKey('sub-setting-window')), findsOneWidget);
    expect(find.byKey(const ValueKey('sub-list')), findsOneWidget);
    expect(find.text('测试订阅'), findsOneWidget);
    expect(find.byKey(const ValueKey('sub-add')), findsOneWidget);
    expect(find.byKey(const ValueKey('sub-close')), findsOneWidget);
  });

  testWidgets('delete removes the selected subscription', (tester) async {
    final container = makeSubsContainer();
    addTearDown(container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: SubSettingWindow())),
      ),
    );
    await tester.pump();

    final controller = container.read(subsControllerProvider.notifier);
    controller.reload();
    final id = container.read(subsControllerProvider).items.first.id;
    controller.select(id);
    await tester.pump();
    controller.delete(<String>[id]);
    await tester.pump();

    expect(container.read(subsControllerProvider).items, isEmpty);
    expect(find.byKey(const ValueKey('sub-empty')), findsOneWidget);
  });

  testWidgets('toggling enabled persists through the bridge', (tester) async {
    final bridge = SeededSubsBridge();
    final container = makeSubsContainer(bridge: bridge);
    addTearDown(container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: SubSettingWindow())),
      ),
    );
    await tester.pump();

    final id = container.read(subsControllerProvider).items.first.id;
    container.read(subsControllerProvider.notifier).setEnabled(id, false);
    await tester.pump();

    final item = container.read(subsControllerProvider).items.first;
    expect(item.id, id);
    expect(item.enabled, isFalse);
    expect(bridge.getSubItem(id)!.enabled, isFalse);
  });

  testWidgets('update reports a structured success status', (tester) async {
    final container = makeSubsContainer();
    addTearDown(container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: SubSettingWindow())),
      ),
    );
    await tester.pump();

    final controller = container.read(subsControllerProvider.notifier);
    final result = await controller.update(viaProxy: false);
    await tester.pump();

    expect(result.ok, isTrue);
    expect(container.read(subsControllerProvider).status!.isSuccess, isTrue);
  });
}
