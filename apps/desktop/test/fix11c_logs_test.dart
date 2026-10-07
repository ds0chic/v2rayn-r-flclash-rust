import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_format.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_incremental.dart';

import 'support/fake_monitor_bridge.dart';

/// SP-22: log rows coalesce for [logCoalesceWindow]; counters/pause flags
/// still apply immediately. Await this after an emit before reading rows.
Future<void> flushLogRows() =>
    Future<void>.delayed(logCoalesceWindow + const Duration(milliseconds: 100));

m.LogLineDto line(String text, {int level = 2, bool truncated = false}) =>
    m.LogLineDto(text: text, level: level, truncated: truncated);

({
  ProviderContainer container,
  FakeMonitorBridge bridge,
  MonitorController controller,
})
harness() {
  final bridge = FakeMonitorBridge();
  final container = ProviderContainer(
    overrides: [monitorBridgeProvider.overrideWithValue(bridge)],
  );
  addTearDown(container.dispose);
  addTearDown(bridge.disposeStreams);
  return (
    container: container,
    bridge: bridge,
    controller: container.read(monitorControllerProvider.notifier),
  );
}

void main() {
  test('formatLogsForCopy matches upstream newline-joined text', () {
    expect(formatLogsForCopy(const <m.LogLineDto>[]), '');
    expect(
      formatLogsForCopy(<m.LogLineDto>[
        line('a'),
        const m.LogLineDto(text: 'b\n', level: 2, truncated: false),
      ]),
      'a\nb\n',
    );
  });

  test('keyword/level filtering is presentation-only', () async {
    final h = harness();
    h.controller.setPageVisible('logs', true);
    h.bridge.emitLogs(<m.LogLineDto>[
      line('info start'),
      line('warn disk', level: 3),
      line('error disk full', level: 4),
    ]);
    await flushLogRows();
    final state = h.container.read(monitorControllerProvider);
    expect(state.logs.length, 3);

    h.controller.setMinLevel(3);
    expect(h.container.read(monitorControllerProvider).visibleLogs.length, 2);

    h.controller.setKeyword('disk');
    expect(h.container.read(monitorControllerProvider).visibleLogs.length, 2);

    // Invalid regex falls back to substring matching.
    h.controller.setKeyword('[');
    expect(h.container.read(monitorControllerProvider).visibleLogs, isEmpty);
  });

  test('collection pause freezes the view even if a batch arrives', () {
    final h = harness();
    h.controller.setPageVisible('logs', true);
    h.controller.setCollectingPaused(true);
    expect(h.bridge.lastCollectingPaused, true);

    h.bridge.emitLogs(<m.LogLineDto>[
      line('should not appear'),
    ], collectingPaused: true);
    final state = h.container.read(monitorControllerProvider);
    expect(state.logs, isEmpty);
    expect(state.collectingPaused, true);
  });

  test('hidden page keeps collecting but freezes UI, then resyncs', () {
    final h = harness();
    h.controller.setPageVisible('logs', true);
    h.controller.setPageVisible('logs', false);
    expect(h.bridge.pageVisibility['logs'], false);

    h.bridge.emitLogs(<m.LogLineDto>[line('while hidden')]);
    expect(h.container.read(monitorControllerProvider).logs, isEmpty);

    // The Rust ring retained it; becoming visible resyncs the tail.
    h.bridge.logs = <m.LogLineDto>[line('while hidden')];
    h.controller.setPageVisible('logs', true);
    expect(
      h.container.read(monitorControllerProvider).logs.single.text,
      'while hidden',
    );
  });

  test('auto-refresh toggle freezes and resyncs the view', () {
    final h = harness();
    h.controller.setPageVisible('logs', true);
    h.controller.setAutoRefresh(false);

    h.bridge.emitLogs(<m.LogLineDto>[line('frozen out')]);
    expect(h.container.read(monitorControllerProvider).logs, isEmpty);

    h.bridge.logs = <m.LogLineDto>[line('frozen out')];
    h.controller.setAutoRefresh(true);
    expect(
      h.container.read(monitorControllerProvider).logs.single.text,
      'frozen out',
    );
  });

  test('auto-scroll toggle drives the Rust scroll-pause flag', () {
    final h = harness();
    h.controller.setPageVisible('logs', true);
    h.controller.setScrollPaused(true);
    expect(h.bridge.lastScrollPaused, true);
    expect(h.container.read(monitorControllerProvider).scrollPaused, true);
  });

  test('clear empties the ring and shows the upstream marker', () async {
    final h = harness();
    h.controller.setPageVisible('logs', true);
    h.bridge.emitLogs(<m.LogLineDto>[line('gone')]);
    await flushLogRows();
    expect(h.container.read(monitorControllerProvider).logs.length, 1);

    h.controller.clearLogs();
    final state = h.container.read(monitorControllerProvider);
    expect(h.bridge.clearLogsCount, 1);
    expect(state.logs.single.text, '----- Message cleared -----');
  });

  test('display buffer is bounded under a large burst', () async {
    final h = harness();
    h.controller.setPageVisible('logs', true);
    h.bridge.emitLogs(<m.LogLineDto>[
      for (var i = 0; i < 2600; i++) line('line $i'),
    ]);
    await flushLogRows();
    final state = h.container.read(monitorControllerProvider);
    expect(state.logs.length, maxDisplayedLogs);
    expect(state.logs.last.text, 'line 2599');
  });
}
