// R4-22 repro: cancel/generation binding for the speedtest range.
//
// These assertions fail on the pre-fix controller:
//   * `cancelSpeedTest` stopped the poller immediately and bound the stop to a
//     single (latest) job id, so a result measured just before the runner
//     actually exited was never read, and an older in-flight job leaked with no
//     way to stop it.
//
// Synthetic only: no native library, no network, no port, no user data.
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import '../support/fake_monitor_bridge.dart';
import '../support/fake_platform_bridge.dart';
import '../support/r4_22_bridge.dart';

ProviderContainer _container(R422Bridge bridge) {
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

void main() {
  test('cancel keeps the real job binding and a result measured before the '
      'runner exits is not lost', () async {
    final bridge = R422Bridge()..activeJobs = 1;
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    final state = container.read(profilesControllerProvider);
    final target = state.visible[1].id;

    controller.selectRow(target);
    controller.emitAction(ProfileAction.realping);
    final jobId = container.read(profilesControllerProvider).speedTestJobId!;
    expect(bridge.startedJobs, contains(jobId));

    controller.cancelSpeedTest();
    expect(bridge.cancelledRuns, contains(jobId));
    expect(
      container.read(profilesControllerProvider).speedTestStage,
      'SpeedtestingStop',
    );

    // The runner measured the node just before it actually stopped; a stop
    // must still surface that result instead of dropping it.
    bridge.setResult(target, 27);
    bridge.activeJobs = 0;
    await Future<void>.delayed(const Duration(milliseconds: 350));

    final after = container.read(profilesControllerProvider);
    final row = after.all.firstWhere((r) => r.id == target);
    expect(
      row.delay,
      27,
      reason: 'a result already obtained before stop must not be lost',
    );
    expect(after.speedTestStage, 'SpeedtestingStop');
    expect(after.speedTestMessage, '已停止测速');
  });

  test('stop cancels every in-flight run, not just the latest job id', () {
    final bridge = R422Bridge()..activeJobs = 1;
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    controller.selectRow(
      container.read(profilesControllerProvider).visible.first.id,
    );

    controller.emitAction(ProfileAction.tcping);
    final first = container.read(profilesControllerProvider).speedTestJobId!;
    controller.emitAction(ProfileAction.realping);
    final second = container.read(profilesControllerProvider).speedTestJobId!;
    expect(second, isNot(first));

    controller.cancelSpeedTest();
    expect(
      bridge.cancelledRuns,
      containsAll(<String>[first, second]),
      reason: 'every tracked job must be cancelled (upstream ExitLoop)',
    );
  });
}
