// R4-13.S01 settings-save-to-effect contract (CheckUpdateItem instance).
//
// Saving the CheckUpdateItem group must reach its real consumer: the update
// check request (`t16CheckUpdates`) and reopening the window. Synthetic bridge
// only: no native library, core, port, host proxy/TUN/registry or user data.
import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';
import 'package:v2rayn_desktop/features/update/update_controller.dart';

ProviderContainer _container(BridgePort bridge) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
  ],
);

SyntheticBridgePort _seeded({
  bool preRelease = false,
  bool viaProxy = true,
  List<String>? selectedTypes,
}) {
  final bridge = SyntheticBridgePort();
  bridge.saveSettingsJson(
    jsonEncode(<String, dynamic>{
      ...defaultSettingsJson(),
      'CheckUpdateItem': <String, dynamic>{
        'CheckPreReleaseUpdate': preRelease,
        'UpdateViaProxy': viaProxy,
        'SelectedCoreTypes': selectedTypes,
      },
    }),
    0,
  );
  return bridge;
}

void main() {
  test('persisted CheckUpdateItem seeds prerelease/viaProxy/selection', () {
    final bridge = _seeded(
      preRelease: true,
      viaProxy: false,
      selectedTypes: <String>['xray', 'mihomo'],
    );
    final container = _container(bridge);
    addTearDown(container.dispose);

    final state = container.read(updateControllerProvider);
    expect(state.prerelease, isTrue);
    expect(state.viaProxy, isFalse);
    expect(state.selected, <String>{'xray', 'mihomo'});
  });

  test('missing CheckUpdateItem falls back to upstream defaults', () {
    // No settings stored at all: UpdateViaProxy defaults true and every
    // supported target is selected (SelectedCoreTypes null => all).
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    addTearDown(container.dispose);

    final state = container.read(updateControllerProvider);
    expect(state.prerelease, isFalse);
    expect(state.viaProxy, isTrue);
    final supported = state.targets
        .where((target) => target.supported)
        .map((target) => target.core)
        .toSet();
    expect(state.selected, supported);
  });

  test('toggleCore persists SelectedCoreTypes for the next open', () {
    final bridge = _seeded(
      selectedTypes: <String>['xray', 'mihomo', 'sing_box'],
    );
    final container = _container(bridge);
    addTearDown(container.dispose);

    final controller = container.read(updateControllerProvider.notifier);
    controller.toggleCore('mihomo', false);

    final reopened = _container(bridge);
    addTearDown(reopened.dispose);
    final state = reopened.read(updateControllerProvider);
    expect(state.selected, <String>{'xray', 'sing_box'});
  });

  test('the update check consumer receives the persisted parameters', () async {
    final bridge = _seeded(
      preRelease: true,
      viaProxy: true,
      selectedTypes: <String>['xray'],
    );
    bridge.proxyAvailable = true;
    final container = _container(bridge);
    addTearDown(container.dispose);

    await container.read(updateControllerProvider.notifier).checkOnly();

    expect(
      bridge.t16Calls.last,
      'check_updates:xray:true:true',
      reason: 'prerelease/viaProxy/cores must reach the real check request',
    );
  });
}
