// FIX-10B: node-table ordering writes the upstream `ProfileExItem.Sort` field.
//
// These widget-less controller tests drive the synthetic bridge (no native
// library) and assert that every ordering path — drag reorder, column-header
// sort, "按测试结果排序" and the move up/down/top/bottom actions — persists the
// current visible id order through `applyProfileOrder` (`(i + 1) * 10` in Rust).
// Empty and single-row lists are never written. The SQLite reopen read-back is
// proven by the Rust `fix10_speedtest_result`/`speedtest` tests.
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
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

void main() {
  test('column-header sort persists the new visible order', () {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    bridge.appliedProfileOrders.clear();

    controller.sortBy('Remarks');

    final expected = _visibleIds(container);
    expect(expected.length, greaterThan(1));
    expect(bridge.appliedProfileOrders, isNotEmpty);
    expect(bridge.appliedProfileOrders.last, expected);
  });

  test('sort by test result persists the delay order', () {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    final visible = container.read(profilesControllerProvider).visible;
    // Give the first two rows a measurable delay so the sort is observable.
    bridge.seedSpeedResult(visible[0].id, 200, 1.0);
    bridge.seedSpeedResult(visible[1].id, 10, 1.0);
    controller.reload();
    bridge.appliedProfileOrders.clear();

    controller.sortByResult();

    final ordered = _visibleIds(container);
    expect(bridge.appliedProfileOrders.last, ordered);
    expect(ordered.first, visible[1].id, reason: 'lowest delay sorts first');
  });

  test('drag reorder moves the row and persists the order', () {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    // Activate a column sort first so the drag must clear it to stick.
    controller.sortBy('Remarks');
    final before = _visibleIds(container);
    bridge.appliedProfileOrders.clear();

    controller.handleDrop(before[2], before[0]);

    final after = _visibleIds(container);
    expect(after.first, before[2], reason: 'dragged row moved to the top');
    expect(bridge.appliedProfileOrders.last, after);
    expect(
      container.read(profilesControllerProvider).sort.direction,
      SortDirection.none,
      reason: 'manual drag clears the active column sort',
    );

    // Dragging a row onto itself is a no-op that writes nothing.
    final writes = bridge.appliedProfileOrders.length;
    controller.handleDrop(after.first, after.first);
    expect(bridge.appliedProfileOrders.length, writes);
  });

  test('move up/down/top/bottom persist the visible order', () {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    final before = _visibleIds(container);

    controller.selectRow(before[2]);
    bridge.appliedProfileOrders.clear();
    controller.emitAction(ProfileAction.moveTop);
    expect(bridge.appliedProfileOrders.last, _visibleIds(container));
    expect(_visibleIds(container).first, before[2]);

    controller.emitAction(ProfileAction.moveDown);
    expect(bridge.appliedProfileOrders.last, _visibleIds(container));

    controller.emitAction(ProfileAction.moveBottom);
    expect(bridge.appliedProfileOrders.last, _visibleIds(container));
    expect(_visibleIds(container).last, before[2]);
  });

  test('whole-group sort writes the group order even when filtered', () {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);

    final all = container.read(profilesControllerProvider).visible;
    final firstRow = all.first;
    // Filter down to exactly one visible id (matches by remarks).
    controller.setFilter(firstRow.remarks);
    controller.submitFilter();
    expect(container.read(profilesControllerProvider).visible.length, 1);
    bridge.appliedProfileOrders.clear();
    controller.sortBy('Remarks');
    // R3-PROF-03: the sort persists the whole group (hidden rows included),
    // not just the filtered visible set.
    expect(bridge.appliedProfileOrders, isNotEmpty);
    expect(
      bridge.appliedProfileOrders.last.length,
      all.length,
      reason: 'whole group order includes rows hidden by the filter',
    );

    // Empty visible set: sorting by result never throws and, like the header
    // sort (`SortServers` over `ProfileModels(subId, "")`), still orders the
    // whole group.
    bridge.appliedProfileOrders.clear();
    controller.setFilter('___no_such_node___');
    controller.submitFilter();
    expect(container.read(profilesControllerProvider).visible, isEmpty);
    controller.sortByResult();
    expect(bridge.appliedProfileOrders.single.length, all.length);
  });
}
