import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';

import 'support/fake_monitor_bridge.dart';
import 'support/profiles_harness.dart';

void main() {
  testWidgets('status bar shows placeholders with no statistics data', (
    tester,
  ) async {
    final monitor = FakeMonitorBridge();
    addTearDown(monitor.disposeStreams);
    await pumpApp(tester, rows: 10, monitor: monitor);
    final text = tester
        .widget<Text>(find.byKey(const ValueKey('status-proxy-speed')))
        .data!;
    expect(text.contains('--'), isTrue);
  });

  testWidgets('status bar shows real proxy/direct rates from the stream', (
    tester,
  ) async {
    final monitor = FakeMonitorBridge();
    addTearDown(monitor.disposeStreams);
    final container = await pumpApp(tester, rows: 10, monitor: monitor);
    container
        .read(monitorControllerProvider.notifier)
        .configure(
          core: 2,
          statePort: 11809,
          statePort2: 0,
          enableStatistics: true,
          displayRealTimeSpeed: true,
          refreshIntervalMs: 1000,
        );
    monitor.emitTraffic(
      proxyUpBps: BigInt.from(1024),
      proxyDownBps: BigInt.from(2048),
      directUpBps: BigInt.from(0),
      directDownBps: BigInt.from(512),
    );
    await tester.pump();

    final proxy = tester
        .widget<Text>(find.byKey(const ValueKey('status-proxy-speed')))
        .data!;
    expect(proxy.contains('1.00 KB/s'), isTrue);
    expect(proxy.contains('2.00 KB/s'), isTrue);

    final direct = tester
        .widget<Text>(find.byKey(const ValueKey('status-direct-speed')))
        .data!;
    expect(direct.contains('512 B/s'), isTrue);
  });
}
