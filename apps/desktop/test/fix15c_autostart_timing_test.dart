import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'support/fake_platform_bridge.dart';
import 'support/synthetic_runtime_bridge.dart';

/// FIX-15C: `GuiItem.AutoRun` must only touch the Run key at the upstream commit
/// point (`SaveSettingAsync` -> `AutoStartupHandler.UpdateTask`, after a
/// successful `SaveConfig`), and only when the saved value changed.
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
        home: Scaffold(
          body: Builder(
            builder: (context) => TextButton(
              key: const ValueKey('fix15c-open-settings'),
              onPressed: () => OptionSettingWindow.show(context),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.byKey(const ValueKey('fix15c-open-settings')));
  await tester.pumpAndSettle();
  expect(find.byType(OptionSettingWindow), findsOneWidget);
}

Future<void> _openDisplayTabAndToggle(WidgetTester tester) async {
  await tester.tap(find.widgetWithText(Tab, 'v2rayN 设置'));
  await tester.pumpAndSettle();
  await tester.ensureVisible(find.byKey(const ValueKey('autorun-toggle')));
  await tester.pumpAndSettle();
  await tester.tap(find.byKey(const ValueKey('autorun-toggle')));
  await tester.pump();
}

Future<void> _save(WidgetTester tester) async {
  await tester.ensureVisible(find.byKey(const ValueKey('settings-save')));
  await tester.tap(find.byKey(const ValueKey('settings-save')));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('enabling AutoRun writes the Run key only on save', (
    tester,
  ) async {
    final platform = FakePlatformBridge();
    final container = _container(platform);
    await _pumpOption(tester, container);

    await _openDisplayTabAndToggle(tester);
    expect(platform.autostart, isEmpty, reason: 'toggle alone writes nothing');

    await _save(tester);
    expect(find.byType(OptionSettingWindow), findsNothing);
    expect(platform.autostart, hasLength(1));
    expect(platform.autostart.values.single, isTrue);
  });

  testWidgets('saving an unchanged AutoRun never touches the Run key', (
    tester,
  ) async {
    final platform = FakePlatformBridge();
    final container = _container(platform);
    await _pumpOption(tester, container);

    await _save(tester);
    expect(platform.autostart, isEmpty);
  });

  testWidgets('disabling a saved AutoRun clears the Run key on save', (
    tester,
  ) async {
    final platform = FakePlatformBridge();
    final container = _container(platform);
    await _pumpOption(tester, container);

    await _openDisplayTabAndToggle(tester);
    await _save(tester);
    final name = platform.autostart.keys.single;
    expect(platform.autostart[name], isTrue);

    await tester.tap(find.byKey(const ValueKey('fix15c-open-settings')));
    await tester.pumpAndSettle();
    await _openDisplayTabAndToggle(tester);
    await _save(tester);

    expect(platform.autostart, hasLength(1));
    expect(platform.autostart[name], isFalse);
  });
}
