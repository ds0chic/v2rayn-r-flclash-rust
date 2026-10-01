// T15b: speedtest UI wiring. These tests drive the controller through the
// synthetic bridge (no native library) and assert that:
//   * the menu/shortcut actions reach the bridge with the right action kind;
//   * Mixed/Fast use all nodes while the per-selection tests use the selection;
//   * Esc cancels a running job (ACT-PROF-038);
//   * the ProfileEx result overlay updates Delay/Speed without reordering;
//   * `RemoveInvalidServerResult` is invoked by the menu action.
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';

ProviderContainer _container(SyntheticBridgePort bridge) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(50),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
      monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  test('selection tests use the selection and Mixed uses all nodes', () {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);

    controller.selectRow('syn-000003');
    controller.emitAction(ProfileAction.tcping);
    expect(bridge.speedTestCalls.last['kind'], 0);
    expect(bridge.speedTestCalls.last['ids'], <String>['syn-000003']);

    controller.emitAction(ProfileAction.realping);
    expect(bridge.speedTestCalls.last['kind'], 1);

    controller.emitAction(ProfileAction.speedtest);
    expect(bridge.speedTestCalls.last['kind'], 3);

    controller.emitAction(ProfileAction.mixedTest);
    expect(bridge.speedTestCalls.last['kind'], 4);
    expect(bridge.speedTestCalls.last['ids'], <String>[]);

    controller.emitAction(ProfileAction.fastRealping);
    expect(bridge.speedTestCalls.last['kind'], 5);
    expect(bridge.speedTestCalls.last['ids'], <String>[]);

    // The job is tracked so Esc can cancel it.
    final state = container.read(profilesControllerProvider);
    expect(state.speedTestRunning, isTrue);
    expect(state.speedTestJobId, isNotNull);
    expect(state.speedTestStage, 'Speedtesting');
  });

  test('Escape cancels the running test (ACT-PROF-038)', () {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);

    controller.selectRow('syn-000001');
    controller.emitAction(ProfileAction.tcping);
    final jobId = container.read(profilesControllerProvider).speedTestJobId!;

    controller.emitAction(ProfileAction.escape);
    expect(bridge.cancelledTestJobs, contains(jobId));
    final state = container.read(profilesControllerProvider);
    expect(state.speedTestRunning, isFalse);
    expect(state.speedTestStage, 'SpeedtestingStop');
    // Esc is still the last recorded keyboard action.
    expect(state.lastEvent?.action, ProfileAction.escape);
  });

  test('ProfileEx overlay updates Delay/Speed and preserves order', () {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);

    final before = container
        .read(profilesControllerProvider)
        .visible
        .map((r) => r.id)
        .toList();
    bridge.seedSpeedResult('syn-000010', 88, 12.5, ipInfo: '203.0.113.9');
    controller.reload();

    final state = container.read(profilesControllerProvider);
    final after = state.visible.map((r) => r.id).toList();
    expect(after, before, reason: 'reload must not reorder rows');
    final row = state.visible.firstWhere((r) => r.id == 'syn-000010');
    expect(row.delay, 88);
    expect(row.speed, contains('12.5'));
    expect(row.ipInfo, '203.0.113.9');
  });

  test('RemoveInvalidServerResult reaches the bridge', () {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);

    bridge.seedSpeedResult('syn-000000', -1, 0);
    bridge.seedSpeedResult('syn-000001', 20, 1.0);
    controller.emitAction(ProfileAction.removeInvalid);
    expect(bridge.removeInvalidCalls, 1);
    final rows = bridge.speedTestResults();
    expect(rows.length, 1);
    expect(rows.single.indexId, 'syn-000001');
  });
}
