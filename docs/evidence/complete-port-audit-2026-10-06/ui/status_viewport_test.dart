import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/status_bar_view.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import '../../../../apps/desktop/test/support/counting_runtime_bridge.dart';
import '../../../../apps/desktop/test/support/fake_monitor_bridge.dart';
import '../../../../apps/desktop/test/support/fake_platform_bridge.dart';

void main() {
  testWidgets('original 800px minimum width shows controls and rates together', (tester) async {
    tester.view.physicalSize = const Size(800, 180);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final monitor = FakeMonitorBridge();
    addTearDown(monitor.disposeStreams);
    final container = ProviderContainer(overrides: [
      bridgePortProvider.overrideWithValue(SyntheticBridgePort(count: 3)),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(3),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge(initialMode: SysProxyMode.unchanged)),
      runtimeBridgeProvider.overrideWithValue(CountingRuntimeBridge()),
      monitorBridgeProvider.overrideWithValue(monitor),
    ]);
    addTearDown(container.dispose);
    await tester.pumpWidget(UncontrolledProviderScope(container: container, child: const MaterialApp(home: Scaffold(body: StatusBarView()))));
    await tester.pump(const Duration(milliseconds: 30));
    final keys = <String>['system-proxy-selector', 'routing-selector', 'status-proxy-speed', 'status-direct-speed'];
    final clipped = <String>[];
    for (final key in keys) {
      final rect = tester.getRect(find.byKey(ValueKey(key)));
      debugPrint('AUDIT viewport $key: $rect');
      if (rect.left < 0 || rect.right > 800 || rect.top < 0 || rect.bottom > 180) clipped.add(key);
    }
    for (final key in keys) {
      await tester.ensureVisible(find.byKey(ValueKey(key)));
      await tester.pump();
      final rect = tester.getRect(find.byKey(ValueKey(key)));
      debugPrint('AUDIT after horizontal scroll $key: $rect');
      expect(rect.left, greaterThanOrEqualTo(-0.5));
      expect(rect.right, lessThanOrEqualTo(800.5));
    }
    expect(clipped, isEmpty, reason: 'core controls and both rate lines must be visible without horizontally scrolling the bottom bar at the upstream minimum width');
  });
}
