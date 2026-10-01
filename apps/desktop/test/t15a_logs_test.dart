import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/logs_view.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';

import 'support/fake_monitor_bridge.dart';

Future<FakeMonitorBridge> pumpLogs(WidgetTester tester) async {
  final fake = FakeMonitorBridge();
  addTearDown(fake.disposeStreams);
  final container = ProviderContainer(
    overrides: [monitorBridgeProvider.overrideWithValue(fake)],
  );
  addTearDown(container.dispose);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: LogsView())),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 20));
  return fake;
}

void main() {
  testWidgets('logs view subscribes and renders streamed lines', (
    tester,
  ) async {
    final fake = await pumpLogs(tester);
    expect(fake.pageVisibility['logs'], true);

    fake.emitLogs(const <m.LogLineDto>[
      m.LogLineDto(text: '[Info] core started', level: 2, truncated: false),
      m.LogLineDto(text: '[Error] boom', level: 4, truncated: true),
    ]);
    await tester.pump();

    expect(find.byKey(const ValueKey('logs-list')), findsOneWidget);
    expect(find.textContaining('core started'), findsOneWidget);
    expect(find.textContaining('boom'), findsOneWidget);
    // Overflow/truncation hint is visible.
    expect(find.byKey(const ValueKey('logs-overflow')), findsOneWidget);
  });

  testWidgets('logs view shows the empty placeholder with no data', (
    tester,
  ) async {
    await pumpLogs(tester);
    expect(find.byKey(const ValueKey('logs-empty')), findsOneWidget);
  });

  testWidgets('level filter hides lower-severity lines', (tester) async {
    final fake = await pumpLogs(tester);
    fake.emitLogs(const <m.LogLineDto>[
      m.LogLineDto(text: 'info one', level: 2, truncated: false),
      m.LogLineDto(text: 'error two', level: 4, truncated: false),
    ]);
    await tester.pump();

    final container = ProviderScope.containerOf(
      tester.element(find.byType(LogsView)),
    );
    container.read(monitorControllerProvider.notifier).setMinLevel(4);
    await tester.pump();

    expect(find.textContaining('error two'), findsOneWidget);
    expect(find.textContaining('info one'), findsNothing);
    expect(fake.lastMinLevel, 4);
  });

  testWidgets('pause-collect button toggles the bridge pause flag', (
    tester,
  ) async {
    final fake = await pumpLogs(tester);
    await tester.tap(find.byKey(const ValueKey('logs-pause-collect')));
    await tester.pump();
    expect(fake.lastCollectingPaused, true);
    expect(find.text('继续采集'), findsOneWidget);
  });

  testWidgets('clear button is disabled per upstream FND-004', (tester) async {
    await pumpLogs(tester);
    final clear = tester.widget<TextButton>(
      find.byKey(const ValueKey('logs-clear')),
    );
    expect(clear.onPressed, isNull);
  });
}
