// T06a: controller persistence flow through the bridge interface (synthetic
// backend, same contract as the FRB port).
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

ProviderContainer makeContainer(SyntheticBridgePort bridge) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(200),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  test('save, copy, delete and revision conflict via the bridge', () {
    final bridge = SyntheticBridgePort(count: 200);
    final container = makeContainer(bridge);
    final controller = container.read(profilesControllerProvider.notifier);

    // Save a new VLESS node.
    final draft = controller.newDraft(ConfigType.vless)
      ..remarks = 'controller-vless'
      ..address = '192.0.2.44'
      ..port = 443
      ..network = 'ws';
    final result = controller.saveDraft(draft.toDto());
    expect(result.ok, isTrue, reason: result.error?.code);
    final savedId = result.profile!.indexId;
    expect(savedId, isNotEmpty);

    final state = container.read(profilesControllerProvider);
    expect(state.profiles.any((p) => p.indexId == savedId), isTrue);

    // Stale revision is a structured conflict.
    final stale = bridge.saveProfile(draft.toDto(), 0);
    expect(stale.ok, isFalse);
    expect(stale.error!.code, 'E_REVISION_STALE');

    // Copy the selected node.
    controller.selectRow(savedId);
    final copy = controller.copySelected();
    expect(copy.ok, isTrue);
    expect(copy.copies.single.remarks.contains('副本'), isTrue);

    // Delete the selection.
    final copyId = copy.copies.single.indexId;
    controller.selectRow(copyId);
    final deleted = controller.deleteSelected();
    expect(deleted.ok, isTrue);
    expect(deleted.removed, BigInt.one);

    // Active node round-trips.
    controller.selectRow(savedId);
    controller.setActive(savedId);
    expect(container.read(profilesControllerProvider).activeId, savedId);
  });

  test('remarks and empty-remarks validation', () {
    final bridge = SyntheticBridgePort(count: 10);
    final container = makeContainer(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    final draft = controller.newDraft(ConfigType.trojan)
      ..remarks = 'trojan-1'
      ..address = '192.0.2.45'
      ..port = 443;
    final saved = controller.saveDraft(draft.toDto());
    expect(saved.ok, isTrue);

    final renamed = controller.renameProfile(saved.profile!.indexId, '新备注 🚀');
    expect(renamed.ok, isTrue);
    expect(renamed.profile!.remarks, '新备注 🚀');

    final empty = controller.renameProfile(saved.profile!.indexId, '   ');
    expect(empty.ok, isFalse);
    expect(empty.error!.code, 'E_FIELD_REQUIRED');
  });
}
