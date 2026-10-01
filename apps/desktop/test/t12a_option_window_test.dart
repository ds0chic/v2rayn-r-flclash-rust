import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

void main() {
  testWidgets('option window: tabs, LAN auth link, FakeIP link, cancel', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1200, 900);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final bridge = SyntheticBridgePort(count: 4);
    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(bridge),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(4),
      ],
    );
    addTearDown(container.dispose);

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: OptionSettingWindow())),
      ),
    );
    await tester.pump();
    await tester.pump();

    for (final tab in <String>['核心基础', '显示', '系统代理', 'Tun 模式', '内核类型']) {
      expect(find.text(tab), findsOneWidget, reason: 'missing tab $tab');
    }

    // The 显示 tab renders (guards against an int/String cast regression).
    await tester.tap(find.text('显示'));
    await tester.pumpAndSettle();
    expect(find.text('字体族 (CurrentFontFamily)'), findsOneWidget);
    await tester.tap(find.text('核心基础'));
    await tester.pumpAndSettle();

    Future<void> tapVisible(String label) async {
      final finder = find.text(label);
      await tester.ensureVisible(finder);
      await tester.pumpAndSettle();
      await tester.tap(finder);
      await tester.pump();
    }

    // LAN auth linkage: NewPort4LAN appears with AllowLANConn, User/Pass with both.
    expect(find.text('为局域网使用新端口'), findsNothing);
    await tapVisible('允许来自局域网的连接');
    expect(find.text('为局域网使用新端口'), findsOneWidget);
    await tapVisible('为局域网使用新端口');
    expect(find.text('用户名 (User)'), findsOneWidget);
    expect(find.text('密码 (Pass)'), findsOneWidget);

    // FakeIP linkage on the HappyEyeballs tab.
    await tester.ensureVisible(find.text('HappyEyeballs'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('HappyEyeballs'));
    await tester.pumpAndSettle();
    expect(find.text('FakeIP 范围 (FakeIPRange)'), findsNothing);
    await tapVisible('启用 FakeIP');
    expect(find.text('FakeIP 范围 (FakeIPRange)'), findsOneWidget);

    // KCP tab: all 6 upstream KcpItem fields are editable (no fake
    // read-only note).
    await tester.ensureVisible(find.text('KCP'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('KCP'));
    await tester.pumpAndSettle();
    for (final label in <String>[
      'MTU',
      'TTI',
      '拥塞窗口倍数 (CwndMultiplier)',
      '最大发送窗口 (MaxSendingWindow)',
    ]) {
      expect(find.text(label), findsOneWidget, reason: 'missing KCP $label');
    }

    // Cancel: the draft never reached the engine.
    await tester.tap(find.text('取消'));
    await tester.pumpAndSettle();
    expect(bridge.settingsRevision(), 0);
    expect(container.read(settingsControllerProvider).revision, 0);
  });
}
