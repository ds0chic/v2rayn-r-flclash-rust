// RE-PROF-06: dedup / remove-invalid scope is the whole current group (`subid`),
// not the live text filter, and result cleanup must not erase another group's
// failure evidence or the current group's evidence when a delete fails.
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';

ProviderContainer _container(SyntheticBridgePort bridge) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(10),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
      monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

c.ProfileDto _node({
  required String id,
  required String subid,
  required String remarks,
  required String address,
}) => c.ProfileDto(
  indexId: id,
  configType: ConfigType.vless,
  coreType: CoreType.xray,
  configVersion: 4,
  subid: subid,
  isSub: true,
  displayLog: true,
  remarks: remarks,
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

void _add(SyntheticBridgePort bridge, c.ProfileDto dto) {
  final result = bridge.saveImportedProfile(dto, bridge.profileRevision());
  expect(result.ok, isTrue, reason: 'test setup save');
}

void main() {
  const groupA = 'SUB-R06-A';
  const groupB = 'SUB-R06-B';

  test('dedup scopes to the group even when the filter hides a duplicate', () {
    final bridge = SyntheticBridgePort(count: 4);
    _add(
      bridge,
      _node(
        id: 'r06-a1',
        subid: groupA,
        remarks: 'visible',
        address: '192.0.2.70',
      ),
    );
    _add(
      bridge,
      _node(
        id: 'r06-a2',
        subid: groupA,
        remarks: 'hidden-dup',
        address: '192.0.2.70',
      ),
    );
    _add(
      bridge,
      _node(
        id: 'r06-b1',
        subid: groupB,
        remarks: 'other',
        address: '192.0.2.71',
      ),
    );
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    controller.setGroupSubId(groupA);
    controller.setFilter('visible'); // hides r06-a2 from the table

    final removed = controller.removeDuplicateProfiles();
    expect(removed, 1, reason: 'the filter-hidden duplicate is still removed');
    final ids = bridge.queryAllProfiles().map((p) => p.indexId).toSet();
    expect(ids.contains('r06-a2'), isFalse);
    expect(ids.contains('r06-a1'), isTrue);
    expect(ids.contains('r06-b1'), isTrue, reason: 'other group untouched');
  });

  test('remove-invalid scopes to the group and preserves other groups', () {
    final bridge = SyntheticBridgePort(count: 4);
    _add(
      bridge,
      _node(
        id: 'r06-a1',
        subid: groupA,
        remarks: 'hidden',
        address: '192.0.2.72',
      ),
    );
    _add(
      bridge,
      _node(
        id: 'r06-a2',
        subid: groupA,
        remarks: 'visible',
        address: '192.0.2.73',
      ),
    );
    _add(
      bridge,
      _node(
        id: 'r06-b1',
        subid: groupB,
        remarks: 'other',
        address: '192.0.2.74',
      ),
    );
    bridge.seedSpeedResult('r06-a1', -1, 0);
    bridge.seedSpeedResult('r06-b1', -1, 0);
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    controller.setGroupSubId(groupA);
    controller.setFilter('visible'); // hides the failed r06-a1

    final removed = controller.removeInvalidResults();
    expect(
      removed,
      1,
      reason: 'the filter-hidden failed node is still deleted',
    );
    final ids = bridge.queryAllProfiles().map((p) => p.indexId).toSet();
    expect(ids.contains('r06-a1'), isFalse);
    expect(ids.contains('r06-a2'), isTrue);
    expect(ids.contains('r06-b1'), isTrue, reason: 'other group node kept');
    final results = bridge.speedTestResults().map((r) => r.indexId).toSet();
    expect(
      results.contains('r06-a1'),
      isFalse,
      reason: 'the deleted node result row is pruned',
    );
    expect(
      results.contains('r06-b1'),
      isTrue,
      reason: 'other group failure evidence is kept',
    );
    expect(bridge.removeInvalidCalls, 1);
  });

  test('remove-invalid keeps evidence when the delete fails', () {
    final bridge = SyntheticBridgePort(count: 4);
    _add(
      bridge,
      _node(
        id: 'r06-a1',
        subid: groupA,
        remarks: 'failed',
        address: '192.0.2.75',
      ),
    );
    bridge.seedSpeedResult('r06-a1', -1, 0);
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    controller.setGroupSubId(groupA);
    bridge.failDeleteProfiles = true;

    final removed = controller.removeInvalidResults();
    expect(removed, 0);
    expect(
      bridge.queryAllProfiles().any((p) => p.indexId == 'r06-a1'),
      isTrue,
      reason: 'a failed delete must not remove the profile',
    );
    expect(
      bridge.speedTestResults().any((r) => r.indexId == 'r06-a1'),
      isTrue,
      reason: 'a failed delete must keep the same group failure evidence',
    );
    expect(bridge.removeInvalidCalls, 0);
  });
}
