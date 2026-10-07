import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/connections_view.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_incremental.dart';

import 'support/fake_monitor_bridge.dart';

m.ClashConnectionDto _connection(String id, String host) =>
    m.ClashConnectionDto(
      id: id,
      host: host,
      network: 'tcp',
      connectionType: 'HTTP',
      chains: const <String>['A'],
      rule: 'MATCH',
      processPath: 'C:/app.exe',
      source: '127.0.0.1:5000',
      destination: '93.184.216.34:443',
      upload: BigInt.from(10),
      download: BigInt.from(20),
      start: '2026-01-01T00:00:00Z',
    );

Future<FakeMonitorBridge> pumpConnections(
  WidgetTester tester, {
  FakeMonitorBridge? bridge,
}) async {
  final fake =
      bridge ??
      FakeMonitorBridge(
        clashApiSupported: true,
        connections: <m.ClashConnectionDto>[
          _connection('c1', 'example.com'),
          _connection('c2', 'other.org'),
        ],
      );
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
  testWidgets('non-clash core shows the unsupported message', (tester) async {
    await pumpConnections(tester, bridge: FakeMonitorBridge());
    expect(
      find.byKey(const ValueKey('connections-unsupported')),
      findsOneWidget,
    );
  });

  testWidgets('renders connection rows and filters by host', (tester) async {
    await pumpConnections(tester);
    expect(find.byKey(const ValueKey('connections-list')), findsOneWidget);
    expect(find.text('example.com'), findsOneWidget);
    expect(find.text('other.org'), findsOneWidget);

    await tester.enterText(
      find.byKey(const ValueKey('connections-filter')),
      'example',
    );
    // SP-22: the filter needle is debounced for [filterDebounceWindow].
    await tester.pump(filterDebounceWindow + const Duration(milliseconds: 100));
    await tester.pump();
    expect(find.text('example.com'), findsOneWidget);
    expect(find.text('other.org'), findsNothing);
  });

  testWidgets('close selected and close all reach the bridge', (tester) async {
    final fake = await pumpConnections(tester);

    // Tapping the row body toggles its selection (DataTable checkbox column).
    await tester.tap(find.text('example.com').first);
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('connections-close-selected')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 20));
    expect(fake.closedConnections, contains('c1'));

    await tester.tap(find.byKey(const ValueKey('connections-close-all')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 20));
    expect(fake.closeAllCount, 1);
  });
}
