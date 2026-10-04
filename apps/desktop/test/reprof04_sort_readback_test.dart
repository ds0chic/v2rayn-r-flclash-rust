// RE-PROF-04: the node table reads its order back from the persisted
// `ProfileExItem.Sort`, toggles sort direction two-way, and surfaces a failed
// order write instead of silently dropping it.
//
// These widget-less controller tests drive the synthetic bridge (no native
// library). The real SQLite read-back and `Sort` output ordering are proven by
// the Rust `speedtest`/`fix10_speedtest_result`/`bridge_api::speedtest` tests.
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

ProviderContainer _container(SyntheticBridgePort bridge) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(20),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

List<String> _visibleIds(ProviderContainer container) => container
    .read(profilesControllerProvider)
    .visible
    .map((r) => r.id)
    .toList();

ProfileSummary _row(String id) => ProfileSummary(
  id: id,
  configType: ConfigType.vmess,
  remarks: id,
  address: '192.0.2.1',
  port: 443,
  network: 'raw',
  streamSecurity: '',
  subRemarks: '',
  delay: -1,
  speed: '-',
  todayUp: BigInt.zero,
  ipInfo: '-',
  todayDown: BigInt.zero,
  totalUp: BigInt.zero,
  totalDown: BigInt.zero,
  coreType: CoreType.xray,
);

void main() {
  test('orderByPersistedSort reorders rows and appends unknown ids', () {
    final rows = [_row('a'), _row('b'), _row('c')];
    final ordered = orderByPersistedSort(rows, ['c', 'a']);
    expect(ordered.map((r) => r.id).toList(), ['c', 'a', 'b']);
    expect(ordered.length, rows.length, reason: 'no row is dropped');
  });

  test('reload reads the persisted Sort order back after a drag', () {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    final before = _visibleIds(container);

    controller.handleDrop(before[3], before[0]);
    final dragged = _visibleIds(container);
    expect(dragged.first, before[3]);

    // A reload (also what the 150 ms speedtest poll does) must not fall back to
    // the query index-id order once the order was written.
    controller.reload();
    expect(_visibleIds(container), dragged);
  });

  test('column-header sort toggles ascending and descending, never none', () {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);

    controller.sortBy('Remarks');
    expect(
      container.read(profilesControllerProvider).sort.direction,
      SortDirection.ascending,
    );
    controller.sortBy('Remarks');
    expect(
      container.read(profilesControllerProvider).sort.direction,
      SortDirection.descending,
    );
    controller.sortBy('Remarks');
    expect(
      container.read(profilesControllerProvider).sort.direction,
      SortDirection.ascending,
      reason: 'same-column repeat toggles two-way, not back to none',
    );
  });

  test('sortByResult supports both directions with failures sinking', () {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    final ids = _visibleIds(container);
    final slow = ids[0];
    final fast = ids[1];
    final failed = ids[2];
    bridge.seedSpeedResult(slow, 100000, 1.0);
    bridge.seedSpeedResult(fast, 1, 1.0);
    bridge.seedSpeedResult(failed, -1, 0.0);
    controller.reload();

    controller.sortByResult(); // ascending
    final asc = _visibleIds(container);
    expect(asc.indexOf(fast), lessThan(asc.indexOf(slow)));
    expect(asc.indexOf(failed), greaterThan(asc.indexOf(slow)));

    controller.sortByResult(); // descending
    final desc = _visibleIds(container);
    expect(desc.indexOf(slow), lessThan(desc.indexOf(fast)));
    expect(
      desc.indexOf(failed),
      greaterThan(desc.indexOf(fast)),
      reason: 'failed/unknown always sink regardless of direction',
    );
    expect(bridge.appliedProfileOrders.last, desc);
  });

  test('a failed order write is surfaced and cleared by the next success', () {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    final before = _visibleIds(container);

    bridge.failApplyProfileOrder = true;
    controller.handleDrop(before[2], before[0]);
    final message = container.read(profilesControllerProvider).orderMessage;
    expect(message, isNotNull);
    expect(message, contains('E_ORDER_PERSIST'));

    bridge.failApplyProfileOrder = false;
    controller.sortBy('Remarks');
    expect(container.read(profilesControllerProvider).orderMessage, isNull);
  });
}
