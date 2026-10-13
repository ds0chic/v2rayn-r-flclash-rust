import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';
import 'package:v2rayn_desktop/features/settings/settings_fields.dart';

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
  tester.view.physicalSize = const Size(1200, 900);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: OptionSettingWindow())),
    ),
  );
  await tester.pump();
  await tester.pumpAndSettle();
}

Finder _dirtyDots() => find.byWidgetPredicate(
  (w) =>
      w.key is ValueKey &&
      (w.key as ValueKey).value is String &&
      ((w.key as ValueKey).value as String).startsWith('settings-dirty-'),
);

void main() {
  test('UB2-08: modifiedSettingsGroups flags only changed groups', () {
    final canonical = defaultSettingsJson();
    final draft = mergeWithSettingsDefaults(<String, dynamic>{});
    expect(modifiedSettingsGroups(draft, canonical), isEmpty);
    expect(settingsValueEquals(10808, 10808.0), isTrue);

    (draft['SpeedTestItem'] as Map<String, dynamic>)['SpeedTestTimeout'] = 30;
    expect(modifiedSettingsGroups(draft, canonical), <String>{'SpeedTestItem'});
  });

  testWidgets('UB2-08: search finds a row inside a collapsed section', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);

    // Display tab, then fold the speedtest section.
    await tester.tap(find.text('v2rayN 设置'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('测速'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('测速'));
    await tester.pumpAndSettle();
    expect(find.text('多线程测试时的并发数量'), findsNothing);

    // Non-live search: results appear only after an explicit submit.
    await tester.enterText(find.byKey(const ValueKey('settings-search')), '并发');
    await tester.pump();
    expect(
      find.byKey(const ValueKey('settings-search-result-0')),
      findsNothing,
    );
    await tester.tap(find.byKey(const ValueKey('settings-search-go')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('settings-search-result-0')));
    await tester.pumpAndSettle();

    expect(find.text('多线程测试时的并发数量'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('settings-search-result-0')),
      findsNothing,
    );
  });

  testWidgets('UB2-08: editing one field marks exactly one dirty dot', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);

    // Baseline first: exactly one NEW dot must appear, whatever the seed
    // already deviates (seed purity is not this test's business).
    final baseline = _dirtyDots().evaluate().length;

    await tester.tap(find.text('v2rayN 设置'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(
      find.byKey(const ValueKey('settings-keep-older-dedupl')),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('settings-keep-older-dedupl')));
    await tester.pump();

    expect(_dirtyDots(), findsNWidgets(baseline + 1));
    expect(
      find.byKey(const ValueKey('settings-dirty-GuiItem.KeepOlderDedupl')),
      findsOneWidget,
    );
  });

  testWidgets('UB2-08: per-group reset restores canonical, others untouched', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);

    await tester.tap(find.text('v2rayN 设置'));
    await tester.pumpAndSettle();
    final baseline = _dirtyDots().evaluate().length;

    // Diverge SpeedTestItem.SpeedTestTimeout (10 -> 30).
    final row = find.byWidgetPredicate(
      (w) => w is SettingsNumberField && w.label == '测速单个超时值',
    );
    await tester.ensureVisible(row);
    await tester.pumpAndSettle();
    await tester.enterText(
      find.descendant(of: row, matching: find.byType(TextField)),
      '30',
    );
    await tester.pump();
    // Diverge GuiItem.AutoRun in parallel.
    await tester.ensureVisible(find.byKey(const ValueKey('autorun-toggle')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('autorun-toggle')));
    await tester.pump();
    expect(_dirtyDots(), findsNWidgets(baseline + 2));

    // Reset only the speedtest section (double-confirm: button + dialog).
    await tester.ensureVisible(
      find.byKey(const ValueKey('settings-reset-display-speedtest')),
    );
    await tester.pumpAndSettle();
    await tester.tap(
      find.byKey(const ValueKey('settings-reset-display-speedtest')),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('settings-reset-confirm')));
    await tester.pumpAndSettle();

    final saved =
        jsonDecode(bridge.getSettings().settingsJson) as Map<String, dynamic>;
    // Reset group restored to canonical…
    expect(
      (saved['SpeedTestItem'] as Map<String, dynamic>)['SpeedTestTimeout'],
      10,
    );
    // …every other group byte-identical to canonical (the GuiItem draft edit
    // above stays a pending draft edit — the reset never wrote it).
    expect(
      settingsValueEquals(saved['GuiItem'], defaultSettingsJson()['GuiItem']),
      isTrue,
    );

    expect(
      find.byKey(
        const ValueKey('settings-dirty-SpeedTestItem.SpeedTestTimeout'),
      ),
      findsNothing,
    );
    expect(
      find.byKey(const ValueKey('settings-dirty-GuiItem.AutoRun')),
      findsOneWidget,
    );
    expect(_dirtyDots(), findsNWidgets(baseline + 1));
  });
}
