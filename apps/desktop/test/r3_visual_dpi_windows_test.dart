// R3-VISUAL-DPI-TRAY: the settings window (all five pages) and the routing
// window must not overflow/truncate across the 100/125/150/200% DPI matrix.
// One page build drives both windows through a ValueListenableBuilder, so the
// locked flutter_tester does not rebuild the whole app per window.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/routing/routing_windows.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'support/dpi_assertions.dart';
import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';

void main() {
  final windows = <(String, Widget Function())>[
    ('设置', OptionSettingWindow.new),
    ('路由', RoutingSettingWindow.new),
  ];

  testWidgets('settings/routing windows survive the 100-200% DPI matrix', (
    tester,
  ) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(SyntheticBridgePort(count: 20)),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(20),
        platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
        monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
      ],
    );
    addTearDown(container.dispose);

    final index = ValueNotifier<int>(0);
    addTearDown(index.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: ValueListenableBuilder<int>(
          valueListenable: index,
          builder: (context, i, _) =>
              MaterialApp(home: Scaffold(body: windows[i].$2())),
        ),
      ),
    );
    await tester.pump();

    for (final scale in kDpiScales) {
      for (final logical in kLogicalSizes) {
        applyDpi(tester, scale, logical);

        // Settings: each of the five upstream pages.
        index.value = 0;
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 40));
        final tabs = find.byType(Tab);
        final tabCount = tabs.evaluate().length;
        expect(tabCount, 5, reason: 'settings must expose five pages');
        for (var t = 0; t < tabCount; t++) {
          await tester.tap(tabs.at(t));
          await tester.pump();
          await tester.pump(const Duration(milliseconds: 40));
          expectNoLayoutProblems(
            tester,
            'settings tab $t ${dpiLabel(scale, logical)}',
          );
        }

        // Routing window.
        index.value = 1;
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 40));
        expectNoLayoutProblems(tester, 'routing ${dpiLabel(scale, logical)}');
      }
    }
  });
}
