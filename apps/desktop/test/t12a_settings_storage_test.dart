import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

ProviderContainer _container(BridgePort bridge) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
    profileRowCountProvider.overrideWithValue(4),
  ],
);

void main() {
  test('whole-tree save, revision conflict and reopen', () {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);

    final controller = container.read(settingsControllerProvider.notifier);
    final loaded = controller.load();
    expect(loaded.loaded, isTrue);
    expect(loaded.revision, 0);

    final draft = controller.draft();
    (draft['GuiItem'] as Map<String, dynamic>)['EnableStatistics'] = true;
    (draft['CoreBasicItem'] as Map<String, dynamic>)['Loglevel'] = 'debug';
    final result = controller.saveDocument(draft);
    expect(result.ok, isTrue);
    expect(container.read(settingsControllerProvider).revision, 1);
    expect(bridge.settingsRevision(), 1);

    // A save at the stale revision is rejected and changes nothing.
    final stale = bridge.saveSettingsJson(
      jsonEncode(<String, dynamic>{'GuiItem': <String, dynamic>{}}),
      0,
    );
    expect(stale.ok, isFalse);
    expect(stale.error!.code, 'E_REVISION_STALE');

    // Reopen against the same store: the saved values are restored.
    final reopened = _container(bridge);
    addTearDown(reopened.dispose);
    final loaded2 = reopened.read(settingsControllerProvider.notifier).load();
    expect(
      (loaded2.document['GuiItem'] as Map<String, dynamic>)['EnableStatistics'],
      isTrue,
    );
    expect(
      (loaded2.document['CoreBasicItem'] as Map<String, dynamic>)['Loglevel'],
      'debug',
    );
  });

  test('group save only replaces the target group', () {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    final controller = container.read(settingsControllerProvider.notifier);
    controller.load();

    final before = controller.draft();
    final ui = Map<String, dynamic>.of(before['UiItem'] as Map<String, dynamic>)
      ..['CurrentTheme'] = 'Dark';
    final result = controller.saveGroup('UiItem', ui);
    expect(result.ok, isTrue);

    final after = controller.draft();
    expect((after['UiItem'] as Map<String, dynamic>)['CurrentTheme'], 'Dark');
    expect(
      after['CoreBasicItem'],
      equals(before['CoreBasicItem']),
      reason: 'unrelated group must be untouched',
    );
  });

  test('unknown keys and null/empty distinctions round trip', () {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    final controller = container.read(settingsControllerProvider.notifier);
    controller.load();
    final draft = controller.draft();
    (draft['CoreBasicItem'] as Map<String, dynamic>)['FutureToggle'] = 42;
    draft['SubIndexId'] = '';
    (draft['CoreBasicItem'] as Map<String, dynamic>)['Loglevel'] = null;
    expect(controller.saveDocument(draft).ok, isTrue);

    final restored = _container(bridge)
      ..read(settingsControllerProvider.notifier).load();
    addTearDown(restored.dispose);
    final document = restored.read(settingsControllerProvider).document;
    expect(
      (document['CoreBasicItem'] as Map<String, dynamic>)['FutureToggle'],
      42,
    );
    expect(document['SubIndexId'], '');
    expect(
      (document['CoreBasicItem'] as Map<String, dynamic>)['Loglevel'],
      isNull,
    );
  });
}
