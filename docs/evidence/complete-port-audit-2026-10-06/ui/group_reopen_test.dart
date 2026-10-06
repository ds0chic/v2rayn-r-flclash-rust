import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

void main() {
  test('current group survives a controller reopen through canonical settings', () {
    final bridge = SyntheticBridgePort(count: 3);
    final saved = bridge.saveSubItem(const c.SubItemDto(id: '', remarks: 'Synthetic group', url: 'https://example.invalid/synthetic-group', moreUrl: '', enabled: true, userAgent: '', sort: 0, autoUpdateInterval: 0, updateTime: 0));
    expect(saved.ok, isTrue);
    final id = bridge.listSubItems().items.single.id;
    final container = ProviderContainer(overrides: [
      bridgePortProvider.overrideWithValue(bridge),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(3),
    ]);
    addTearDown(container.dispose);
    container.read(settingsControllerProvider.notifier).load();
    container.read(profilesControllerProvider.notifier).setGroupSubId(id);
    final settings = jsonDecode(bridge.getSettings().settingsJson) as Map<String, dynamic>;
    expect(settings['SubIndexId'], id, reason: 'current node group must enter the canonical config that close/restore will save');
    container.invalidate(profilesControllerProvider);
    expect(container.read(profilesControllerProvider).groupSubId, id, reason: 'reopen must restore the current group');
  });
}
