import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/connections_view.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/monitor/proxies_view.dart';

import 'support/fake_monitor_bridge.dart';

const _group = m.ClashProxyDto(
  name: 'GROUP',
  proxyType: 'Selector',
  isGroup: true,
  now: 'A',
  all: <String>['A', 'B'],
  delay: 0,
  provider: null,
);

Future<FakeMonitorBridge> pumpProxies(
  WidgetTester tester, {
  FakeMonitorBridge? bridge,
}) async {
  final fake =
      bridge ??
      FakeMonitorBridge(
        clashApiSupported: true,
        proxies: const <m.ClashProxyDto>[_group],
        clashMode: 'Rule',
      );
  addTearDown(fake.disposeStreams);
  final container = ProviderContainer(
    overrides: [monitorBridgeProvider.overrideWithValue(fake)],
  );
  addTearDown(container.dispose);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: ProxiesView())),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 20));
  return fake;
}

Future<FakeMonitorBridge> pumpConnections(
  WidgetTester tester, {
  FakeMonitorBridge? bridge,
}) async {
  final fake = bridge ?? FakeMonitorBridge(clashApiSupported: true);
  addTearDown(fake.disposeStreams);
  final container = ProviderContainer(
    overrides: [monitorBridgeProvider.overrideWithValue(fake)],
  );
  addTearDown(container.dispose);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: ConnectionsView())),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 20));
  return fake;
}

void main() {
  testWidgets(
    'no running session / unsupported core is disabled with a message',
    (tester) async {
      await pumpProxies(tester, bridge: FakeMonitorBridge());
      expect(find.byKey(const ValueKey('proxies-unsupported')), findsOneWidget);
      expect(find.byKey(const ValueKey('proxies-mode')), findsNothing);
    },
  );

  testWidgets('mode selector reflects the live mode and switches via bridge', (
    tester,
  ) async {
    final fake = await pumpProxies(tester);
    expect(find.byKey(const ValueKey('proxies-mode')), findsOneWidget);
    expect(find.text('Rule'), findsWidgets);

    await tester.tap(find.byKey(const ValueKey('proxies-mode')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('proxies-mode-Global')).last);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 20));
    expect(fake.clashMode, 'Global');
  });

  testWidgets('failed mode switch surfaces an error, not a fake success', (
    tester,
  ) async {
    final fake = await pumpProxies(
      tester,
      bridge: FakeMonitorBridge(
        clashApiSupported: true,
        proxies: const <m.ClashProxyDto>[_group],
        clashMode: 'Rule',
        modeFails: true,
      ),
    );
    await tester.tap(find.byKey(const ValueKey('proxies-mode')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('proxies-mode-Direct')).last);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 20));
    expect(fake.clashMode, 'Rule');
    final tooltip = tester.widget<Tooltip>(
      find.ancestor(
        of: find.byKey(const ValueKey('proxies-mode')),
        matching: find.byType(Tooltip),
      ),
    );
    expect(tooltip.message, isNotNull);
    expect(tooltip.message, isNot('Clash 运行模式'));
  });

  testWidgets('close-all failure reports instead of silently succeeding', (
    tester,
  ) async {
    final fake = await pumpConnections(tester);
    fake.closeAllFails = true;
    await tester.tap(find.byKey(const ValueKey('connections-close-all')));
    await tester.pump();
    expect(
      find.byKey(const ValueKey('connections-action-error')),
      findsOneWidget,
    );
  });
}
