import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/logs_view.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';

import 'support/fake_monitor_bridge.dart';

/// D3b-06: the log keyword regex compiles once per pattern, and traffic-only
/// rebuilds never yank a scroll position away from history.
void main() {
  test('log keyword regex compiles once per pattern', () {
    final first = compileLogFilter('disk (?:error|warn)');
    expect(first, isNotNull);
    expect(identical(compileLogFilter('disk (?:error|warn)'), first), isTrue);
    expect(compileLogFilter('other'), isNot(first));
    expect(compileLogFilter('['), isNull);
  });

  testWidgets('traffic-only rebuilds keep the history scroll position', (
    tester,
  ) async {
    final fake = FakeMonitorBridge(
      initialLogs: List<m.LogLineDto>.generate(
        200,
        (i) => m.LogLineDto(text: 'line $i', level: 2, truncated: false),
      ),
    );
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

    double offset() => tester
        .state<ScrollableState>(
          find.descendant(
            of: find.byKey(const ValueKey('logs-list')),
            matching: find.byType(Scrollable),
          ),
        )
        .position
        .pixels;
    double maxExtent() => tester
        .state<ScrollableState>(
          find.descendant(
            of: find.byKey(const ValueKey('logs-list')),
            matching: find.byType(Scrollable),
          ),
        )
        .position
        .maxScrollExtent;

    // Tailed to the end after the initial load.
    expect(maxExtent(), greaterThan(0));
    expect(offset(), maxExtent());

    // Read history: scroll up, then a traffic-only batch rebuilds the page.
    await tester.drag(
      find.byKey(const ValueKey('logs-list')),
      const Offset(0, 400),
    );
    await tester.pumpAndSettle();
    final history = offset();
    expect(history, lessThan(maxExtent()));

    fake.emitTraffic(proxyUpBps: BigInt.from(7));
    await tester.pump();
    expect(offset(), history);
  });
}
