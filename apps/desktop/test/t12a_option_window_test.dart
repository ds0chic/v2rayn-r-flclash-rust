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

      // FIX-16 / R3-WPF-Option-Labels: exactly the frozen upstream TabItem set
      // (Core / N / SystemProxy / TunMode / CoreType). KCP and HappyEyeballs are
      // retained sections, not top-level pages.
      for (final tab in <String>[
        'Core: 基础设置',
        'v2rayN 设置',
        '系统代理设置',
        'Tun 模式设置',
        'Core 类型设置',
      ]) {
        expect(find.text(tab), findsOneWidget, reason: 'missing tab $tab');
      }
      for (final gone in <String>['KCP', 'HappyEyeballs', '测速']) {
        expect(
          find.widgetWithText(Tab, gone),
          findsNothing,
          reason: '$gone must not be a top-level tab',
        );
      }

      // The v2rayN settings tab renders (guards against a cast regression).
      await tester.tap(find.text('v2rayN 设置'));
      await tester.pumpAndSettle();
      expect(find.text('当前字体 (需重启)'), findsOneWidget);

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

      await tester.tap(find.text('Core: 基础设置'));
      await tester.pumpAndSettle();

      // Upstream (frozen XAML rows 8-10) renders the LAN new-port row and the
      // auth user/pass fields unconditionally; RC previously hid them behind
      // AllowLANConn / NewPort4LAN.
      expect(find.text('为局域网开启新的端口'), findsOneWidget);
      expect(find.text('认证用户名'), findsOneWidget);
      expect(find.text('认证密码'), findsOneWidget);
      // No raw storage keys leak into visible field labels.
      expect(find.textContaining('(LocalPort)'), findsNothing);
      expect(find.textContaining('(SniffingEnabled)'), findsNothing);

      // KCP retained section is still editable on the Core page.
      await tester.ensureVisible(find.text('最大发送窗口'));
      await tester.pumpAndSettle();
      expect(find.text('最大发送窗口'), findsOneWidget);

      await tester.tap(find.text('v2rayN 设置'));
      await tester.pumpAndSettle();
      // FakeIP linkage is retained on the v2rayN settings page (upstream DNS
      // window owner) until FIX-16B adds the DNS window.
      await tapVisible('启用 FakeIP');
      expect(find.text('全局 FakeIP'), findsOneWidget);

      // Cancel: the draft never reached the engine.
      await tester.tap(find.text('取消'));
      await tester.pumpAndSettle();
      expect(bridge.settingsRevision(), 0);
      expect(container.read(settingsControllerProvider).revision, 0);
    },
  );
}
