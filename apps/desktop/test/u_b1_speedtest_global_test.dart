import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';
import 'support/profiles_harness.dart';

/// Test notifier that reuses the real controller build, then freezes a
/// speedtest in progress (no backend run needed to cover the projection).
class _SpeedyProfiles extends ProfilesController {
  @override
  ProfilesState build() => super.build().copyWith(
    speedTestRunning: true,
    speedTestStage: 'tcping',
    speedTestMessage: 'tcping 3/10',
  );
}

ProviderContainer _speedContainer() {
  return ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(5),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
      monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
      profilesControllerProvider.overrideWith(_SpeedyProfiles.new),
    ],
  );
}

void main() {
  testWidgets('speedtest progress shows in the status bar on any tab', (
    tester,
  ) async {
    await pumpApp(tester, container: _speedContainer());
    expect(find.byKey(const ValueKey('speedtest-global')), findsOneWidget);
    expect(
      tester.widget<Text>(find.byKey(const ValueKey('speedtest-global'))).data,
      contains('tcping 3/10'),
    );
    // Switching tabs keeps the indicator (and its stop entry) visible.
    await tester.tap(find.byKey(const ValueKey('main-tab-info')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));
    expect(find.byKey(const ValueKey('speedtest-global')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('speedtest-global-stop')));
    await tester.pump();
  });

  testWidgets('no speedtest indicator when idle', (tester) async {
    await pumpApp(tester, rows: 5);
    expect(find.byKey(const ValueKey('speedtest-global')), findsNothing);
    expect(find.byKey(const ValueKey('speedtest-global-stop')), findsNothing);
  });
}
