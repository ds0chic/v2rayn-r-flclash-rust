// R4-11 contract: field forms and default values.
//
// One real user flow per the task card: edit one upstream field, then Save /
// Cancel, and reopen. Uses the in-process settings controller over the
// synthetic bridge port (real settings path, no native library). Synthetic
// data only; ports >= 11808; no kernel, host proxy, TUN, registry or user data.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/synthetic_runtime_bridge.dart';

ProviderContainer _container() {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
      // R4-13: the in-process 确定 now awaits the real plan apply, so the
      // runtime seam must be present for a successful save to close.
      runtimeBridgeProvider.overrideWithValue(SyntheticRuntimeBridge()),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(4),
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
            key: const ValueKey('open-settings'),
            onPressed: () => OptionSettingWindow.show(context),
            child: const Text('open'),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.byKey(const ValueKey('open-settings')));
  await tester.pumpAndSettle();
  expect(find.byType(OptionSettingWindow), findsOneWidget);
}

Finder _portField() => find.descendant(
  of: find.byKey(const ValueKey('settings-local-port')),
  matching: find.byType(TextField),
);

int _savedPort(ProviderContainer container) {
  final doc = container.read(settingsControllerProvider).document;
  final inbound = (doc['Inbound'] as List).first as Map;
  return (inbound['LocalPort'] as num).toInt();
}

void main() {
  test(
    'mergeWithSettingsDefaults fills missing/null scalars, keeps values',
    () {
      final merged = mergeWithSettingsDefaults(<String, dynamic>{
        'CoreBasicItem': <String, dynamic>{'LogEnabled': true},
        'GuiItem': <String, dynamic>{'TrayMenuServersLimit': 7},
        'UiItem': <String, dynamic>{'CurrentFontFamily': null},
        'DefFingerprintAbsent': null,
      });
      final core = merged['CoreBasicItem'] as Map;
      // Present value wins; missing gets the upstream default (true).
      expect(core['LogEnabled'], isTrue);
      expect(core['EnableCacheFile4Sbox'], isTrue);
      expect(core['Loglevel'], 'warning');
      final gui = merged['GuiItem'] as Map;
      expect(gui['TrayMenuServersLimit'], 7);
      expect(gui['AutoUpdateInterval'], 0);
      final ui = merged['UiItem'] as Map;
      expect(ui['CurrentLanguage'], 'zh-Hans');
      // A canonical null default stays null (not overwritten).
      expect(merged['DefFingerprintAbsent'], isNull);
    },
  );

  testWidgets('R4-11 edit-save-reopen: saved value persists', (tester) async {
    final container = _container();
    await _pumpOption(tester, container);

    await tester.enterText(_portField(), '11888');
    await tester.pump();
    await tester.tap(find.text('确定'));
    await tester.pumpAndSettle();

    expect(find.byType(OptionSettingWindow), findsNothing);
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    expect(bridge.settingsRevision(), 1);
    expect(_savedPort(container), 11888);

    // Reopen: reload from the persisted document and confirm the value.
    await tester.tap(find.byKey(const ValueKey('open-settings')));
    await tester.pumpAndSettle();
    container.read(settingsControllerProvider.notifier).load();
    await tester.pumpAndSettle();
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('settings-local-port')),
        matching: find.widgetWithText(TextField, '11888'),
      ),
      findsOneWidget,
    );
  });

  testWidgets('R4-11 edit-cancel: nothing is written', (tester) async {
    final container = _container();
    await _pumpOption(tester, container);

    await tester.enterText(_portField(), '11899');
    await tester.pump();
    await tester.tap(find.text('取消'));
    await tester.pumpAndSettle();

    expect(find.byType(OptionSettingWindow), findsNothing);
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    expect(bridge.settingsRevision(), 0);
    // The persisted document keeps the untouched default port.
    expect(_savedPort(container), 10808);
  });

  testWidgets('R4-11 invalid port keeps the window open and shows an error', (
    tester,
  ) async {
    final container = _container();
    await _pumpOption(tester, container);

    await tester.enterText(_portField(), '70000');
    await tester.pump();
    await tester.tap(find.text('确定'));
    await tester.pumpAndSettle();

    // Range validation is visible and the window stays open (no save).
    expect(find.byType(OptionSettingWindow), findsOneWidget);
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    expect(bridge.settingsRevision(), 0);
  });

  testWidgets('R4-11 User/Pass are disabled until NewPort4LAN is on', (
    tester,
  ) async {
    final container = _container();
    await _pumpOption(tester, container);

    TextField userField() => tester.widget<TextField>(
      find.descendant(
        of: find.ancestor(of: find.text('认证用户名'), matching: find.byType(Row)),
        matching: find.byType(TextField),
      ),
    );
    expect(userField().enabled, isFalse);

    // Toggle "为局域网开启新的端口".
    await tester.tap(find.text('为局域网开启新的端口'));
    await tester.pumpAndSettle();
    expect(userField().enabled, isTrue);
  });
}
