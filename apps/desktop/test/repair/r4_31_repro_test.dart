// R4-31 repro: the 150 ms speedtest/statistics poll must not churn the node
// table read model.
//
// These assertions fail on the pre-fix controller:
//   * `_refreshLive` unconditionally re-ran `_recompute` (full filter + sort)
//     and assigned a brand-new `visible` list on every poll tick, even when the
//     live overlay produced identical values. Under a 10k list with logs and
//     monitoring refreshing concurrently this is continuous UI-isolate churn.
//   * When the cached base was empty the poll called the full synchronous
//     `reload()` (whole-table read) every 150 ms instead of leaving the
//     structural read to event-driven refreshes.
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
  test(
    'an idle poll tick must not rebuild the node table read model',
    () async {
      final bridge = R422Bridge()..activeJobs = 1;
      final container = _container(bridge);
      final controller = container.read(profilesControllerProvider.notifier);
      controller.selectRow(
        container.read(profilesControllerProvider).visible.first.id,
      );
      controller.emitAction(ProfileAction.realping);

      await Future<void>.delayed(const Duration(milliseconds: 250));
      final first = container.read(profilesControllerProvider).visible;
      await Future<void>.delayed(const Duration(milliseconds: 220));
      final second = container.read(profilesControllerProvider).visible;

      expect(
        identical(first, second),
        isTrue,
        reason: 'an idle poll tick must not rebuild the read model',
      );
    },
  );

  test(
    'the poll must not run a structural full read on every empty-base tick',
    () async {
      final bridge = R422Bridge()..activeJobs = 1;
      final container = _container(bridge);
      final controller = container.read(profilesControllerProvider.notifier);
      controller.selectRow(
        container.read(profilesControllerProvider).visible.first.id,
      );
      controller.emitAction(ProfileAction.realping);

      // The store reports no rows while the run is still active: the live poll
      // must leave the structural read to event-driven refresh, not re-read the
      // whole table every 150 ms.
      bridge.returnEmptySnapshots = true;
      controller.reload();
      final baseline = bridge.snapshotCalls;
      await Future<void>.delayed(const Duration(milliseconds: 350));

      expect(
        bridge.snapshotCalls,
        baseline,
        reason: 'an empty base must not trigger a full read every poll tick',
      );
    },
  );
}
