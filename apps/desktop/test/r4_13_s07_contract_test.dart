// R4-13.S07 settings-save-to-effect contract (Fragment4RayItem instance).
//
// The fragment group must survive the UI load-parity merge and the save ->
// reopen round trip, including the legacy `Length`/`Interval` migration
// (FLD-CFG-154/155) that upstream applies in `ConfigHandler.LoadConfig`.
// Synthetic bridge only: no native library, core, port, host proxy/TUN or
// user data.
import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/counting_runtime_bridge.dart';
import 'support/fake_platform_bridge.dart';

ProviderContainer _container(BridgePort bridge) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    runtimeBridgeProvider.overrideWithValue(CountingRuntimeBridge()),
    platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
    uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
  ],
);

Map<String, dynamic> _fragment(Map<String, dynamic> document) =>
    document['Fragment4RayItem'] as Map<String, dynamic>;

void main() {
  test('legacy Length/Interval seed empty fragment lists', () {
    final merged = mergeWithSettingsDefaults(<String, dynamic>{
      'Fragment4RayItem': <String, dynamic>{
        'Packets': 'tlshello',
        'Lengths': <dynamic>[],
        'Delays': <dynamic>[],
        'Length': '100-200',
        'Interval': '30-40',
      },
    });
    expect(_fragment(merged)['Lengths'], <String>['100-200']);
    expect(_fragment(merged)['Delays'], <String>['30-40']);
  });

  test('empty lists without legacy get the frozen defaults', () {
    final merged = mergeWithSettingsDefaults(<String, dynamic>{
      'Fragment4RayItem': <String, dynamic>{
        'Lengths': <dynamic>[],
        'Delays': <dynamic>[],
      },
    });
    expect(_fragment(merged)['Lengths'], <String>['50-100']);
    expect(_fragment(merged)['Delays'], <String>['10-20']);
  });

  test('a non-empty list is never overwritten by the legacy scalar', () {
    final merged = mergeWithSettingsDefaults(<String, dynamic>{
      'Fragment4RayItem': <String, dynamic>{
        'Lengths': <dynamic>['5-10'],
        'Delays': <dynamic>['1-2'],
        'Length': '100-200',
        'Interval': '30-40',
      },
    });
    expect(_fragment(merged)['Lengths'], <String>['5-10']);
    expect(_fragment(merged)['Delays'], <String>['1-2']);
  });

  test('promoted fragment lists persist and are visible on reopen', () async {
    final bridge = SyntheticBridgePort();
    final seed = defaultSettingsJson();
    seed['Fragment4RayItem'] = <String, dynamic>{
      'Packets': 'tlshello',
      'Lengths': <dynamic>[],
      'Delays': <dynamic>[],
      'MaxSplit': '0',
      'Length': '100-200',
      'Interval': '30-40',
    };
    bridge.saveSettingsJson(jsonEncode(seed), 0);

    final container = _container(bridge);
    addTearDown(container.dispose);
    final controller = container.read(settingsControllerProvider.notifier);
    controller.load();
    // The option window seeds its draft through the load-parity merge.
    final draft = mergeWithSettingsDefaults(controller.draft());
    final outcome = await controller.saveAndApply(draft);
    expect(outcome.ok, isTrue);
    expect(outcome.saved, isTrue);

    final reopened = _container(bridge);
    addTearDown(reopened.dispose);
    reopened.read(settingsControllerProvider.notifier).load();
    final restored = _fragment(
      reopened.read(settingsControllerProvider).document,
    );
    expect(restored['Lengths'], <String>['100-200']);
    expect(restored['Delays'], <String>['30-40']);
  });
}
