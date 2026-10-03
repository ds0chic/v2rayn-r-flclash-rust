import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

import 'support/fake_platform_bridge.dart';
import 'support/synthetic_runtime_bridge.dart';

ProviderContainer _container(FakePlatformBridge platform) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(4),
      platformBridgeProvider.overrideWithValue(platform),
      runtimeBridgeProvider.overrideWithValue(SyntheticRuntimeBridge()),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

Future<void> _pumpOption(
  WidgetTester tester,
  ProviderContainer container,
) async {
  tester.view.physicalSize = const Size(1200, 900);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(
        home: Builder(
          builder: (context) => TextButton(
            key: const ValueKey('fix08-open-settings'),
            onPressed: () => OptionSettingWindow.show(context),
            child: const Text('open'),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.byKey(const ValueKey('fix08-open-settings')));
  await tester.pumpAndSettle();
  expect(find.byType(OptionSettingWindow), findsOneWidget);
}

Finder _portField() => find.descendant(
  of: find.byKey(const ValueKey('settings-local-port')),
  matching: find.byType(TextField),
);

/// FIX-08 (SET-06): an invalid port blocks the save, keeps the window open
/// with the error shown, writes nothing and reports no success.
void main() {
  testWidgets('option save with an invalid port stays open with an error', (
    tester,
  ) async {
    final platform = FakePlatformBridge();
    final container = _container(platform);
    await _pumpOption(tester, container);

    await tester.enterText(_portField(), '');
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('settings-save')));
    await tester.pump();

    expect(find.byType(OptionSettingWindow), findsOneWidget);
    expect(find.text('请填写本地监听端口'), findsOneWidget);
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    expect(bridge.settingsRevision(), 0);
    expect(container.read(settingsControllerProvider).revision, 0);
    expect(platform.autostart, isEmpty);
  });
}
