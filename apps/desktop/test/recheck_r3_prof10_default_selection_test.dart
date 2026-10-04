// R3-PROF-10: default selection on a group switch / new save
// (pending -> active -> first), Esc keeps the selection, and Shift/arrow
// range operations derive from the main row (`primaryId`).
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';

import 'recheck01_group_inheritance_test.dart'
    show StoredBridge, makeStoredContainer;

void main() {
  test('group switch selects the active node', () {
    final bridge = StoredBridge(count: 3);
    final ids = bridge.queryAllProfiles().map((p) => p.indexId).toList();
    bridge.setActiveProfile(ids[1]);
    final container = makeStoredContainer(bridge: bridge);
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);

    controller.setGroupSubId('sub-000');
    final state = container.read(profilesControllerProvider);
    expect(state.selected, <String>{ids[1]});
    expect(state.primaryId, ids[1]);
  });

  test('group switch with no active node selects the first row', () {
    final container = makeStoredContainer(bridge: StoredBridge(count: 3));
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);

    controller.setGroupSubId('sub-000');
    final state = container.read(profilesControllerProvider);
    expect(state.visible, isNotEmpty);
    expect(state.selected, <String>{state.visible.first.id});
    expect(state.primaryId, state.visible.first.id);
  });

  test('saving a new node selects it', () {
    final container = makeStoredContainer(bridge: StoredBridge(count: 0));
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);

    final draft = controller.newDraft(ConfigType.vless)
      ..remarks = 'new-node'
      ..address = '192.0.2.1';
    final result = controller.saveDraft(draft.toDto());
    expect(result.ok, isTrue);
    final id = result.profile!.indexId;

    final state = container.read(profilesControllerProvider);
    expect(state.visible.map((r) => r.id), contains(id));
    expect(state.selected, <String>{id});
    expect(state.primaryId, id);
  });

  test('Esc stops the test but keeps the selection', () {
    final container = makeStoredContainer(bridge: StoredBridge(count: 3));
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);
    final id = container.read(profilesControllerProvider).visible.first.id;

    controller.selectRow(id);
    controller.emitAction(ProfileAction.escape);
    final state = container.read(profilesControllerProvider);
    expect(state.selected, <String>{id});
    expect(state.speedTestRunning, isFalse);
    expect(state.lastEvent?.action, ProfileAction.escape);
  });

  test('Shift range extends from the main row, not the set tail', () {
    final container = makeStoredContainer(bridge: StoredBridge(count: 6));
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);
    final ids = container
        .read(profilesControllerProvider)
        .visible
        .map((r) => r.id)
        .toList();

    controller.selectRow(ids[0]);
    controller.selectRow(ids[3], shift: true);
    var state = container.read(profilesControllerProvider);
    expect(state.selected, <String>{ids[0], ids[1], ids[2], ids[3]});
    expect(state.primaryId, ids[0], reason: 'the anchor stays current');

    // A second shift extends from the same main row, not the previous tail.
    controller.selectRow(ids[1], shift: true);
    state = container.read(profilesControllerProvider);
    expect(state.selected, <String>{ids[0], ids[1]});
    expect(state.primaryId, ids[0]);
  });

  test('arrow keys move from the main row, not the first selected', () {
    final container = makeStoredContainer(bridge: StoredBridge(count: 6));
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);
    final ids = container
        .read(profilesControllerProvider)
        .visible
        .map((r) => r.id)
        .toList();

    controller.selectRow(ids[1]);
    controller.selectRow(ids[2], ctrl: true);
    expect(container.read(profilesControllerProvider).primaryId, ids[2]);

    controller.emitAction(ProfileAction.navigateDown);
    final state = container.read(profilesControllerProvider);
    expect(state.selected, <String>{ids[3]});
    expect(state.primaryId, ids[3]);
  });
}
