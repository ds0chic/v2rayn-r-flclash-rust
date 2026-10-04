// R3-PROF-01..04 controller/pure-state regressions.
//
// These widget-less controller tests drive the synthetic bridge (no native
// library): independent main row / command target, hidden-object refusal,
// stable sort with failed-value sinking and whole-group order writes, and the
// settings-driven dedup outcome. Real SQLite read-back is covered by the Rust
// speedtest/persistence suites.
import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profile_actions.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';

ProviderContainer _container(SyntheticBridgePort bridge) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(20),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
      monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

c.ProfileDto _vless(String id, String address, {String subid = ''}) =>
    c.ProfileDto(
      indexId: id,
      configType: ConfigType.vless,
      coreType: CoreType.xray,
      configVersion: 4,
      subid: subid,
      isSub: true,
      displayLog: true,
      remarks: id,
      address: address,
      port: 443,
      password: '',
      username: '',
      network: 'raw',
      security: const c.SecurityDto(),
      protoExtra: const c.ProtocolExtraDto(extraJson: '{}'),
      transportExtra: const c.TransportExtraDto(extraJson: '{}'),
      extraJson: '{}',
    );

ProfileSummary _summary(String id, {int delay = -1, String speed = '-'}) =>
    ProfileSummary(
      id: id,
      configType: ConfigType.vmess,
      remarks: id,
      address: '192.0.2.1',
      port: 443,
      network: 'raw',
      streamSecurity: '',
      subRemarks: '',
      delay: delay,
      speed: speed,
      todayUp: BigInt.zero,
      ipInfo: '-',
      todayDown: BigInt.zero,
      totalUp: BigInt.zero,
      totalDown: BigInt.zero,
      coreType: CoreType.xray,
    );

List<String> _visible(ProviderContainer c) =>
    c.read(profilesControllerProvider).visible.map((r) => r.id).toList();

void main() {
  test('R3-PROF-01: multi-selection keeps an independent main row', () {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    final ids = _visible(container);

    controller.selectRow(ids[0]);
    controller.selectRow(ids[1], ctrl: true);
    var state = container.read(profilesControllerProvider);
    expect(state.selected, <String>{ids[0], ids[1]});
    expect(state.primaryId, ids[1], reason: 'last clicked/current row');

    // The captured context restores the batch selection without collapsing it.
    expect(controller.restoreContextTargets(<String>[ids[0], ids[1]]), isTrue);
    state = container.read(profilesControllerProvider);
    expect(state.selected, <String>{ids[0], ids[1]});
    expect(state.primaryId, ids[1]);

    // A single-object command resolves the captured target first, then the
    // main row, never requiring selected.length == 1.
    expect(resolveSingleTarget(state, ids[0]), ids[0]);
    expect(resolveSingleTarget(state, null), ids[1]);
  });

  test('R3-PROF-02: a filter-hidden captured target is refused', () {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    final ids = _visible(container);

    controller.selectRow(ids[0]);
    expect(container.read(profilesControllerProvider).primaryId, ids[0]);

    // Hide ids[0] behind a text filter that still shows another row.
    controller.setFilter('Synthetic-00001');
    final state = container.read(profilesControllerProvider);
    expect(state.visible.map((r) => r.id), isNot(contains(ids[0])));
    expect(controller.isVisibleTarget(ids[0]), isFalse);
    expect(state.primaryId, isNull, reason: 'hidden main row is cleared');
    expect(controller.restoreContextTargets(<String>[ids[0]]), isFalse);
  });

  test('R3-PROF-03: Delay/Speed sort sinks <=0 in both directions', () {
    final rows = <ProfileSummary>[
      _summary('a', delay: -1),
      _summary('b', delay: 10),
      _summary('c', delay: 100),
    ];
    final columns = defaultProfileColumns();
    String ids(List<ProfileSummary> r) => r.map((e) => e.id).join(',');

    expect(
      ids(
        applySort(
          rows,
          columns,
          const SortSpec(
            columnKey: 'DelayVal',
            direction: SortDirection.ascending,
          ),
        ),
      ),
      'b,c,a',
    );
    expect(
      ids(
        applySort(
          rows,
          columns,
          const SortSpec(
            columnKey: 'DelayVal',
            direction: SortDirection.descending,
          ),
        ),
      ),
      'c,b,a',
    );

    final speed = <ProfileSummary>[
      _summary('a', speed: '-'),
      _summary('b', speed: '10 MB/s'),
      _summary('c', speed: '100 MB/s'),
    ];
    expect(
      ids(
        applySort(
          speed,
          columns,
          const SortSpec(
            columnKey: 'SpeedVal',
            direction: SortDirection.ascending,
          ),
        ),
      ),
      'b,c,a',
    );
    expect(
      ids(
        applySort(
          speed,
          columns,
          const SortSpec(
            columnKey: 'SpeedVal',
            direction: SortDirection.descending,
          ),
        ),
      ),
      'c,b,a',
    );
  });

  test('R3-PROF-03: sortByResult syncs state.sort and survives reload', () {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    final ids = _visible(container);
    final failed = ids[0];
    final fast = ids[1];
    final slow = ids[2];
    bridge.seedSpeedResult(failed, -1, 0);
    bridge.seedSpeedResult(fast, 10, 1);
    bridge.seedSpeedResult(slow, 100, 1);
    controller.reload();

    // A prior column sort must not resurface after "sort by test result".
    controller.sortBy('Remarks');
    controller.sortByResult();
    final after = _visible(container);
    expect(after.indexOf(fast), lessThan(after.indexOf(slow)));
    expect(after.indexOf(failed), greaterThan(after.indexOf(slow)));
    expect(
      container.read(profilesControllerProvider).sort.columnKey,
      'DelayVal',
      reason: 'result sort is synced into state.sort',
    );

    controller.reload();
    expect(_visible(container), after, reason: 'reload keeps result order');
  });

  test(
    'R3-PROF-03: header sort writes the whole group including hidden rows',
    () {
      final bridge = SyntheticBridgePort();
      final container = _container(bridge);
      final controller = container.read(profilesControllerProvider.notifier);
      final ids = _visible(container);
      controller.setFilter('Synthetic-00003'); // hides 19 rows

      controller.sortBy('Remarks');
      final written = bridge.appliedProfileOrders.last;
      expect(written.length, ids.length, reason: 'whole group written');
      expect(written.contains(ids[0]), isTrue, reason: 'hidden row updated');
    },
  );

  test('R3-PROF-04: dedup reads KeepOlderDedupl=false (newer wins)', () {
    final bridge = SyntheticBridgePort(count: 0);
    expect(
      bridge
          .saveImportedProfile(
            _vless('old', '192.0.2.50'),
            bridge.profileRevision(),
          )
          .ok,
      isTrue,
    );
    expect(
      bridge
          .saveImportedProfile(
            _vless('new', '192.0.2.50'),
            bridge.profileRevision(),
          )
          .ok,
      isTrue,
    );
    // Persist the frozen `GuiItem.KeepOlderDedupl = false` (the default too).
    bridge.saveSettingsJson(
      jsonEncode(<String, Object>{
        'GuiItem': <String, Object>{'KeepOlderDedupl': false},
      }),
      bridge.getSettings().revision.toInt(),
    );
    expect(readKeepOlderDedupl(bridge), isFalse);

    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    controller.reload();

    final outcome = controller.removeDuplicateProfilesDetailed();
    expect(outcome.ok, isTrue);
    expect(outcome.hadDuplicates, isTrue);
    expect(outcome.removed, 1);
    final ids = bridge.queryAllProfiles().map((p) => p.indexId).toSet();
    expect(ids.contains('old'), isFalse, reason: 'older entry removed');
    expect(ids.contains('new'), isTrue);
  });

  test('R3-PROF-04: delete failure is reported, not "no duplicates"', () {
    final bridge = SyntheticBridgePort(count: 0);
    bridge.saveImportedProfile(_vless('old', '192.0.2.51'), 0);
    bridge.saveImportedProfile(_vless('new', '192.0.2.51'), 1);
    bridge.failDeleteProfiles = true;

    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    controller.reload();

    final outcome = controller.removeDuplicateProfilesDetailed(keepOlder: true);
    expect(outcome.ok, isFalse);
    expect(outcome.hadDuplicates, isTrue);
    expect(outcome.errorCode, 'E_DELETE_FAILED');
  });

  test('R3-PROF-04: dedup flags a deleted active node', () {
    final bridge = SyntheticBridgePort(count: 0);
    bridge.saveImportedProfile(_vless('old', '192.0.2.52'), 0);
    bridge.saveImportedProfile(_vless('new', '192.0.2.52'), 1);
    bridge.setActiveProfile('old');
    bridge.saveSettingsJson(
      jsonEncode(<String, Object>{
        'GuiItem': <String, Object>{'KeepOlderDedupl': false},
      }),
      bridge.getSettings().revision.toInt(),
    );

    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    controller.reload();
    expect(container.read(profilesControllerProvider).activeId, 'old');

    final outcome = controller.removeDuplicateProfilesDetailed();
    expect(outcome.activeRemoved, isTrue);
    expect(outcome.removed, 1);
  });
}
