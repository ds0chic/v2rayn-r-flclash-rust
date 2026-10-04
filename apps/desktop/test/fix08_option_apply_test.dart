import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

import 'support/fake_platform_bridge.dart';
import 'support/synthetic_runtime_bridge.dart';

ProviderContainer _container() {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(4),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
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
      // Open the window as a dialog so 取消/保存/应用 pop a real route and
      // closure assertions are meaningful.
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

/// FIX-08 first flow ("参数修改 → 确定"): editing a parameter then tapping
/// 确定 (upstream's single confirm button) persists the visible draft and
/// applies the plan; reopening shows the saved value.
void main() {
  testWidgets('option confirm saves the visible draft then applies', (
    tester,
  ) async {
    final container = _container();
    await _pumpOption(tester, container);

    await tester.enterText(_portField(), '11809');
    await tester.pump();
    await tester.tap(find.text('确定'));
    await tester.pumpAndSettle();

    // The window closed after a successful save.
    expect(find.byType(OptionSettingWindow), findsNothing);
    // The draft value (not a stale document) was persisted.
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    expect(bridge.settingsRevision(), 1);
    final doc = container.read(settingsControllerProvider).document;
    final inbound = (doc['Inbound'] as List).first as Map;
    expect(inbound['LocalPort'], 11809);
    // The real plan was applied after the save.
    expect(container.read(runtimeControllerProvider).state, 'Running');
  });
}
