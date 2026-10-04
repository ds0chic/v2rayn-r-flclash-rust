import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

import 'support/profiles_harness.dart';

/// One page build covers all shell-chrome assertions to stay within the
/// flutter_tester build budget documented in docs/evidence/T01.md §6.
void main() {
  testWidgets(
    'shell chrome: status bar, menu, three layouts, split persistence',
    (tester) async {
      final store = MemoryUiStateStore();
      final container = await pumpApp(tester, rows: 100, store: store);

      // LAY-MAIN-002: vertical is the upstream default.
      expect(find.byKey(const ValueKey('split-vertical')), findsOneWidget);
      expect(find.byKey(const ValueKey('split-horizontal')), findsNothing);

      // Menu structure/text (LAY-MAIN-004).
      for (final label in <String>['配置项', '订阅分组', '设置', '帮助', '重启服务']) {
        expect(find.text(label), findsWidgets, reason: 'missing menu $label');
      }

      // Status bar shows no fabricated live state (LAY-STATUSBAR-001).
      final proxySpeed = tester
          .widget<Text>(find.byKey(const ValueKey('status-proxy-speed')))
          .data!;
      final directSpeed = tester
          .widget<Text>(find.byKey(const ValueKey('status-direct-speed')))
          .data!;
      final runningNode = tester
          .widget<Text>(find.byKey(const ValueKey('running-node')))
          .data!;
      expect(proxySpeed.contains('--'), isTrue);
      expect(directSpeed.contains('--'), isTrue);
      expect(runningNode.contains('未运行'), isTrue);
      for (final text in <String>[proxySpeed, directSpeed, runningNode]) {
        expect(text.contains('已连接'), isFalse);
      }

      // R3-WPF-Main-Chrome M5: upstream status-bar wording (ResUI 本地/局域网/
      // 启用 Tun) rather than the RC 入站/LAN/TUN labels.
      final inbound = tester
          .widget<Text>(find.byKey(const ValueKey('status-inbound')))
          .data!;
      final inboundLan = tester
          .widget<Text>(find.byKey(const ValueKey('status-inbound-lan')))
          .data!;
      expect(inbound.startsWith('本地:'), isTrue, reason: inbound);
      expect(inboundLan.startsWith('局域网:'), isTrue, reason: inboundLan);
      expect(find.text('启用 Tun'), findsOneWidget);

      // R3-WPF-Main-Chrome M3: the row header is untitled; no `#` data column.
      expect(find.text('#'), findsNothing);

      // T16: 检查更新 now opens the real update window instead of reporting
      // "尚未实现". Close it so the layout assertions below still run.
      await tester.tap(find.text('帮助'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('检查更新'));
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('update-window')), findsOneWidget);
      await tester.tap(find.byKey(const ValueKey('update-close')));
      await tester.pumpAndSettle();

      // Switch layout through the toolbar (UIItem.MainGirdOrientation).
      await tester.tap(find.byKey(const ValueKey('layout-selector')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('水平布局').last);
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('split-horizontal')), findsOneWidget);
      expect(store.loadSection('layout')!['mode'], 'horizontal');

      // Splitter fraction is persisted.
      final before = (store.loadSection('layout')!['horizontal_split'] as num)
          .toDouble();
      await tester.drag(
        find.byKey(const ValueKey('split-horizontal')),
        const Offset(160, 0),
      );
      await tester.pumpAndSettle();
      final after = (store.loadSection('layout')!['horizontal_split'] as num)
          .toDouble();
      expect(after, greaterThan(before));

      // Tab layout keeps profiles first (LAY-MAIN-003).
      container
          .read(uiShellControllerProvider.notifier)
          .setLayout(AppLayoutMode.tab);
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('main-tab-profiles')), findsOneWidget);
      expect(find.byKey(const ValueKey('main-tab-info')), findsOneWidget);

      // Reopen with the same store: layout + split restored.
      final restored = ProviderContainer(
        overrides: [
          bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
          uiStateStoreProvider.overrideWithValue(store),
          profileRowCountProvider.overrideWithValue(100),
        ],
      );
      addTearDown(restored.dispose);
      final reloaded = restored.read(uiShellControllerProvider);
      expect(reloaded.layout, AppLayoutMode.tab);
      expect(reloaded.horizontalSplit, after);
    },
  );
}
