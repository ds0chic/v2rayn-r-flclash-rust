import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/logs_view.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_incremental.dart';

import 'support/fake_monitor_bridge.dart';

/// SP-22: log rows coalesce for [logCoalesceWindow]; pump past it so the
/// flush timer fires before reading rows. Counters apply immediately.
Future<void> pumpLogFlush(WidgetTester tester) =>
    tester.pump(logCoalesceWindow + const Duration(milliseconds: 100));

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
    await pumpLogFlush(tester);
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
    await pumpLogFlush(tester);
    await tester.pump();

    final container = ProviderScope.containerOf(
      tester.element(find.byType(LogsView)),
    );
    container.read(monitorControllerProvider.notifier).setMinLevel(4);
    await tester.pump();

    expect(find.textContaining('error two'), findsOneWidget);
    expect(find.textContaining('info one'), findsNothing);
    // Level filtering is presentation-only: the Rust ring is untouched.
    expect(fake.setLogFilterCalls, 0);
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

  testWidgets('clear button clears the ring and shows the upstream marker', (
    tester,
  ) async {
    final fake = await pumpLogs(tester);
    fake.logs = const <m.LogLineDto>[
      m.LogLineDto(text: 'keep me', level: 2, truncated: false),
    ];
    await tester.ensureVisible(find.byKey(const ValueKey('logs-clear')));
    await tester.tap(find.byKey(const ValueKey('logs-clear')));
    await tester.pump();
    expect(fake.clearLogsCount, 1);
    expect(find.textContaining('Message cleared'), findsOneWidget);
    expect(find.textContaining('keep me'), findsNothing);
  });
}
