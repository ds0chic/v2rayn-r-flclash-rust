// R4-06 contract: the bottom bar exposes identifiable, clickable partitions
// and reads real running state / rates / stats.
//
// One page build covers the whole card contract because the locked
// flutter_tester leaks native resources per `pumpWidget`.
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/status_bar_view.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_controller.dart';

import 'support/counting_runtime_bridge.dart';
import 'support/dpi_assertions.dart';
import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';

void main() {
  testWidgets('status bar partitions are identifiable and clickable', (
    tester,
  ) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    applyDpi(tester, 1.0, const Size(1440, 200));

    final platform = FakePlatformBridge(initialMode: SysProxyMode.unchanged);
    final monitor = FakeMonitorBridge();
    addTearDown(monitor.disposeStreams);
    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(SyntheticBridgePort(count: 3)),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(3),
        platformBridgeProvider.overrideWithValue(platform),
        // No live session: the bar must say so honestly, never fabricate.
        runtimeBridgeProvider.overrideWithValue(CountingRuntimeBridge()),
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
    await tester.pump(const Duration(milliseconds: 20));

    // Identifiable controls: the two-line ports, Tun switch, and the
    // system-proxy / routing selectors all rendered as real widgets.
    for (final key in <String>[
      'status-inbound',
      'status-inbound-lan',
      'tun-toggle',
      'tun-actual',
      'system-proxy-selector',
      'status-sysproxy',
      'routing-mode-selector',
      'routing-selector',
      'running-node',
      'running-info',
      'status-proxy-speed',
      'status-direct-speed',
      'status-today-traffic',
      'status-details',
    ]) {
      expect(
        find.byKey(ValueKey<String>(key)),
        findsWidgets,
        reason: 'missing $key',
      );
    }

    // Readable, honest values with no session.
    final runningNode = tester
        .widget<Text>(find.byKey(const ValueKey('running-node')))
        .data!;
    expect(runningNode.contains('未运行'), isTrue, reason: runningNode);
    final proxySpeed = tester
        .widget<Text>(find.byKey(const ValueKey('status-proxy-speed')))
        .data!;
    expect(proxySpeed.contains('--'), isTrue, reason: proxySpeed);

    // The selectors carry a visible dropdown affordance (not bare status text).
    for (final key in <String>[
      'system-proxy-selector',
      'routing-mode-selector',
      'routing-selector',
    ]) {
      expect(
        find.descendant(
          of: find.byKey(ValueKey<String>(key)),
          matching: find.byIcon(Icons.arrow_drop_down),
        ),
        findsWidgets,
        reason: '$key has no dropdown affordance',
      );
    }

    // Clicking the system-proxy control routes through the shared use case.
    // With no live session it must report honestly instead of faking success.
    await tester.ensureVisible(
      find.byKey(const ValueKey('system-proxy-selector')),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('system-proxy-selector')));
    await tester.pumpAndSettle();
    await tester.tap(find.textContaining('Pac 模式').last);
    await tester.pumpAndSettle();
    final platformView = container.read(platformControllerProvider);
    expect(platformView.error?.code, 'E_NO_RUNNING_SESSION');
    expect(platformView.message, contains('没有运行中的代理会话'));
    expect(platform.appliedModes, isEmpty);

    // Technical diagnostics moved into the details popup.
    expect(find.byKey(const ValueKey('runtime-info')), findsNothing);
    await tester.ensureVisible(find.byKey(const ValueKey('status-details')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('status-details')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('runtime-info')), findsOneWidget);
    expect(find.byKey(const ValueKey('runtime-revision')), findsOneWidget);
    expect(find.byKey(const ValueKey('status-counts')), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();

    // Partitions do not overlap; controls stay hittable at 100% and 125% DPI.
    for (final scale in <double>[1.0, 1.25]) {
      applyDpi(tester, scale, const Size(1440, 200));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 20));
      expect(tester.takeException(), isNull, reason: 'overflow at ${scale}x');
      final proxy = tester.getRect(
        find.byKey(const ValueKey('system-proxy-selector')),
      );
      final mode = tester.getRect(
        find.byKey(const ValueKey('routing-mode-selector')),
      );
      final route = tester.getRect(
        find.byKey(const ValueKey('routing-selector')),
      );
      expect(
        mode.left,
        greaterThanOrEqualTo(proxy.right - 0.5),
        reason: 'routing-mode overlaps system-proxy at ${scale}x',
      );
      expect(
        route.left,
        greaterThanOrEqualTo(mode.right - 0.5),
        reason: 'routing overlaps routing-mode at ${scale}x',
      );
      for (final key in <String>[
        'system-proxy-selector',
        'tun-toggle',
        'routing-selector',
        'status-details',
      ]) {
        final finder = find.byKey(ValueKey<String>(key));
        await tester.ensureVisible(finder);
        await tester.pump();
        expect(
          finder.hitTestable(),
          findsWidgets,
          reason: '$key unreachable at ${scale}x',
        );
      }
    }
  });
}
