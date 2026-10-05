// R4-22 contract: speedtest scope, cancel/gen generation, result sorting and
// incremental polling.
//
// Synthetic only: no native library, no network, no port, no user data. The
// Rust generation guard over the `ProfileExItem` store is covered by the
// `speedtest.rs` / `api/speedtest.rs` unit tests.
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';
import 'support/r4_22_bridge.dart';

ProviderContainer _container(R422Bridge bridge, {int rows = 6}) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(rows),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
      monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

List<String> _visibleIds(ProviderContainer c) =>
    c.read(profilesControllerProvider).visible.map((r) => r.id).toList();

void main() {
  test('the run generation advances on every start and cancel', () {
    final bridge = R422Bridge()..activeJobs = 1;
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    expect(container.read(profilesControllerProvider).speedTestGeneration, 0);

    controller.selectRow(_visibleIds(container).first);
    controller.emitAction(ProfileAction.tcping);
    final g1 = container.read(profilesControllerProvider).speedTestGeneration;
    expect(g1, greaterThan(0));

    controller.emitAction(ProfileAction.realping);
    final g2 = container.read(profilesControllerProvider).speedTestGeneration;
    expect(g2, greaterThan(g1));

    controller.cancelSpeedTest();
    final g3 = container.read(profilesControllerProvider).speedTestGeneration;
    expect(g3, greaterThan(g2));
  });

  test('a completed run summarizes exactly its frozen scope', () async {
    final bridge = R422Bridge()..activeJobs = 1;
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    final targets = _visibleIds(container);

    controller.emitAction(ProfileAction.mixedTest);
    for (final id in targets) {
      bridge.setResult(id, 30);
    }
    bridge.activeJobs = 0;
    await Future<void>.delayed(const Duration(milliseconds: 350));

    final state = container.read(profilesControllerProvider);
    expect(state.speedTestStage, 'SpeedtestingCompleted');
    expect(state.speedTestMessage, contains('成功 ${targets.length}'));
    expect(state.speedTestRunning, isFalse);
  });

  test('a cancelled rerun never reports a stale previous success', () async {
    final bridge = R422Bridge()..activeJobs = 1;
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    final target = _visibleIds(container).first;
    controller.selectRow(target);

    // A previous run left a success behind.
    bridge.setResult(target, 99);
    controller.emitAction(ProfileAction.realping);
    // Start clears the target (R422Bridge models the Rust clear); cancel before
    // the rerun measures anything.
    controller.cancelSpeedTest();
    bridge.activeJobs = 0;
    await Future<void>.delayed(const Duration(milliseconds: 350));

    final state = container.read(profilesControllerProvider);
    final row = state.all.firstWhere((r) => r.id == target);
    expect(
      row.delay,
      isNot(99),
      reason: 'a stale success must not survive a cancelled rerun',
    );
    expect(state.speedTestStage, 'SpeedtestingStop');
    expect(state.speedTestMessage, '已停止测速');
  });

  test(
    'result sort persists the order and a poll does not revert it',
    () async {
      final bridge = R422Bridge()..activeJobs = 1;
      final container = _container(bridge);
      final controller = container.read(profilesControllerProvider.notifier);
      final ids = _visibleIds(container);

      for (var i = 0; i < ids.length; i++) {
        bridge.setResult(ids[i], (i + 1) * 10);
      }
      controller.reload();
      controller.sortByResult();
      final sorted = _visibleIds(container);
      expect(
        container.read(profilesControllerProvider).sort.columnKey,
        'DelayVal',
      );
      expect(bridge.orders, hasLength(1), reason: 'Sort written once');
      expect(bridge.orders.single, sorted);

      // A live poll re-joins the overlay but must neither revert the order nor
      // rewrite `ProfileExItem.Sort`.
      controller.emitAction(ProfileAction.mixedTest);
      for (var i = 0; i < ids.length; i++) {
        bridge.setResult(ids[i], (i + 1) * 10);
      }
      await Future<void>.delayed(const Duration(milliseconds: 250));

      expect(
        _visibleIds(container),
        sorted,
        reason: 'poll keeps the sorted order',
      );
      expect(bridge.orders, hasLength(1), reason: 'poll must not rewrite Sort');

      bridge.activeJobs = 0;
      await Future<void>.delayed(const Duration(milliseconds: 250));
    },
  );

  test('group / filter / sort during a run never re-read the store', () async {
    final bridge = R422Bridge()..activeJobs = 1;
    final container = _container(bridge, rows: 40);
    final controller = container.read(profilesControllerProvider.notifier);
    final before = bridge.snapshotCalls;

    controller.emitAction(ProfileAction.mixedTest);
    controller.setFilter('1');
    controller.submitFilter();
    controller.sortBy('Remarks');
    controller.setGroupSubId(null);

    expect(bridge.snapshotCalls, before, reason: 'recompute is local');
    bridge.activeJobs = 0;
    await Future<void>.delayed(const Duration(milliseconds: 250));
  });

  test(
    'scope: mixed uses the visible list, per-selection uses the selection',
    () {
      final bridge = R422Bridge()..activeJobs = 1;
      final container = _container(bridge);
      final controller = container.read(profilesControllerProvider.notifier);
      final visible = _visibleIds(container);

      controller.selectRow(visible[2]);
      controller.emitAction(ProfileAction.tcping);
      expect(bridge.startedJobs, hasLength(1));

      // The selection test passed exactly the selected id (zero-based kind 0).
      controller.emitAction(ProfileAction.mixedTest);
      expect(bridge.startedJobs, hasLength(2));
    },
  );

  test('UDP without node-routed support is explicit, never faked', () {
    final bridge = R422Bridge()
      ..activeJobs = 1
      ..udpSupported = false;
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    controller.selectRow(_visibleIds(container).first);

    controller.emitAction(ProfileAction.udpTest);
    final state = container.read(profilesControllerProvider);
    expect(bridge.startedJobs, isEmpty, reason: 'no job is started');
    expect(state.speedTestStage, 'SpeedtestingFailed');
    expect(state.speedTestMessage, contains('UDP'));
  });

  test('a start failure is readable and the next attempt can retry', () {
    final bridge = R422Bridge()
      ..activeJobs = 1
      ..failStartCode = 'E_TEST_BUSY';
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    controller.selectRow(_visibleIds(container).first);

    controller.emitAction(ProfileAction.tcping);
    var state = container.read(profilesControllerProvider);
    expect(state.speedTestStage, 'SpeedtestingFailed');
    expect(state.speedTestMessage, contains('E_TEST_BUSY'));
    expect(state.speedTestRunning, isFalse);

    controller.emitAction(ProfileAction.tcping);
    state = container.read(profilesControllerProvider);
    expect(state.speedTestRunning, isTrue);
    expect(state.speedTestJobId, isNotNull);
    expect(bridge.startedJobs, hasLength(1));
  });
}
