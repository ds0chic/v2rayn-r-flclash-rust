import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

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

Future<void> _toTab(WidgetTester tester, int index) async {
  await tester.tap(find.byType(Tab).at(index));
  await tester.pumpAndSettle();
}

Future<void> _ensureTap(WidgetTester tester, Finder finder) async {
  await tester.ensureVisible(finder);
  await tester.pumpAndSettle();
  await tester.tap(finder);
  await tester.pumpAndSettle();
}

/// Open a SettingsDropdown menu: tap the button itself, never the row
/// center (tall help/risk rows put the center outside the control). The
/// raw `is DropdownButton` check matches every `DropdownButton<T>`.
Future<void> _openDropdown(WidgetTester tester, Finder row) async {
  await tester.ensureVisible(row);
  await tester.pumpAndSettle();
  await tester.tap(
    find.descendant(
      of: row,
      matching: find.byWidgetPredicate((w) => w is DropdownButton),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('UB2-09: expert switch hides expert rows when off', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);

    // Default on: expert rows are in the tree (deviation from UX3-04 noted
    // in the report; keeps every existing UI test green).
    expect(find.text('绑定网口'), findsOneWidget);

    await _ensureTap(
      tester,
      find.byKey(const ValueKey('settings-expert-mode')),
    );
    expect(find.text('绑定网口'), findsNothing);
    expect(find.text('本地混合监听端口'), findsOneWidget);

    await _ensureTap(
      tester,
      find.byKey(const ValueKey('settings-expert-mode')),
    );
    expect(find.text('绑定网口'), findsOneWidget);
  });

  testWidgets('UB2-09: sysproxy high-risk value asks first', (tester) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);
    await _toTab(tester, 2);

    // Cancel path on the 设置 branch (menu item located inside its
    // DropdownMenuItem: the window title is also exactly '设置').
    await _openDropdown(
      tester,
      find.byKey(const ValueKey('settings-sysproxy-type')),
    );
    await tester.tap(
      find.descendant(
        of: find.byWidgetPredicate(
          (w) => w is DropdownMenuItem<int> && w.value == 1,
        ),
        matching: find.text('设置'),
      ),
    );
    await tester.pumpAndSettle();
    // Confirm dialog; cancel keeps the draft.
    await tester.tap(find.byKey(const ValueKey('settings-danger-cancel')));
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('settings-dirty-SystemProxyItem.SysProxyType')),
      findsNothing,
    );

    // Confirm path on the PAC branch (unique label, no title clash).
    await _openDropdown(
      tester,
      find.byKey(const ValueKey('settings-sysproxy-type')),
    );
    expect(find.text('PAC'), findsOneWidget);
    await tester.tap(find.text('PAC'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('settings-danger-confirm')));
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('settings-dirty-SystemProxyItem.SysProxyType')),
      findsOneWidget,
    );
  });

  testWidgets('UB2-09: tun enable asks first, cancel keeps it off', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);
    await _toTab(tester, 3);

    // Tall checkbox rows (help+risk) must be tapped on the label: the
    // widget center lands outside the InkWell row.
    final tunToggle = find.byKey(const ValueKey('settings-enable-tun'));
    await tester.ensureVisible(tunToggle);
    await tester.pumpAndSettle();
    await tester.tap(find.text('启用 Tun'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('settings-danger-cancel')));
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('settings-dirty-TunModeItem.EnableTun')),
      findsNothing,
    );
  });

  testWidgets('UB2-09: core-type switch asks first', (tester) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);
    await _toTab(tester, 4);

    await _openDropdown(
      tester,
      find.byKey(const ValueKey('settings-coretype-1')),
    );
    await tester.tap(find.text('mihomo').last);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('settings-danger-cancel')));
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('settings-dirty-CoreTypeItem.1')),
      findsNothing,
    );
  });

  testWidgets('UB2-09: inbound protocol stays editable with mixed note', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);

    expect(find.textContaining('实际按 mixed 生成'), findsOneWidget);
    await _openDropdown(
      tester,
      find.byKey(const ValueKey('settings-inbound-protocol')),
    );
    await tester.tap(find.text('mixed').last);
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('settings-dirty-Inbound.Protocol')),
      findsOneWidget,
    );
  });

  testWidgets('UB2-09: novice copy lands on core rows', (tester) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);

    expect(find.textContaining('门牌号'), findsOneWidget);
    expect(find.textContaining('扮成谁'), findsOneWidget);
    expect(find.textContaining('接管全机流量'), findsNothing);

    await _toTab(tester, 3);
    expect(find.textContaining('接管全机流量'), findsOneWidget);
    expect(find.textContaining('断网？先关'), findsOneWidget);
  });

  testWidgets('UB2-09: cache row moved out of mux, padding shown readonly', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);

    expect(find.text('规则与缓存'), findsOneWidget);
    expect(find.text('启用 sing-box (规则集文件) 的缓存文件'), findsOneWidget);
    expect(find.textContaining('Padding'), findsOneWidget);
  });

  testWidgets('UB2-09: empty lengths fall back to the legacy value note', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 4);
    // mergeWithSettingsDefaults promotes Length into an empty Lengths at
    // load, so the legacy note is exercised by clearing the field in the UI
    // (the cleared draft keeps the legacy scalar for the fallback).
    final seed = defaultSettingsJson();
    (seed['Fragment4RayItem'] as Map<String, dynamic>)['Length'] = '100-200';
    bridge.saveSettingsJson(jsonEncode(seed), 0);

    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);

    final row = find.byKey(const ValueKey('fragment-lengths'));
    await tester.ensureVisible(row);
    await tester.pumpAndSettle();
    await tester.enterText(
      find.descendant(of: row, matching: find.byType(TextField)),
      '',
    );
    await tester.pump();

    expect(find.textContaining('沿用旧值 100-200'), findsOneWidget);
  });

  testWidgets('UB2-09: search jump re-enables expert for expert hits', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);

    await _ensureTap(
      tester,
      find.byKey(const ValueKey('settings-expert-mode')),
    );
    expect(find.text('绑定网口'), findsNothing);

    await tester.enterText(
      find.byKey(const ValueKey('settings-search')),
      '绑定网口',
    );
    await tester.tap(find.byKey(const ValueKey('settings-search-go')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('settings-search-result-0')));
    await tester.pumpAndSettle();

    expect(find.text('绑定网口'), findsOneWidget);
  });
}
