import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/monitor/connections_view.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/monitor/proxies_view.dart';

import 'support/fake_monitor_bridge.dart';

/// D3b-02: every monitor page marks itself visible on open and must mark
/// itself invisible on dispose ("only visible pages are subscribed").
Future<FakeMonitorBridge> pumpView(WidgetTester tester, Widget view) async {
  final fake = FakeMonitorBridge(clashApiSupported: true);
  addTearDown(fake.disposeStreams);
  final container = ProviderContainer(
    overrides: [monitorBridgeProvider.overrideWithValue(fake)],
  );
  addTearDown(container.dispose);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(home: Scaffold(body: view)),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 20));
  return fake;
}

Future<void> disposeView(WidgetTester tester) =>
    tester.pumpWidget(const MaterialApp(home: Scaffold(body: SizedBox())));

void main() {
  testWidgets('connections page marks invisible on dispose', (tester) async {
    final fake = await pumpView(tester, const ConnectionsView());
    expect(fake.pageVisibility['connections'], isTrue);
    await disposeView(tester);
    expect(fake.pageVisibility['connections'], isFalse);
  });

  testWidgets('proxies page marks invisible on dispose', (tester) async {
    final fake = await pumpView(tester, const ProxiesView());
    expect(fake.pageVisibility['proxies'], isTrue);
    await disposeView(tester);
    expect(fake.pageVisibility['proxies'], isFalse);
  });
}
