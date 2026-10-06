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

import '../support/counting_runtime_bridge.dart';
import '../support/fake_monitor_bridge.dart';
import '../support/fake_platform_bridge.dart';

/// SP-17 status-bar integration: the strip shows the retained actual A with a
/// visible failure + retry/view entries, the stale platform message is demoted
/// out of the headline, and the original availability entry exists.
Future<ProviderContainer> _pumpStatusBar(
  WidgetTester tester, {
  required RuntimeView initial,
  String? platformMessage,
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
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
      runtimeBridgeProvider.overrideWithValue(
        CountingRuntimeBridge(initial: initial),
      ),
      monitorBridgeProvider.overrideWithValue(monitor),
    ],
  );
  addTearDown(container.dispose);
  if (platformMessage != null) {
    container
        .read(platformControllerProvider.notifier)
        .setMessage(platformMessage);
  }
  await container.read(runtimeControllerProvider.notifier).refresh();
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
  testWidgets('retained actual A with failure offers retry and view', (
    tester,
  ) async {
    final container = await _pumpStatusBar(
      tester,
      initial: const RuntimeView(
        state: 'Running',
        hostAlive: true,
        ports: <int>[11911],
        sessionId: 'synthetic-session-a',
        desiredRevision: null,
        appliedRevision: null,
      ),
      platformMessage: '旧平台消息：系统代理已切换',
    );

    // Drive a failed switch to B through the real controller queue.
    final bridge =
        container.read(runtimeBridgeProvider) as CountingRuntimeBridge;
    bridge.applyError = const RuntimeErrorView(
      code: 'E_CORE_START_FAILED',
      messageKey: 'error.core_start_failed',
    );
    await container
        .read(runtimeControllerProvider.notifier)
        .applyActive(targetId: 'synthetic-b');
    await tester.pump();

    // Actual A retained in the headline nodes.
    expect(find.byKey(const ValueKey('running-node')), findsOneWidget);
    final node = tester
        .widget<Text>(find.byKey(const ValueKey('running-node')))
        .data!;
    expect(node, contains('运行中'));
    expect(node, isNot(contains('synthetic-b')));
    // Failure visible with retry + view entries.
    expect(find.byKey(const ValueKey('runtime-error')), findsOneWidget);
    expect(find.byKey(const ValueKey('runtime-error-retry')), findsOneWidget);
    expect(find.byKey(const ValueKey('runtime-error-view')), findsOneWidget);
    // Stale platform message is not the headline.
    expect(find.text('旧平台消息：系统代理已切换'), findsNothing);
    // Original availability entry exists (ACT-STAT-004).
    expect(
      find.byKey(const ValueKey('running-availability-test')),
      findsOneWidget,
    );
  });

  testWidgets('platform message returns as headline with no failure', (
    tester,
  ) async {
    await _pumpStatusBar(
      tester,
      initial: const RuntimeView(
        state: 'Running',
        hostAlive: true,
        ports: <int>[11911],
        sessionId: 'synthetic-session-a',
      ),
      platformMessage: '系统代理: 直接连接',
    );

    expect(find.byKey(const ValueKey('status-message')), findsOneWidget);
    expect(find.text('系统代理: 直接连接'), findsOneWidget);
    expect(find.byKey(const ValueKey('runtime-error-retry')), findsNothing);
  });
}
