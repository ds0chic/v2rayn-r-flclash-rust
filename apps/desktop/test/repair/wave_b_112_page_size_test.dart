// Wave B FLD-CFG-112: `SpeedTestItem.SpeedTestPageSize` data-layer consumer.
//
// Integrator ruling (SP-24 waveB-settings-family-2026-10-07.md): no upstream
// control exists (frozen `OptionSettingWindow.xaml` has no PageSize row), so
// acceptance is the data-layer consumer read — the canonical value must be
// consumed by `profiles_controller.dart:661` (`_resolveSpeedTestConfig` ->
// `configureSpeedTest(pageSize: ...)`) and survive reopen.
//
// Synthetic-only: SyntheticBridgePort stands in for the persisted settings
// tree; no network/10808/system-proxy/user secrets. Scope is test-only: no
// production file is touched by this half.
import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/sub_entry.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import '../support/fake_monitor_bridge.dart';
import '../support/fake_platform_bridge.dart';

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

/// Seed the canonical `SpeedTestItem.SpeedTestPageSize` on a fake bridge
/// through the real group-save seam (full-group replace, like the engine).
void _seedPageSize(SyntheticBridgePort bridge, Object? pageSize) {
  final load = bridge.getSettings();
  expect(load.ok, isTrue);
  final document = jsonDecode(load.settingsJson) as Map<String, dynamic>;
  final item = Map<String, dynamic>.from(document['SpeedTestItem'] as Map);
  item['SpeedTestPageSize'] = pageSize;
  final revision =
      decodeGroupRevisions(load.groupRevisionsJson)['SpeedTestItem'] ?? 0;
  expect(
    bridge.saveSettingsGroup('SpeedTestItem', jsonEncode(item), revision).ok,
    isTrue,
  );
}

/// Start a one-node speedtest through the controller (the
/// `profiles_controller.dart:661` consumer path) and return the page size
/// the controller handed to the bridge.
String _configuredPage(
  SyntheticBridgePort bridge,
  ProviderContainer container,
) {
  final controller = container.read(profilesControllerProvider.notifier);
  controller.selectRow('syn-000001');
  controller.startSpeedTest(ProfileAction.tcping);
  expect(bridge.lastSpeedTestConfig, isNotNull);
  return bridge.lastSpeedTestConfig!;
}

void main() {
  group('FLD-CFG-112 canonical page size consumed by profiles controller', () {
    test('default null falls back to the controller default (1000)', () {
      final bridge = SyntheticBridgePort(count: 10);
      final container = _container(bridge);
      expect(_configuredPage(bridge, container), contains('page=1000'));
    });

    test('canonical value is consumed and survives reopen (fake bridge)', () {
      final bridge = SyntheticBridgePort(count: 10);
      _seedPageSize(bridge, 7);

      final first = _container(bridge);
      expect(_configuredPage(bridge, first), contains('page=7'));

      // Reopen: a fresh controller over the same persisted tree still
      // consumes the canonical value.
      final second = _container(bridge);
      expect(_configuredPage(bridge, second), contains('page=7'));
    });
  });
}
