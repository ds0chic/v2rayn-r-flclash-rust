// FIX-16D: at desktop font scaling (1.5x) the settings window must not overlap
// or truncate, and its primary buttons must stay reachable (hittable). This is
// the widget-level guard; the real Windows window run is tracked separately.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';

void main() {
  testWidgets('settings window stays usable at 1.5x desktop text scale', (
    tester,
  ) async {
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.binding.setSurfaceSize(const Size(1000, 700));

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

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: const TextScaler.linear(1.5)),
            child: child!,
          ),
          home: const Scaffold(body: OptionSettingWindow()),
        ),
      ),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 60));

    expect(tester.takeException(), isNull);

    // Primary actions stay reachable (hit-testable) at the larger font.
    // Upstream has a single 确定/取消 pair (R3-WPF-Option-Labels).
    final save = find.byKey(const ValueKey('settings-save'));
    final cancel = find.byKey(const ValueKey('settings-cancel'));
    expect(save, findsOneWidget);
    expect(cancel, findsOneWidget);
    await tester.ensureVisible(save);
    await tester.pump();
    await tester.tap(save, warnIfMissed: false);
    await tester.pump();
    expect(tester.takeException(), isNull);
  });
}
