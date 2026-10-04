import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/status_bar_view.dart';
import 'package:v2rayn_desktop/bridge/api/routing.dart' as r;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/routing/routing_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

import 'support/counting_runtime_bridge.dart';
import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';

const RuntimeView _runningView = RuntimeView(
  state: 'Running',
  hostAlive: true,
  ports: <int>[11808],
  sessionId: 'synthetic',
);

r.RoutingProfileDto _secondRouting(r.RoutingProfileDto first, String id) =>
    r.RoutingProfileDto(
      id: id,
      remarks: '绕过大陆',
      url: first.url,
      ruleSet: first.ruleSet,
      ruleNum: first.ruleNum,
      enabled: true,
      locked: false,
      customIcon: '',
      customRulesetPath4Singbox: '',
      domainStrategy: first.domainStrategy,
      domainStrategy4Singbox: first.domainStrategy4Singbox,
      sort: first.sort + 1,
      isActive: false,
    );

Future<ProviderContainer> _pumpStatusBar(
  WidgetTester tester, {
  required FakePlatformBridge platform,
  required CountingRuntimeBridge runtime,
}) async {
  tester.view.physicalSize = const Size(1600, 200);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);

  final monitor = FakeMonitorBridge();
  addTearDown(monitor.disposeStreams);
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(SyntheticBridgePort(count: 3)),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(3),
      platformBridgeProvider.overrideWithValue(platform),
      runtimeBridgeProvider.overrideWithValue(runtime),
      monitorBridgeProvider.overrideWithValue(monitor),
    ],
  );
  addTearDown(container.dispose);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: StatusBarView())),
    ),
  );
  await tester.pump();
  return container;
}

void main() {
  group('RR-02 default routing command', () {
    test(
      'setDefaultAndReload persists the default and triggers reload/apply',
      () async {
        final bridge = SyntheticBridgePort();
        final runtime = CountingRuntimeBridge(initial: _runningView);
        final container = ProviderContainer(
          overrides: [
            bridgePortProvider.overrideWithValue(bridge),
            runtimeBridgeProvider.overrideWithValue(runtime),
          ],
        );
        addTearDown(container.dispose);

        final controller = container.read(routingControllerProvider.notifier);
        final first = container.read(routingControllerProvider).items.first;
        bridge.saveRouting(_secondRouting(first, 'r-bypass'));
        controller.reload();
        expect(container.read(routingControllerProvider).items.length, 2);

        final applyBefore = runtime.applyCalls;
        await controller.setDefaultAndReload('r-bypass');

        final state = container.read(routingControllerProvider);
        final active = state.items.firstWhere((e) => e.isActive);
        expect(active.id, 'r-bypass');
        expect(runtime.applyCalls, applyBefore + 1);
        // The editor selection is not repurposed as the default.
        expect(state.selectedId, first.id);
      },
    );

    test(
      'select stays edit-selection only and never changes the default',
      () async {
        final bridge = SyntheticBridgePort();
        final runtime = CountingRuntimeBridge(initial: _runningView);
        final container = ProviderContainer(
          overrides: [
            bridgePortProvider.overrideWithValue(bridge),
            runtimeBridgeProvider.overrideWithValue(runtime),
          ],
        );
        addTearDown(container.dispose);

        final controller = container.read(routingControllerProvider.notifier);
        final first = container.read(routingControllerProvider).items.first;
        bridge.saveRouting(_secondRouting(first, 'r-bypass'));
        controller.reload();
        final applyBefore = runtime.applyCalls;

        controller.select('r-bypass');

        final state = container.read(routingControllerProvider);
        expect(state.selectedId, 'r-bypass');
        expect(state.items.firstWhere((e) => e.isActive).id, first.id);
        expect(runtime.applyCalls, applyBefore);
      },
    );
  });

  group('RR-03 system proxy / PAC', () {
    testWidgets('PAC saves SysProxyType and serves the configured pac.txt', (
      tester,
    ) async {
      final platform = FakePlatformBridge(initialMode: SysProxyMode.unchanged);
      final runtime = CountingRuntimeBridge(initial: _runningView);
      final container = await _pumpStatusBar(
        tester,
        platform: platform,
        runtime: runtime,
      );
      await container.read(runtimeControllerProvider.notifier).refresh();
      await tester.pump();

      await tester.tap(find.byKey(const ValueKey('system-proxy-selector')));
      await tester.pumpAndSettle();
      await tester.tap(find.textContaining('Pac 模式').last);
      await tester.pumpAndSettle();

      expect(platform.appliedModes, contains(SysProxyMode.pac));
      expect(platform.pacStartCount, 1);
      expect(platform.pacFilePaths.single, endsWith('pac.txt'));
      final doc = container.read(settingsControllerProvider).document;
      expect(
        (doc['SystemProxyItem'] as Map)['SysProxyType'],
        SysProxyMode.pac.value,
      );
    });

    testWidgets('mode without a running session reports honestly', (
      tester,
    ) async {
      final platform = FakePlatformBridge(initialMode: SysProxyMode.unchanged);
      final runtime = CountingRuntimeBridge();
      final container = await _pumpStatusBar(
        tester,
        platform: platform,
        runtime: runtime,
      );

      await tester.tap(find.byKey(const ValueKey('system-proxy-selector')));
      await tester.pumpAndSettle();
      await tester.tap(find.textContaining('Pac 模式').last);
      await tester.pumpAndSettle();

      expect(platform.pacStartCount, 0);
      final view = container.read(platformControllerProvider);
      expect(view.error?.code, 'E_NO_RUNNING_SESSION');
      expect(view.message, contains('没有运行中的代理会话'));
      final doc = container.read(settingsControllerProvider).document;
      expect(
        (doc['SystemProxyItem'] as Map)['SysProxyType'],
        SysProxyMode.pac.value,
      );
    });

    test(
      'startup restore re-applies the saved mode from the applied port',
      () async {
        final platform = FakePlatformBridge(
          initialMode: SysProxyMode.forcedClear,
        );
        final runtime = CountingRuntimeBridge(initial: _runningView);
        final container = ProviderContainer(
          overrides: [
            bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
            platformBridgeProvider.overrideWithValue(platform),
            runtimeBridgeProvider.overrideWithValue(runtime),
          ],
        );
        addTearDown(container.dispose);
        container.read(settingsControllerProvider.notifier).saveGroup(
          'SystemProxyItem',
          <String, dynamic>{'SysProxyType': SysProxyMode.forcedChange.value},
        );
        await container.read(runtimeControllerProvider.notifier).refresh();

        container
            .read(platformControllerProvider.notifier)
            .restoreAppliedModeOnLaunch();

        expect(platform.appliedModes, contains(SysProxyMode.forcedChange));
        expect(platform.lastServer, '127.0.0.1:11808');
      },
    );
  });
}
