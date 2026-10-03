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

/// FIX-08 (SET-06): edits + AutoRun toggle followed by 取消 leave no trace:
/// no settings revision, no Run-key write (upstream only runs
/// `AutoStartupHandler.UpdateTask` after a successful `SaveConfig`).
void main() {
  testWidgets('option cancel discards edits and never touches autostart', (
    tester,
  ) async {
    final platform = FakePlatformBridge();
    final container = _container(platform);
    await _pumpOption(tester, container);

    await tester.enterText(_portField(), '11809');
    await tester.pump();
    // Toggle AutoRun on (draft only since FIX-08). The toggle lives on the
    // 显示 tab, which must be selected first.
    await tester.tap(find.text('显示'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.byKey(const ValueKey('autorun-toggle')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('autorun-toggle')));
    await tester.pump();

    await tester.tap(find.text('取消'));
    await tester.pumpAndSettle();

    expect(find.byType(OptionSettingWindow), findsNothing);

    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    expect(bridge.settingsRevision(), 0);
    expect(container.read(settingsControllerProvider).revision, 0);
    // No host autostart write happened (toggle alone must not write).
    expect(platform.autostart, isEmpty);
  });
}
