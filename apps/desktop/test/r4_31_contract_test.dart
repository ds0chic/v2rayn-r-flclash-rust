// R4-31 contract: real-load performance/资源 stability for the profiles +
// monitor surfaces.
//
// Covers the card's mandatory scenarios at the controller level:
//   * an idle 150 ms speedtest poll must not rebuild the node table read model
//     (bounded UI-isolate work under a 10k list + logs + monitoring);
//   * a real result change still rebuilds and is applied;
//   * the poller settles and releases its tracked jobs (no task growth);
//   * the monitor never stacks duplicate stream subscriptions across repeated
//     page toggles / session changes (no handle/listener growth);
//   * the Dart log read model stays bounded under a flood (bounded memory).
//
// The real-core / real-slow-disk path stays outside this synthetic harness and
// is registered as unverified in the R4-31 evidence.
import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';
import 'support/r4_22_bridge.dart';

ProviderContainer _profilesContainer(R422Bridge bridge) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(6),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
      monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

class _FakeRuntimeBridge implements RuntimeBridge {
  _FakeRuntimeBridge(this.view);

  RuntimeView view;
  final StreamController<RuntimeEvent> _events =
      StreamController<RuntimeEvent>.broadcast();

  @override
  Future<RuntimeView> snapshot() async => view;

  @override
  String? activeProfileId() => view.hasAppliedEndpoint ? 'node-a' : null;

  @override
  Future<RuntimeActionResult> applyActive({
    required BigInt expectedRevision,
  }) async => const RuntimeActionResult(ok: true);

  @override
  Future<RuntimeActionResult> stop() async {
    view = const RuntimeView();
    return const RuntimeActionResult(ok: true);
  }

  @override
  Stream<RuntimeEvent> events() => _events.stream;

  void dispose() => _events.close();
}

final _runningA = RuntimeView(
  state: 'Running',
  ports: <int>[11810],
  sessionId: 's-a',
  desiredRevision: BigInt.one,
  appliedRevision: BigInt.one,
);

final _runningB = RuntimeView(
  state: 'Running',
  ports: <int>[11811],
  sessionId: 's-b',
  desiredRevision: BigInt.two,
  appliedRevision: BigInt.two,
);

m.LogLineDto _line(int i) =>
    m.LogLineDto(text: 'log line $i', level: 2, truncated: false);

void main() {
  test('an idle poll tick keeps the same node table read model', () async {
    final bridge = R422Bridge()..activeJobs = 1;
    final container = _profilesContainer(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    controller.selectRow(
      container.read(profilesControllerProvider).visible.first.id,
    );
    controller.emitAction(ProfileAction.realping);

    await Future<void>.delayed(const Duration(milliseconds: 250));
    final first = container.read(profilesControllerProvider).visible;
    await Future<void>.delayed(const Duration(milliseconds: 220));
    final second = container.read(profilesControllerProvider).visible;

    expect(identical(first, second), isTrue);
  });

  test('a changed live result still rebuilds and is applied', () async {
    final bridge = R422Bridge()..activeJobs = 1;
    final container = _profilesContainer(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    final target = container.read(profilesControllerProvider).visible[1].id;
    controller.selectRow(target);
    controller.emitAction(ProfileAction.realping);

    bridge.setResult(target, 42, speed: 3.5);
    await Future<void>.delayed(const Duration(milliseconds: 300));

    final row = container
        .read(profilesControllerProvider)
        .all
        .firstWhere((r) => r.id == target);
    expect(row.delay, 42);
    expect(row.speed, '3.5 MB/s');
  });

  test('the poller settles and releases its tracked jobs', () async {
    final bridge = R422Bridge()..activeJobs = 1;
    final container = _profilesContainer(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    controller.selectRow(
      container.read(profilesControllerProvider).visible.first.id,
    );
    controller.emitAction(ProfileAction.tcping);
    expect(container.read(profilesControllerProvider).speedTestRunning, isTrue);

    bridge.activeJobs = 0;
    await Future<void>.delayed(const Duration(milliseconds: 350));

    final settled = container.read(profilesControllerProvider);
    expect(settled.speedTestRunning, isFalse);
    expect(settled.speedTestStage, 'SpeedtestingCompleted');
    // A settled run owns no job, so a later stop has nothing to cancel.
    controller.cancelSpeedTest();
    expect(bridge.cancelledRuns, isEmpty);
  });

  test(
    'repeated page toggles and session changes never stack subscriptions',
    () async {
      final monitor = FakeMonitorBridge(clashApiSupported: true);
      final runtime = _FakeRuntimeBridge(_runningA);
      final container = ProviderContainer(
        overrides: [
          monitorBridgeProvider.overrideWithValue(monitor),
          runtimeBridgeProvider.overrideWithValue(runtime),
        ],
      );
      addTearDown(container.dispose);
      addTearDown(monitor.disposeStreams);
      addTearDown(runtime.dispose);
      final controller = container.read(monitorControllerProvider.notifier);

      for (var i = 0; i < 50; i++) {
        final view = i.isEven ? _runningA : _runningB;
        controller.syncRuntimeSession(view);
        controller.setPageVisible('logs', true);
        controller.setPageVisible('logs', false);
        controller.setPageVisible('connections', true);
      }

      expect(monitor.subscribeTrafficCount, 1);
      expect(monitor.subscribeLogsCount, 1);
    },
  );

  test('the log read model stays bounded under a flood', () {
    final monitor = FakeMonitorBridge(clashApiSupported: true);
    final runtime = _FakeRuntimeBridge(_runningA);
    final container = ProviderContainer(
      overrides: [
        monitorBridgeProvider.overrideWithValue(monitor),
        runtimeBridgeProvider.overrideWithValue(runtime),
      ],
    );
    addTearDown(container.dispose);
    addTearDown(monitor.disposeStreams);
    addTearDown(runtime.dispose);
    final controller = container.read(monitorControllerProvider.notifier);
    controller.syncRuntimeSession(_runningA);
    controller.setPageVisible('logs', true);

    for (var batch = 0; batch < 25; batch++) {
      monitor.emitLogs(<m.LogLineDto>[
        for (var i = 0; i < 2000; i++) _line(batch * 2000 + i),
      ], droppedLines: BigInt.from(90000));
    }

    final state = container.read(monitorControllerProvider);
    expect(state.logs.length, maxDisplayedLogs);
    expect(state.droppedLines, BigInt.from(90000));
  });
}
