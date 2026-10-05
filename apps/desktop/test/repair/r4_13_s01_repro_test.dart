// R4-13.S01 repro: CheckUpdateItem was dead data.
//
// Before the fix the check-update window ignored the persisted
// `CheckUpdateItem` group (prerelease/ viaProxy/ SelectedCoreTypes) and never
// wrote changes back, so a saved value had no effect and reopening reset to
// defaults. This file fails on the pre-fix revision and passes after.
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

SyntheticBridgePort _bridgeWith({
  bool? preRelease,
  bool? viaProxy,
  List<String>? selectedTypes,
}) {
  final bridge = SyntheticBridgePort();
  bridge.saveSettingsJson(
    jsonEncode(<String, dynamic>{
      ...defaultSettingsJson(),
      'CheckUpdateItem': <String, dynamic>{
        'CheckPreReleaseUpdate': preRelease ?? false,
        'UpdateViaProxy': viaProxy ?? true,
        'SelectedCoreTypes': selectedTypes,
      },
    }),
    0,
  );
  return bridge;
}

void main() {
  test('persisted CheckUpdateItem seeds the check-update window', () {
    final bridge = _bridgeWith(
      preRelease: true,
      viaProxy: false,
      selectedTypes: <String>['xray'],
    );
    final container = _container(bridge);
    addTearDown(container.dispose);

    final state = container.read(updateControllerProvider);
    expect(state.prerelease, isTrue);
    expect(state.viaProxy, isFalse);
    expect(state.selected, <String>{'xray'});
  });

  test('toggling prerelease persists it for the next open', () {
    final bridge = _bridgeWith();
    final container = _container(bridge);
    addTearDown(container.dispose);

    final controller = container.read(updateControllerProvider.notifier);
    controller.setPrerelease(true);
    controller.setViaProxy(false);

    final reopened = _container(bridge);
    addTearDown(reopened.dispose);
    final state = reopened.read(updateControllerProvider);
    expect(state.prerelease, isTrue);
    expect(state.viaProxy, isFalse);
  });
}
