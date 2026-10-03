import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/locale_config.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

ProviderContainer _container(SyntheticBridgePort bridge) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
    profileRowCountProvider.overrideWithValue(4),
  ],
);

Future<void> _pumpWindow(
  WidgetTester tester,
  ProviderContainer container,
) async {
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: OptionSettingWindow())),
    ),
  );
  await tester.pump();
  await tester.pumpAndSettle();
}

Map<String, dynamic> _saved(SyntheticBridgePort bridge) =>
    jsonDecode(bridge.getSettings().settingsJson) as Map<String, dynamic>;

void main() {
  test('FIX-16: localeForLanguage maps persisted codes', () {
    expect(
      localeForLanguage('zh-Hans'),
      const Locale.fromSubtags(languageCode: 'zh', scriptCode: 'Hans'),
    );
    expect(
      localeForLanguage('zh-Hant'),
      const Locale.fromSubtags(languageCode: 'zh', scriptCode: 'Hant'),
    );
    expect(localeForLanguage('en'), const Locale('en'));
    expect(localeForLanguage(''), isNull);
    expect(localeForLanguage(null), isNull);
  });

  test('FIX-16: language + font flow into the live shell and theme', () {
    final container = ProviderContainer(
      overrides: [uiStateStoreProvider.overrideWithValue(MemoryUiStateStore())],
    );
    addTearDown(container.dispose);

    container.read(uiShellControllerProvider.notifier).applySettingsDocument(
      <String, dynamic>{
        'UiItem': <String, dynamic>{
          'CurrentLanguage': 'en',
          'CurrentFontFamily': 'Consolas',
          'CurrentFontSize': 14,
        },
      },
    );

    final shell = container.read(uiShellControllerProvider);
    expect(shell.language, 'en');
    expect(localeForLanguage(shell.language), const Locale('en'));
    expect(shell.fontFamily, 'Consolas');
    expect(shell.fontSize, 14);

    final theme = buildAppTheme(
      Brightness.light,
      fontFamily: shell.fontFamily,
      fontSize: shell.fontSize,
    );
    expect(theme.textTheme.bodyMedium?.fontFamily, 'Consolas');
  });

  testWidgets('FIX-16: save then reopen restores HWA/cert/language', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1200, 900);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);

    await _pumpWindow(tester, container);
    await tester.tap(find.text('显示'));
    await tester.pumpAndSettle();

    // 1. Enable HWA (GuiItem.EnableHWA).
    await tester.ensureVisible(
      find.byKey(const ValueKey('settings-enable-hwa')),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('settings-enable-hwa')));
    await tester.pump();

    // 2. Root certificate source (GuiItem.RootCertProvider -> mozilla).
    await tester.ensureVisible(
      find.byKey(const ValueKey('settings-root-cert')),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('settings-root-cert')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Mozilla').last);
    await tester.pumpAndSettle();

    // 3. Language (UiItem.CurrentLanguage -> en).
    await tester.ensureVisible(find.byKey(const ValueKey('settings-language')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('settings-language')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('English').last);
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey('settings-save')));
    await tester.pumpAndSettle();

    final doc = _saved(bridge);
    expect(doc['GuiItem']['EnableHWA'], isTrue);
    expect(doc['GuiItem']['RootCertProvider'], 'mozilla');
    expect(doc['UiItem']['CurrentLanguage'], 'en');
    expect(bridge.settingsRevision(), 1);

    // Reopen with a fresh container bound to the same (persisted) bridge.
    // Tear the old tree down first: the window is rendered as the home body,
    // so its Save pop empties the home navigator.
    await tester.pumpWidget(const SizedBox());
    await tester.pumpAndSettle();
    final reopen = _container(bridge);
    addTearDown(reopen.dispose);
    await _pumpWindow(tester, reopen);
    await tester.tap(find.text('显示'));
    await tester.pumpAndSettle();

    expect(
      reopen.read(settingsControllerProvider).loaded,
      isTrue,
      reason: 'reopen settings must load',
    );
    expect(
      (reopen.read(settingsControllerProvider).document['GuiItem']
          as Map)['EnableHWA'],
      isTrue,
      reason: 'reopen document must carry HWA',
    );

    await tester.ensureVisible(
      find.byKey(const ValueKey('settings-enable-hwa')),
    );
    await tester.pumpAndSettle();
    final hwaBox = tester.widget<Checkbox>(
      find.descendant(
        of: find.byKey(const ValueKey('settings-enable-hwa')),
        matching: find.byType(Checkbox),
      ),
    );
    expect(hwaBox.value, isTrue, reason: 'EnableHWA must survive reopen');

    final cert = tester.widget<DropdownButton<String>>(
      find.descendant(
        of: find.byKey(const ValueKey('settings-root-cert')),
        matching: find.byType(DropdownButton<String>),
      ),
    );
    expect(cert.value, 'mozilla');
  });
}
