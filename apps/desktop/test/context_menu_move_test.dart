// UX-SPACE-02: `移至订阅分组` executes through the real profile-save path.
//
// The context menu has no dedicated move-to-group bridge API; the action
// rewrites each stored `ProfileDto.subid` and saves it via the optimistic
// `saveProfile` seam. This test drives `ProfilesController.moveProfilesToGroup`
// against the synthetic bridge and asserts the stored subid changed (real
// persistence path, no fake success) and the event log records the command.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

import 'support/profiles_harness.dart';

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

void main() {
  test('move-to-group rewrites subid through the real save path', () {
    final container = makeContainer(rows: 20);
    addTearDown(container.dispose);
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    bridge.saveSubItem(_sub('sub-target', '目标分组'));
    final controller = container.read(profilesControllerProvider.notifier);
    controller.reload();

    final ids = container
        .read(profilesControllerProvider)
        .all
        .take(3)
        .map((r) => r.id)
        .toList();
    expect(ids.length, 3);

    controller.resetEvents();
    final ok = controller.moveProfilesToGroup(ids, 'sub-target');
    expect(ok, isTrue);

    // Persisted subid changed for every target, and no rows were added/removed.
    for (final id in ids) {
      expect(bridge.getProfile(id)?.subid, 'sub-target');
    }
    expect(container.read(profilesControllerProvider).totalCount, 20);
    expect(
      container
          .read(profilesControllerProvider)
          .events
          .where((e) => e.action == 'move-to-group')
          .isNotEmpty,
      isTrue,
    );
  });

  test('move-to-group to no-group clears subid', () {
    final container = makeContainer(rows: 20);
    addTearDown(container.dispose);
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    final controller = container.read(profilesControllerProvider.notifier);
    controller.reload();

    final id = container.read(profilesControllerProvider).all.first.id;
    // Give it a group first.
    expect(controller.moveProfilesToGroup([id], 'sub-x'), isTrue);
    expect(bridge.getProfile(id)?.subid, 'sub-x');

    // Move to "无分组" (empty subid).
    expect(controller.moveProfilesToGroup([id], ''), isTrue);
    expect(bridge.getProfile(id)?.subid, '');
  });

  test('move-to-group reports failure when a target vanished', () {
    final container = makeContainer(rows: 20);
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);
    controller.reload();
    final ok = controller.moveProfilesToGroup(<String>['does-not-exist'], 's');
    expect(ok, isFalse);
  });
}
