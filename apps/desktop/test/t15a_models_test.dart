import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_format.dart';

import 'support/fake_monitor_bridge.dart';

void main() {
  test('byte and rate formatting scales and shows placeholders', () {
    expect(formatBytes(null), '--');
    expect(formatRate(null), '--');
    expect(formatBytes(0), '0 B/s');
    expect(formatBytes(1024), '1.00 KB/s');
    expect(formatTraffic(1024 * 1024), '1.00 MB');
    expect(formatRate(BigInt.from(2048)), '2.00 KB/s');
    expect(formatRate(-1), '--');
  });

  test('visibleLogs applies level and keyword filters', () {
    final state = MonitorState(
      logs: const <m.LogLineDto>[
        m.LogLineDto(text: 'info start', level: 2, truncated: false),
        m.LogLineDto(text: 'warn disk', level: 3, truncated: false),
        m.LogLineDto(text: 'error disk full', level: 4, truncated: false),
      ],
      minLevel: 3,
    );
    expect(state.visibleLogs.length, 2);
    final keyword = state.copyWith(keyword: 'disk');
    expect(keyword.visibleLogs.length, 2);
    final none = state.copyWith(keyword: 'missing');
    expect(none.visibleLogs, isEmpty);
  });

  test('traffic batch updates state and marks hasTraffic', () async {
    final fake = FakeMonitorBridge();
    addTearDown(fake.disposeStreams);
    final container = ProviderContainer(
      overrides: [monitorBridgeProvider.overrideWithValue(fake)],
    );
    addTearDown(container.dispose);
    final controller = container.read(monitorControllerProvider.notifier);
    controller.configure(
      core: 2,
      statePort: 11809,
      statePort2: 0,
      enableStatistics: true,
      displayRealTimeSpeed: true,
      refreshIntervalMs: 2000,
    );
    fake.emitTraffic(
      proxyUpBps: BigInt.from(100),
      proxyDownBps: BigInt.from(200),
      directUpBps: BigInt.from(1),
      directDownBps: BigInt.from(2),
    );
    await Future<void>.delayed(Duration.zero);
    final state = container.read(monitorControllerProvider);
    expect(state.hasTraffic, true);
    expect(state.proxyDownBps, BigInt.from(200));
    expect(fake.configured.single, 'core=2;11809;0');
  });

  test('clearStats goes through the bridge', () {
    final fake = FakeMonitorBridge();
    addTearDown(fake.disposeStreams);
    final container = ProviderContainer(
      overrides: [monitorBridgeProvider.overrideWithValue(fake)],
    );
    addTearDown(container.dispose);
    final controller = container.read(monitorControllerProvider.notifier);
    expect(controller.clearStats(), true);
    expect(fake.clearStatsCount, 1);
  });

  test('log pauses are forwarded independently', () {
    final fake = FakeMonitorBridge();
    addTearDown(fake.disposeStreams);
    final container = ProviderContainer(
      overrides: [monitorBridgeProvider.overrideWithValue(fake)],
    );
    addTearDown(container.dispose);
    final controller = container.read(monitorControllerProvider.notifier);
    controller.setCollectingPaused(true);
    expect(fake.lastCollectingPaused, true);
    controller.setScrollPaused(true);
    expect(fake.lastScrollPaused, true);
    expect(fake.lastCollectingPaused, true);
  });
}
