import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
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

const _nodeA = m.ClashProxyDto(
  name: 'A',
  proxyType: 'Shadowsocks',
  isGroup: false,
  now: null,
  all: <String>[],
  delay: 55,
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
        proxies: const <m.ClashProxyDto>[_group, _nodeA],
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

void main() {
  testWidgets('non-clash core shows the unsupported message', (tester) async {
    await pumpProxies(tester, bridge: FakeMonitorBridge());
    expect(find.byKey(const ValueKey('proxies-unsupported')), findsOneWidget);
    expect(find.text('当前内核不提供 Clash API'), findsOneWidget);
  });

  testWidgets('renders groups/nodes and selects a group member', (
    tester,
  ) async {
    final fake = await pumpProxies(tester);
    expect(find.byKey(const ValueKey('proxies-list')), findsOneWidget);
    expect(find.byKey(const ValueKey('proxies-group-GROUP')), findsOneWidget);
    expect(find.byKey(const ValueKey('proxies-node-A')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('proxies-select-GROUP-B')));
    await tester.pump();
    expect(fake.selectedProxies, contains('GROUP/B'));
  });

  testWidgets('single-node and group delay probes hit the bridge', (
    tester,
  ) async {
    final fake = await pumpProxies(tester);
    await tester.tap(find.byKey(const ValueKey('proxies-node-delay-A')).last);
    await tester.pump();
    expect(fake.testedProxies, contains('A'));

    await tester.tap(find.byKey(const ValueKey('proxies-group-delay-GROUP')));
    await tester.pump();
    expect(fake.testedGroups, contains('GROUP'));
    // Group probe populated both children.
    expect(find.byKey(const ValueKey('proxies-child-delay-B')), findsOneWidget);
  });
}
