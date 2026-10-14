import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/update/check_update_view.dart';
import 'package:v2rayn_desktop/features/update/update_controller.dart';

ProviderContainer _container(SyntheticBridgePort bridge) => ProviderContainer(
  overrides: [bridgePortProvider.overrideWithValue(bridge)],
);

Future<void> _pump(WidgetTester tester, ProviderContainer container) async {
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: CheckUpdateView())),
    ),
  );
  await tester.pump();
  await tester.pump();
}

void main() {
  testWidgets('UB2-05: update toggles carry helper captions', (tester) async {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pump(tester, container);

    expect(find.textContaining('新手保持关闭'), findsOneWidget);
    expect(find.textContaining('先看代理本身通不通'), findsOneWidget);
  });

  testWidgets('UB2-05: empty selection skips the bridge check', (tester) async {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pump(tester, container);

    final notifier = container.read(updateControllerProvider.notifier);
    for (final core in List<String>.of(
      container.read(updateControllerProvider).selected,
    )) {
      notifier.toggleCore(core, false);
    }
    await tester.pump();
    expect(container.read(updateControllerProvider).selected, isEmpty);

    await tester.tap(find.byKey(const ValueKey('update-apply-btn')));
    await tester.pumpAndSettle();

    expect(
      bridge.t16Calls.where((c) => c.startsWith('check_updates')),
      isEmpty,
    );
    expect(find.textContaining('未选择检查目标'), findsOneWidget);
  });
}
