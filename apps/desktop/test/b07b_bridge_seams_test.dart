// B07b: bridge error surfacing (S8-03), subscription remarks join (D1-01),
// untested-first snapshot order (D1-02), and the new S8-08 / cancel /
// rollback seam entries. Pure unit tests through SyntheticBridgePort.
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';

c.SubItemDto _sub(String id, String remarks) => c.SubItemDto(
  id: id,
  remarks: remarks,
  url: 'https://example.com/$id',
  moreUrl: '',
  enabled: true,
  userAgent: '',
  sort: 1,
  autoUpdateInterval: 0,
  updateTime: 0,
);

/// Bridge whose profile reads fail the way a corrupt store would surface
/// through the Rust `page.error` arm (S8-03). Fails at the structural-read
/// seam, like the Frb port's `queryAllProfiles` throw inside
/// `fetchProfileSnapshot`.
class _FailingBridge extends SyntheticBridgePort {
  _FailingBridge() : super(count: 5);

  @override
  ProfileSnapshot fetchProfileSnapshot(
    int count, {
    String? text,
    String? subid,
  }) {
    throw const BridgeDataError(code: 'E_IO', messageKey: 'error.io');
  }

  @override
  Future<c.ProfilePageDto> queryProfilesPageAsync({
    required c.ProfileFilterDto filter,
    required c.ProfileSortDto sort,
    required int cursor,
    required int pageSize,
    required int requestGeneration,
  }) async {
    throw const BridgeDataError(code: 'E_IO', messageKey: 'error.io');
  }
}

ProviderContainer _containerWith(BridgePort bridge) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
    profileRowCountProvider.overrideWithValue(5),
    platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
    monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
  ],
);

void main() {
  test('fetchProfileSnapshot joins subscription remarks (D1-01)', () {
    final bridge = SyntheticBridgePort(count: 5);
    expect(bridge.saveSubItem(_sub('sub-000', '我的订阅')).ok, isTrue);
    final snapshot = bridge.fetchProfileSnapshot(5);
    expect(snapshot.summaries, isNotEmpty);
    // True path: dtoToSummary emits the raw subid, the snapshot join
    // replaces it with the subscription remark.
    expect(snapshot.summaries.first.subRemarks, '我的订阅');
  });

  test('fetchProfileSnapshot keeps untested rows first (D1-02)', () {
    final bridge = SyntheticBridgePort(count: 5);
    final ids = bridge
        .fetchProfileSnapshot(5)
        .profiles
        .map((p) => p.indexId)
        .toList();
    expect(ids, hasLength(5));
    bridge.seedSpeedResult(ids[0], 10, 1.0);
    bridge.seedSpeedResult(ids[1], 20, 2.0);
    final order = bridge
        .fetchProfileSnapshot(5)
        .summaries
        .map((s) => s.id)
        .toList();
    // Untested rows keep store order ahead of the result-ordered rows
    // (upstream `Sort = t33?.Sort ?? 0` semantics).
    expect(order, [...ids.sublist(2), ids[0], ids[1]]);
  });

  test('controller build survives a storage failure (S8-03)', () {
    final container = _containerWith(_FailingBridge());
    addTearDown(container.dispose);
    final state = container.read(profilesControllerProvider);
    expect(state.all, isEmpty);
    expect(state.profiles, isEmpty);
    expect(
      state.events.map((e) => e.action),
      contains('build-snapshot-failed'),
    );
  });

  test('controller reload keeps the table on a storage failure (S8-03)', () {
    final container = _containerWith(_FailingBridge());
    addTearDown(container.dispose);
    container.read(profilesControllerProvider);
    final controller = container.read(profilesControllerProvider.notifier);
    controller.reload();
    expect(container.read(profilesControllerProvider).all, isEmpty);
  });

  test(
    'selectAllAcrossPages keeps selection on a page error (S8-03)',
    () async {
      final container = _containerWith(_FailingBridge());
      addTearDown(container.dispose);
      container.read(profilesControllerProvider);
      final controller = container.read(profilesControllerProvider.notifier);
      final selected = await controller.selectAllAcrossPages(pageSize: 2);
      expect(selected, isEmpty);
    },
  );

  test('new seam stubs are wired (S8-08/cancel/rollback)', () {
    final bridge = SyntheticBridgePort(count: 5);
    expect(bridge.subUpdateReport('missing'), isNull);
    expect(bridge.t16CancelUpdate().ok, isTrue);
    expect(bridge.t16RollbackCore('xray').ok, isTrue);
    expect(
      bridge.t16Calls,
      containsAll(['cancel_update', 'rollback_core:xray']),
    );
  });
}
