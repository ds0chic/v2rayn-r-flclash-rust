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
      child: MaterialApp(
        home: Builder(
          builder: (context) => TextButton(
            key: const ValueKey('d201-open-settings'),
            onPressed: () => OptionSettingWindow.show(context),
            child: const Text('open'),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.byKey(const ValueKey('d201-open-settings')));
  await tester.pumpAndSettle();
  expect(find.byType(OptionSettingWindow), findsOneWidget);
}

Finder _portField() => find.descendant(
  of: find.byKey(const ValueKey('settings-local-port')),
  matching: find.byType(TextField),
);

/// D2-01: the in-process dialog must save with the revision captured when its
/// draft was seeded, not the live revision. An interim save from another
/// surface (e.g. the theme dialog's per-change saveGroup) must make this
/// dialog's save fail stale instead of silently overwriting it.
void main() {
  testWidgets('stale dialog draft does not overwrite an interim save', (
    tester,
  ) async {
    final container = _container();
    await _pumpOption(tester, container);

    // Edit the visible draft (based at revision 0).
    await tester.enterText(_portField(), '11809');
    await tester.pump();

    // Another surface saves in the meantime (whole revision 0 -> 1).
    final interim = container
        .read(settingsControllerProvider.notifier)
        .saveGroup('UiItem', <String, dynamic>{'CurrentTheme': 'Dark'});
    expect(interim.ok, isTrue);
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    expect(bridge.settingsRevision(), 1);

    await tester.tap(find.byKey(const ValueKey('settings-save')));
    await tester.pumpAndSettle();

    // The stale save must be rejected: window stays open, no second revision.
    expect(
      find.byType(OptionSettingWindow),
      findsOneWidget,
      reason: 'stale draft save must keep the window open with an error',
    );
    expect(
      bridge.settingsRevision(),
      1,
      reason: 'stale draft must not overwrite the interim save',
    );
  });
}
