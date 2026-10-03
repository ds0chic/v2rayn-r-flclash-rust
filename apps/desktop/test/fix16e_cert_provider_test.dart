import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

ProviderContainer _container(SyntheticBridgePort bridge) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
    profileRowCountProvider.overrideWithValue(1),
  ],
);

void main() {
  test('FIX-16E: provider options match upstream and normalize', () {
    expect(rootCertProviders, <String>['system', 'chrome', 'mozilla']);
    expect(defaultRootCertProvider, 'system');
    expect(normalizeRootCertProvider('mozilla'), 'mozilla');
    expect(normalizeRootCertProvider('chrome'), 'chrome');
    expect(normalizeRootCertProvider('system'), 'system');
    expect(normalizeRootCertProvider('bogus'), 'system');
    expect(normalizeRootCertProvider(''), 'system');
    expect(normalizeRootCertProvider(null), 'system');
  });

  test('FIX-16E: save normalizes invalid provider and keeps valid value', () {
    final bridge = SyntheticBridgePort(count: 1);
    final container = _container(bridge);
    addTearDown(container.dispose);
    final controller = container.read(settingsControllerProvider.notifier);
    controller.load();

    final draft = controller.draft();
    final gui = draft['GuiItem'] as Map<String, dynamic>;
    gui['RootCertProvider'] = 'bogus';
    gui['EnableHWA'] = true;
    final result = controller.saveDocument(draft);
    expect(result.ok, isTrue, reason: result.error?.messageKey);

    final saved =
        jsonDecode(bridge.getSettings().settingsJson) as Map<String, dynamic>;
    expect(saved['GuiItem']['RootCertProvider'], 'system');
    expect(saved['GuiItem']['EnableHWA'], isTrue);

    // A valid value survives unchanged.
    final second = controller.draft();
    (second['GuiItem'] as Map<String, dynamic>)['RootCertProvider'] = 'mozilla';
    final result2 = controller.saveDocument(second);
    expect(result2.ok, isTrue, reason: result2.error?.messageKey);
    final saved2 =
        jsonDecode(bridge.getSettings().settingsJson) as Map<String, dynamic>;
    expect(saved2['GuiItem']['RootCertProvider'], 'mozilla');
  });
}
