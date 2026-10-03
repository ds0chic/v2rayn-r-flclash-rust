import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

void main() {
  testWidgets(
    'option window: 5 upstream tabs, linkage, retained sections, cancel',
    (tester) async {
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

      // FIX-16: exactly the frozen upstream TabItem set (Core / N /
      // SystemProxy / TunMode / CoreType). KCP and HappyEyeballs are retained
      // sections, not top-level pages.
      for (final tab in <String>['核心基础', '显示', '系统代理', 'Tun 模式', '内核类型']) {
        expect(find.text(tab), findsOneWidget, reason: 'missing tab $tab');
      }
      for (final gone in <String>['KCP', 'HappyEyeballs', '测速']) {
        expect(
          find.widgetWithText(Tab, gone),
          findsNothing,
          reason: '$gone must not be a top-level tab',
        );
      }

      // The 显示 tab renders (guards against an int/String cast regression).
      await tester.tap(find.text('显示'));
      await tester.pumpAndSettle();
      expect(find.text('字体族 (CurrentFontFamily)'), findsOneWidget);

      Future<void> tapVisible(String label) async {
        final finder = find.text(label);
        await tester.ensureVisible(finder);
        await tester.pumpAndSettle();
        await tester.tap(finder);
        await tester.pump();
      }

      // Language consumer surface (FIX-16 first batch).
      await tester.ensureVisible(
        find.byKey(const ValueKey('settings-language')),
      );
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('settings-language')), findsOneWidget);
      expect(find.byKey(const ValueKey('settings-root-cert')), findsOneWidget);

      await tester.tap(find.text('核心基础'));
      await tester.pumpAndSettle();

      // LAN auth linkage: NewPort4LAN appears with AllowLANConn, User/Pass with
      // both. Linkage now lives on the Core page (upstream TabItem Core).
      expect(find.text('为局域网使用新端口'), findsNothing);
      await tapVisible('允许来自局域网的连接');
      expect(find.text('为局域网使用新端口'), findsOneWidget);
      await tapVisible('为局域网使用新端口');
      expect(find.text('用户名 (User)'), findsOneWidget);
      expect(find.text('密码 (Pass)'), findsOneWidget);

      // KCP retained section is still editable on the Core page.
      await tester.ensureVisible(find.text('最大发送窗口 (MaxSendingWindow)'));
      await tester.pumpAndSettle();
      expect(find.text('最大发送窗口 (MaxSendingWindow)'), findsOneWidget);

      await tester.tap(find.text('显示'));
      await tester.pumpAndSettle();
      // FakeIP linkage is retained on the 显示 page (upstream DNS window owner).
      await tapVisible('启用 FakeIP');
      expect(find.text('全局 FakeIP (GlobalFakeIp)'), findsOneWidget);

      // Cancel: the draft never reached the engine.
      await tester.tap(find.text('取消'));
      await tester.pumpAndSettle();
      expect(bridge.settingsRevision(), 0);
      expect(container.read(settingsControllerProvider).revision, 0);
    },
  );
}
