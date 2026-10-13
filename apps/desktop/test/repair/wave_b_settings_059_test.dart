// Wave B (FLD-CFG-059): GuiItem KeepOlderDedupl entry.
//
// Split from `wave_b_settings_059_079_test.dart` (via the 059/060 half): the
// locked flutter_tester segfaults after a handful of settings-window builds
// in one process, so each entry keeps its own file.
//
// 原版对照（v2rayN 7.25.4 / 7d6a967，只读 work/）：
// - `ConfigItems.cs:67-85` `GUIItem`（`KeepOlderDedupl=false`，0=关闭）。
//
// 本文件合成数据 + SyntheticBridgePort/SyntheticRuntimeBridge（无 socket、
// 无代理、无 10808/注册表/系统代理操作），断言正式入口→保存→重开。
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import '../support/fake_platform_bridge.dart';
import '../support/synthetic_runtime_bridge.dart';

ProviderContainer _container(SyntheticBridgePort bridge) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge),
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
  ProviderContainer container, {
  ValueKey<String> openKey = const ValueKey('waveb-display-open'),
}) async {
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
            key: openKey,
            onPressed: () => OptionSettingWindow.show(context),
            child: const Text('open'),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.byKey(openKey));
  await tester.pumpAndSettle();
  expect(find.byType(OptionSettingWindow), findsOneWidget);
}

/// 切到“显示”页（第 2 个 Tab）。
Future<void> _showDisplay(WidgetTester tester) async {
  await tester.tap(find.byType(Tab).at(1));
  await tester.pumpAndSettle();
}

Future<void> _reopen(WidgetTester tester, SyntheticBridgePort bridge) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pumpAndSettle();
  await _pumpOption(tester, _container(bridge));
  await _showDisplay(tester);
}

Map<String, dynamic> _persisted(SyntheticBridgePort bridge) =>
    jsonDecode(bridge.getSettings().settingsJson) as Map<String, dynamic>;

bool _checkbox(WidgetTester tester, ValueKey<String> key) =>
    tester
        .widget<Checkbox>(
          find.descendant(of: find.byKey(key), matching: find.byType(Checkbox)),
        )
        .value ??
    false;

void main() {
  group('FLD-CFG-059 KeepOlderDedupl entry', () {
    testWidgets('勾选 → 保存 → 重开保持', (tester) async {
      final bridge = SyntheticBridgePort();
      await _pumpOption(tester, _container(bridge));
      await _showDisplay(tester);

      expect(
        _checkbox(tester, const ValueKey('settings-keep-older-dedupl')),
        isFalse,
      );
      await tester.ensureVisible(
        find.byKey(const ValueKey('settings-keep-older-dedupl')),
      );
      await tester.pumpAndSettle();
      await tester.tap(
        find.byKey(const ValueKey('settings-keep-older-dedupl')),
      );
      await tester.pump();
      expect(
        _checkbox(tester, const ValueKey('settings-keep-older-dedupl')),
        isTrue,
      );

      await tester.tap(find.byKey(const ValueKey('settings-save')));
      await tester.pumpAndSettle();
      expect(find.byType(OptionSettingWindow), findsNothing);
      expect(
        (_persisted(bridge)['GuiItem']
            as Map<String, dynamic>)['KeepOlderDedupl'],
        isTrue,
      );

      await _reopen(tester, bridge);
      expect(
        _checkbox(tester, const ValueKey('settings-keep-older-dedupl')),
        isTrue,
      );
      await tester.pumpWidget(const SizedBox());
    });
  });
}
