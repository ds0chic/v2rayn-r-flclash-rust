import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/status_bar_view.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_controller.dart';

import 'support/counting_runtime_bridge.dart';
import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';

void main() {
  testWidgets('status bar shows the four system-proxy modes and applies one', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1600, 200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final bridge = FakePlatformBridge(initialMode: SysProxyMode.unchanged);
    final runtime = CountingRuntimeBridge(
      initial: const RuntimeView(
        state: 'Running',
        hostAlive: true,
        ports: <int>[11808],
      ),
    );
    final monitor = FakeMonitorBridge();
    addTearDown(monitor.disposeStreams);
    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(SyntheticBridgePort(count: 3)),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(3),
        platformBridgeProvider.overrideWithValue(bridge),
        runtimeBridgeProvider.overrideWithValue(runtime),
        monitorBridgeProvider.overrideWithValue(monitor),
      ],
    );
    addTearDown(container.dispose);
    // Seed the desired mode + refresh the live state. RR-03: a proxy mode that
    // needs a live endpoint resolves it from the running session, so the test
    // starts with an applied runtime snapshot.
    container
        .read(platformControllerProvider.notifier)
        .refresh(SysProxyMode.unchanged);
    await container.read(runtimeControllerProvider.notifier).refresh();

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: StatusBarView())),
      ),
    );
    await tester.pump();

    final selector = find.byKey(const ValueKey('system-proxy-selector'));
    expect(selector, findsOneWidget);

    // Open the menu: all four labels are present.
    await tester.tap(selector);
    await tester.pumpAndSettle();
    for (final mode in SysProxyMode.values) {
      expect(find.textContaining(mode.label), findsWidgets);
    }

    // Select 自动配置系统代理 (ForcedChange) -> bridge apply is recorded.
    await tester.tap(find.text('自动配置系统代理').last);
    await tester.pumpAndSettle();
    expect(bridge.appliedModes, contains(SysProxyMode.forcedChange));
    final view = container.read(platformControllerProvider);
    expect(view.desiredMode, SysProxyMode.forcedChange);
  });

  testWidgets('status bar renders the current proxy state label', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1600, 200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final bridge = FakePlatformBridge(initialMode: SysProxyMode.forcedChange);
    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(SyntheticBridgePort(count: 3)),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(3),
        platformBridgeProvider.overrideWithValue(bridge),
      ],
    );
    addTearDown(container.dispose);
    container
        .read(platformControllerProvider.notifier)
        .refresh(SysProxyMode.forcedChange);

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: StatusBarView())),
      ),
    );
    await tester.pump();

    expect(find.byKey(const ValueKey('status-sysproxy')), findsOneWidget);
    expect(find.textContaining('自动配置系统代理'), findsWidgets);
    expect(find.textContaining('已启用 127.0.0.1:10809'), findsOneWidget);
  });
}
