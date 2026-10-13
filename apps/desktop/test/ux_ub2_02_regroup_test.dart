import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

ProviderContainer _container(BridgePort bridge) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
    profileRowCountProvider.overrideWithValue(4),
  ],
);

Future<void> _pumpDisplay(
  WidgetTester tester,
  ProviderContainer container,
) async {
  tester.view.physicalSize = const Size(1200, 900);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: OptionSettingWindow())),
    ),
  );
  await tester.pump();
  await tester.pump();
  await tester.tap(find.text('v2rayN 设置'));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('UB2-02: display tab regroups startup and tray', (tester) async {
    final container = _container(seededBridge());
    addTearDown(container.dispose);
    await _pumpDisplay(tester, container);

    expect(find.text('启动与托盘'), findsOneWidget);
    expect(find.text('窗口与托盘'), findsNothing);
    // AutoRun moved under the new section (same tab, key preserved).
    final titleTop = tester.getTopLeft(find.text('启动与托盘'));
    final autoRunTop = tester.getTopLeft(
      find.byKey(const ValueKey('autorun-toggle')),
    );
    expect(autoRunTop.dy, greaterThan(titleTop.dy));
  });

  testWidgets('UB2-02: display rows carry novice help text', (tester) async {
    final container = _container(seededBridge());
    addTearDown(container.dispose);
    await _pumpDisplay(tester, container);

    expect(find.textContaining('点 X 后程序继续在后台运行'), findsOneWidget);
    expect(find.textContaining('小于 10 会被自动改回 10'), findsWidgets);
  });

  testWidgets('UB2-02: font size 0 reads as default', (tester) async {
    final seed = defaultSettingsJson();
    (seed['UiItem'] as Map<String, dynamic>)['CurrentFontSize'] = 0;
    final container = _container(seededBridge(seed));
    addTearDown(container.dispose);
    await _pumpDisplay(tester, container);

    expect(find.textContaining('当前：默认'), findsOneWidget);
  });

  testWidgets('UB2-02: nonzero font size shows its value', (tester) async {
    final seed = defaultSettingsJson();
    (seed['UiItem'] as Map<String, dynamic>)['CurrentFontSize'] = 14;
    final container = _container(seededBridge(seed));
    addTearDown(container.dispose);
    await _pumpDisplay(tester, container);

    expect(find.textContaining('当前：14'), findsOneWidget);
  });

  testWidgets('UB2-02: clash retained section points at panels', (
    tester,
  ) async {
    final container = _container(seededBridge());
    addTearDown(container.dispose);
    await _pumpDisplay(tester, container);

    expect(find.textContaining('代理/连接面板'), findsOneWidget);
  });
}
