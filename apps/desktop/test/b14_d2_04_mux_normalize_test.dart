import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

/// D2-04 (parity part): upstream `OptionSettingViewModel.SaveSettingAsync`
/// stores `Mux4RayConcurrency/XudpConcurrency <= 0` as null. A literal 0 or
/// negative value must not be persisted.
void main() {
  test('non-positive mux concurrency is persisted as null', () {
    final bridge = SyntheticBridgePort();
    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(bridge),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(4),
      ],
    );
    addTearDown(container.dispose);

    final notifier = container.read(settingsControllerProvider.notifier);
    notifier.load();
    expect(container.read(settingsControllerProvider).loaded, isTrue);

    final draft = notifier.draft();
    (draft['Mux4RayItem'] as Map<String, dynamic>)['Concurrency'] = 0;
    (draft['Mux4RayItem'] as Map<String, dynamic>)['XudpConcurrency'] = -5;

    final result = notifier.saveDocument(draft);
    expect(result.ok, isTrue);

    final persisted =
        jsonDecode(bridge.getSettings().settingsJson) as Map<String, dynamic>;
    final mux = persisted['Mux4RayItem'] as Map<String, dynamic>;
    expect(
      mux['Concurrency'],
      isNull,
      reason: 'upstream stores Concurrency <= 0 as null',
    );
    expect(
      mux['XudpConcurrency'],
      isNull,
      reason: 'upstream stores XudpConcurrency <= 0 as null',
    );
  });
}
