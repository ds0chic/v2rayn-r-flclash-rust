// Wave B (FLD-CFG-059/060/079/110/112): GuiItem/UiItem/SpeedTestItem rows.
//
// 原版对照（v2rayN 7.25.4 / 7d6a967，只读 work/）：
// - `ConfigItems.cs:67-85` `GUIItem`（`KeepOlderDedupl=false` /
//   `AutoUpdateInterval=0`，0=关闭）；
// - `ConfigItems.cs:88-108` `UIItem.Hide2TrayWhenClose=false`；
// - `v2rayN/Views/MainWindow.xaml.cs:193-197` WPF：X 无条件隐藏到托盘
//   （`Hide2TrayWhenClose` 在 Windows 无 observable effect）；
// - `v2rayN.Desktop/Views/MainWindow.axaml.cs:192-213 + 340-362`：
//   X 取消关闭并隐藏；Linux 且开关关→最小化，否则隐藏；真退出只走菜单确认
//   （`MenuClose_Click`）；独立窗（Sub/Routing/Option）关闭只存状态、不退出；
// - `ConfigItems.cs:157-166` `SpeedTestItem`（`IPAPIUrl` /
//   `SpeedTestPageSize` 可空）；`OptionSettingWindow.xaml:1078` 有 IPAPI 行，
//   无 PageSize 行（112 正式入口未定，本文件只登记缺口、不加控件）。
//
// 本文件合成数据 + SyntheticBridgePort/SyntheticRuntimeBridge（无 socket、
// 无代理、无 10808/注册表/系统代理操作），断言正式入口→保存→重开。
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/desktop_integration.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';
import 'package:v2rayn_desktop/features/settings/settings_window_host.dart';

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

Finder _field(ValueKey<String> key) =>
    find.descendant(of: find.byKey(key), matching: find.byType(TextField));

bool _checkbox(WidgetTester tester, ValueKey<String> key) =>
    tester
        .widget<Checkbox>(
          find.descendant(of: find.byKey(key), matching: find.byType(Checkbox)),
        )
        .value ??
    false;

/// 独立窗口替身：只记调用，不触 Rust、不退出。
class _FakeEditorHost implements SettingsEditorHost {
  Map<String, dynamic>? savedDraft;
  int saveCalls = 0;
  int closeCalls = 0;

  @override
  Future<Map<String, dynamic>> loadSnapshot() async => defaultSettingsJson();

  @override
  Future<SettingsEditorOutcome> save(Map<String, dynamic> draft) async {
    saveCalls++;
    savedDraft = draft;
    return const SettingsEditorOutcome(ok: true);
  }

  @override
  Future<void> close() async {
    closeCalls++;
  }
}

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

  group('FLD-CFG-060 AutoUpdateInterval entry', () {
    testWidgets('填 6 小时 → 保存 → 重开一致', (tester) async {
      final bridge = SyntheticBridgePort();
      await _pumpOption(tester, _container(bridge));
      await _showDisplay(tester);

      await tester.ensureVisible(
        find.byKey(const ValueKey('settings-auto-update-interval')),
      );
      await tester.pumpAndSettle();
      expect(
        tester
            .widget<TextField>(
              _field(const ValueKey('settings-auto-update-interval')),
            )
            .controller
            ?.text,
        '0',
      );
      await tester.enterText(
        _field(const ValueKey('settings-auto-update-interval')),
        '6',
      );
      await tester.pump();

      await tester.tap(find.byKey(const ValueKey('settings-save')));
      await tester.pumpAndSettle();
      expect(
        (_persisted(bridge)['GuiItem']
            as Map<String, dynamic>)['AutoUpdateInterval'],
        6,
      );

      await _reopen(tester, bridge);
      expect(
        tester
            .widget<TextField>(
              _field(const ValueKey('settings-auto-update-interval')),
            )
            .controller
            ?.text,
        '6',
      );
      await tester.pumpWidget(const SizedBox());
    });
  });

  group('FLD-CFG-079 Hide2TrayWhenClose (Windows half)', () {
    test('079 ruling 前置：文档缺省 false；Windows 下 X恒隐藏', () {
      // 缺 UiItem / 缺字段 → 均为 false（冻结默认）。
      expect(
        CloseBehavior.fromDocument(const <String, dynamic>{})
            .hide2TrayWhenClose,
        isFalse,
      );
      expect(
        CloseBehavior.fromDocument(const <String, dynamic>{
          'UiItem': <String, dynamic>{},
        }).hide2TrayWhenClose,
        isFalse,
      );
      // Windows 半：无论开关，X 恒隐藏（WPF `MainWindow_Closing` 无条件
      // `e.Cancel = true` + 隐藏；见证据 079 ruling）。
      expect(
        DesktopIntegration.hideOnClose(
          isWindows: true,
          hide2TrayWhenClose: false,
        ),
        isTrue,
      );
      expect(
        DesktopIntegration.hideOnClose(
          isWindows: true,
          hide2TrayWhenClose: true,
        ),
        isTrue,
      );
      // 非 Windows：跟随开关（Linux 分支；macOS Dock 另案）。
      expect(
        DesktopIntegration.hideOnClose(
          isWindows: false,
          hide2TrayWhenClose: false,
        ),
        isFalse,
      );
      expect(
        DesktopIntegration.hideOnClose(
          isWindows: false,
          hide2TrayWhenClose: true,
        ),
        isTrue,
      );
    });

    testWidgets('开关 → 保存 → 重开一致', (tester) async {
      final bridge = SyntheticBridgePort();
      await _pumpOption(tester, _container(bridge));
      await _showDisplay(tester);

      await tester.ensureVisible(
        find.byKey(const ValueKey('settings-hide2tray')),
      );
      await tester.pumpAndSettle();
      expect(_checkbox(tester, const ValueKey('settings-hide2tray')), isFalse);
      await tester.tap(find.byKey(const ValueKey('settings-hide2tray')));
      await tester.pump();

      await tester.tap(find.byKey(const ValueKey('settings-save')));
      await tester.pumpAndSettle();
      expect(
        (_persisted(bridge)['UiItem']
            as Map<String, dynamic>)['Hide2TrayWhenClose'],
        isTrue,
      );

      await _reopen(tester, bridge);
      expect(_checkbox(tester, const ValueKey('settings-hide2tray')), isTrue);
      await tester.pumpWidget(const SizedBox());
    });

    testWidgets('独立窗取消只关自己：host.close 恰一次、不 save、不退出', (tester) async {
      final host = _FakeEditorHost();
      tester.view.physicalSize = const Size(1200, 900);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final container = ProviderContainer();
      addTearDown(container.dispose);
      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            home: Scaffold(
              body: OptionSettingWindow(host: host, standalone: true),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const ValueKey('settings-cancel')));
      await tester.pumpAndSettle();
      expect(host.closeCalls, 1);
      expect(host.saveCalls, 0);
      expect(host.savedDraft, isNull);
      await tester.pumpWidget(const SizedBox());
    });
  });
}
