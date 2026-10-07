// Wave B (FLD-CFG-036/043/044): inbound Protocol entry editor + auth fields.
//
// 原版对照（v2rayN 7.25.4 / 7d6a967，只读 work/）：
// - `ServiceLib/Models/Configs/ConfigItems.cs:26-38` `InItem`（`Protocol` 无
//   初始化器，`User`/`Pass` 非空字符串）；
// - `ServiceLib/Handler/ConfigHandler.cs:49` 新建入站 `Protocol = "socks"`，
//   `:62` 非空列表强制 `First().Protocol = "socks"`；
// - `ServiceLib/Enums/EInboundProtocol.cs` 冻结值
//   socks=0/socks2=1/socks3=2/pac=3/api=4/api2=5/mixed=6/speedtest=21；
// - `V2rayInboundService.BuildInbound` 按身份取端口偏移、emit 硬编码 `mixed`，
//   LAN 认证仅 `User && Pass` 均非空时装配 accounts；
// - `v2rayN/Views/OptionSettingWindow.xaml` 入站区无 Protocol 行（036 编辑器
//   为本轮新增，不冒充原版控件），User/Pass 行无条件渲染、可用性受
//   `togNewPort4LAN` 门控。
//
// 本文件合成数据 + SyntheticBridgePort/SyntheticRuntimeBridge（无 socket、
// 无代理、无 10808 操作），断言正式入口→保存→重开（数据层；live-core 不跑）。
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
            key: const ValueKey('waveb-open-settings'),
            onPressed: () => OptionSettingWindow.show(context),
            child: const Text('open'),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.byKey(const ValueKey('waveb-open-settings')));
  await tester.pumpAndSettle();
  expect(find.byType(OptionSettingWindow), findsOneWidget);
}

/// 独立重开：同一持久 bridge + 新 container，重建窗口。
Future<ProviderContainer> _reopen(
  WidgetTester tester,
  SyntheticBridgePort bridge,
) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pumpAndSettle();
  final container = _container(bridge);
  await _pumpOption(tester, container);
  return container;
}

Map<String, dynamic> _persisted(SyntheticBridgePort bridge) =>
    jsonDecode(bridge.getSettings().settingsJson) as Map<String, dynamic>;

Map<String, dynamic> _inboundRow(Map<String, dynamic> doc) =>
    (doc['Inbound'] as List<dynamic>).first as Map<String, dynamic>;

Finder _field(ValueKey<String> key) =>
    find.descendant(of: find.byKey(key), matching: find.byType(TextField));

int? _protocolValue(WidgetTester tester) => tester
    .widget<DropdownButton<int>>(
      find.descendant(
        of: find.byKey(const ValueKey('settings-inbound-protocol')),
        matching: find.byType(DropdownButton<int>),
      ),
    )
    .value;

void main() {
  group('FLD-CFG-036 Inbound Protocol entry', () {
    testWidgets('选择 mixed → 保存 → 重开显示 mixed（6）', (tester) async {
      final bridge = SyntheticBridgePort();
      final container = _container(bridge);
      await _pumpOption(tester, container);
      expect(_protocolValue(tester), 0);

      await tester.tap(find.byKey(const ValueKey('settings-inbound-protocol')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('mixed').last);
      await tester.pumpAndSettle();
      expect(_protocolValue(tester), 6);

      await tester.tap(find.byKey(const ValueKey('settings-save')));
      await tester.pumpAndSettle();
      expect(find.byType(OptionSettingWindow), findsNothing);
      expect(_inboundRow(_persisted(bridge))['Protocol'], 6);

      await _reopen(tester, bridge);
      expect(_protocolValue(tester), 6);
      expect(_inboundRow(_persisted(bridge))['Protocol'], 6);
    });

    testWidgets('未知值不静默映射：99 保存后仍是 99，下拉显示未选', (tester) async {
      final bridge = SyntheticBridgePort();
      final doc = _persisted(bridge);
      _inboundRow(doc)['Protocol'] = 99;
      bridge.saveSettingsJson(jsonEncode(doc), 0);

      final container = _container(bridge);
      await _pumpOption(tester, container);
      // 未知值不在冻结集合中：显示未选，但不崩溃、不改写。
      expect(_protocolValue(tester), isNull);

      await tester.tap(find.byKey(const ValueKey('settings-save')));
      await tester.pumpAndSettle();
      expect(_inboundRow(_persisted(bridge))['Protocol'], 99);
    });
  });

  group('FLD-CFG-043/044 inbound auth entry', () {
    testWidgets('门控关闭时认证框禁用；开启后填值 → 保存 → 重开一致', (tester) async {
      final bridge = SyntheticBridgePort();
      final container = _container(bridge);
      await _pumpOption(tester, container);

      // 门控关闭：User/Pass 禁用（灰显不丢值）。
      expect(
        tester
            .widget<TextField>(_field(const ValueKey('settings-inbound-user')))
            .enabled,
        isFalse,
      );
      expect(
        tester
            .widget<TextField>(_field(const ValueKey('settings-inbound-pass')))
            .enabled,
        isFalse,
      );

      await tester.tap(find.byKey(const ValueKey('settings-newport4lan')));
      await tester.pump();
      expect(
        tester
            .widget<TextField>(_field(const ValueKey('settings-inbound-user')))
            .enabled,
        isTrue,
      );

      // 合成凭据（非秘密、仅夹具；永不进日志/证据正文）。
      await tester.enterText(
        _field(const ValueKey('settings-inbound-user')),
        'syn-user',
      );
      await tester.enterText(
        _field(const ValueKey('settings-inbound-pass')),
        'syn-pass',
      );
      await tester.pump();

      await tester.tap(find.byKey(const ValueKey('settings-save')));
      await tester.pumpAndSettle();
      expect(find.byType(OptionSettingWindow), findsNothing);
      final saved = _inboundRow(_persisted(bridge));
      expect(saved['NewPort4LAN'], isTrue);
      expect(saved['User'], 'syn-user');
      expect(saved['Pass'], 'syn-pass');

      await _reopen(tester, bridge);
      expect(
        tester
            .widget<TextField>(_field(const ValueKey('settings-inbound-user')))
            .enabled,
        isTrue,
      );
      expect(
        tester
            .widget<TextField>(_field(const ValueKey('settings-inbound-user')))
            .controller
            ?.text,
        'syn-user',
      );
      expect(
        tester
            .widget<TextField>(_field(const ValueKey('settings-inbound-pass')))
            .controller
            ?.text,
        'syn-pass',
      );
    });

    testWidgets('禁用只灰显不丢值：已有 User/Pass 在门控关闭时仍保存', (tester) async {
      final bridge = SyntheticBridgePort();
      final doc = _persisted(bridge);
      final row = _inboundRow(doc);
      row['NewPort4LAN'] = false;
      row['User'] = 'kept-user';
      row['Pass'] = 'kept-pass';
      bridge.saveSettingsJson(jsonEncode(doc), 0);

      final container = _container(bridge);
      await _pumpOption(tester, container);
      expect(
        tester
            .widget<TextField>(_field(const ValueKey('settings-inbound-user')))
            .controller
            ?.text,
        'kept-user',
      );

      await tester.tap(find.byKey(const ValueKey('settings-save')));
      await tester.pumpAndSettle();
      final saved = _inboundRow(_persisted(bridge));
      expect(saved['User'], 'kept-user');
      expect(saved['Pass'], 'kept-pass');
    });
  });
}
