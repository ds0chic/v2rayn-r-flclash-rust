// Wave B (FLD-CFG-110/112): SpeedTestItem rows.
//
// 原版对照（v2rayN 7.25.4 / 7d6a967，只读 work/）：
// - `ConfigItems.cs:157-166` `SpeedTestItem`（`IPAPIUrl` /
//   `SpeedTestPageSize` 可空，缺省 null）；
// - `OptionSettingWindow.xaml:1078` 有 `cmbIPAPIUrl` 行（110 正式入口在本窗，
//   已接线）；无 PageSize 行（112 正式入口未定，见缺口登记）；
// - `crates/application/src/speedtest.rs:419` 空串过滤（trim 语义）。
//
// 本文件合成数据 + SyntheticBridgePort/SyntheticRuntimeBridge（无 socket、
// 无代理、无 10808 操作、无真实 IP 请求），断言正式入口→保存→重开。
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
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

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
            key: const ValueKey('waveb-speed-open'),
            onPressed: () => OptionSettingWindow.show(context),
            child: const Text('open'),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.byKey(const ValueKey('waveb-speed-open')));
  await tester.pumpAndSettle();
  expect(find.byType(OptionSettingWindow), findsOneWidget);
}

/// 切到“显示”页（第 2 个 Tab，测速区所在）。
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

Finder _field(ValueKey<String> key) =>
    find.descendant(of: find.byKey(key), matching: find.byType(TextField));

void main() {
  group('FLD-CFG-110 IPAPIUrl entry', () {
    testWidgets('填 URL → 保存 → 重开一致；清空落盘 null', (tester) async {
      final bridge = SyntheticBridgePort();
      await _pumpOption(tester, _container(bridge));
      await _showDisplay(tester);

      await tester.ensureVisible(
        find.byKey(const ValueKey('settings-ipapi-url')),
      );
      await tester.pumpAndSettle();
      await tester.enterText(
        _field(const ValueKey('settings-ipapi-url')),
        'https://syn-ip.example/json',
      );
      await tester.pump();
      await tester.tap(find.byKey(const ValueKey('settings-save')));
      await tester.pumpAndSettle();
      expect(find.byType(OptionSettingWindow), findsNothing);
      expect(
        (_persisted(bridge)['SpeedTestItem']
            as Map<String, dynamic>)['IPAPIUrl'],
        'https://syn-ip.example/json',
      );

      await _reopen(tester, bridge);
      expect(
        tester
            .widget<TextField>(_field(const ValueKey('settings-ipapi-url')))
            .controller
            ?.text,
        'https://syn-ip.example/json',
      );

      // 清空 → null 落盘（空串过滤语义对齐 `speedtest.rs:419` trim）。
      await tester.enterText(_field(const ValueKey('settings-ipapi-url')), '');
      await tester.pump();
      await tester.tap(find.byKey(const ValueKey('settings-save')));
      await tester.pumpAndSettle();
      expect(
        (_persisted(bridge)['SpeedTestItem']
            as Map<String, dynamic>)['IPAPIUrl'],
        isNull,
      );
      await tester.pumpWidget(const SizedBox());
    });
  });

  group('FLD-CFG-112 SpeedTestPageSize gap', () {
    test('正式入口未定：本窗无分页编辑器，缺省 null，登记缺口不加控件', () {
      // FLD-CFG-112 允许模块仅 Rust（speedtest.rs/bridge settings.rs），
      // 上游 OptionSettingWindow 亦无对应行：本窗不得私加控件。
      // 正确文件/缺口见 SP-24 证据（消费方
      // `profiles_controller.dart:661` 已读值，正式编辑入口未定）。
      final speed =
          defaultSettingsJson()['SpeedTestItem'] as Map<String, dynamic>;
      expect(speed['SpeedTestPageSize'], isNull);
    });
  });
}
