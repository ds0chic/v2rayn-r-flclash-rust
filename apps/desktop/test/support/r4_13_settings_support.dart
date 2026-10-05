// Shared R4-13 instance helpers: synthetic bridge -> save -> reopen.
//
// Synthetic data only: no native library, core process, port, host
// proxy/TUN/registry or user data.
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

import 'counting_runtime_bridge.dart';
import 'fake_platform_bridge.dart';

ProviderContainer r4_13Container(BridgePort bridge) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    runtimeBridgeProvider.overrideWithValue(CountingRuntimeBridge()),
    platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
    uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
  ],
);

/// A synthetic bridge seeded with the canonical defaults (or [seed]).
SyntheticBridgePort seededBridge([Map<String, dynamic>? seed]) {
  final bridge = SyntheticBridgePort();
  bridge.saveSettingsJson(jsonEncode(seed ?? defaultSettingsJson()), 0);
  return bridge;
}

/// Seed, mutate one draft, save through the real controller and reopen.
///
/// Returns the reopened persisted document so each instance can assert the
/// value is visible after reopen (not just held in the save draft).
Future<Map<String, dynamic>> saveAndReopen(
  BridgePort bridge,
  void Function(Map<String, dynamic> draft) mutate,
) async {
  final container = r4_13Container(bridge);
  final controller = container.read(settingsControllerProvider.notifier);
  controller.load();
  final draft = mergeWithSettingsDefaults(controller.draft());
  mutate(draft);
  final outcome = await controller.saveAndApply(draft);
  expect(outcome.saved, isTrue, reason: 'the document must persist');
  expect(outcome.ok, isTrue, reason: 'a successful save must apply');
  container.dispose();

  final reopened = r4_13Container(bridge);
  addTearDown(reopened.dispose);
  reopened.read(settingsControllerProvider.notifier).load();
  return reopened.read(settingsControllerProvider).document;
}
