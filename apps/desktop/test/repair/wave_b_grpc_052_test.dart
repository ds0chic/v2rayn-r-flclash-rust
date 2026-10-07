// Wave B (FLD-CFG-052/053/054/055): GrpcItem entry editor.
//
// 原版对照（v2rayN 7.25.4 / 7d6a967，只读 work/）：
// - `ServiceLib/Models/Configs/ConfigItems.cs:57-64` `GrpcItem`
//   （`int? IdleTimeout / int? HealthCheckTimeout / bool? PermitWithoutStream /
//   int? InitialWindowsSize`，全可空）；
// - `ServiceLib/Handler/ConfigHandler.cs:83-88` 缺省 60 / 20 / false / 0；
// - `V2rayOutboundService.cs:495-498` gRPC transport 装配
//   idle/health/permit/windows；
// - `OptionSettingWindow.xaml` 无 gRPC 区（编辑器为本轮新增，归入“历史保留”，
//   不冒充原版控件）。
//
// 本文件合成数据 + SyntheticBridgePort/SyntheticRuntimeBridge（无 socket、
// 无代理、无 10808 操作），断言正式入口→保存→重开（数据层；真实 gRPC
// session 不跑，记为缺口）。
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
            key: const ValueKey('waveb-grpc-open'),
            onPressed: () => OptionSettingWindow.show(context),
            child: const Text('open'),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.byKey(const ValueKey('waveb-grpc-open')));
  await tester.pumpAndSettle();
  expect(find.byType(OptionSettingWindow), findsOneWidget);
}

Future<void> _reopen(WidgetTester tester, SyntheticBridgePort bridge) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pumpAndSettle();
  await _pumpOption(tester, _container(bridge));
}

Map<String, dynamic> _persisted(SyntheticBridgePort bridge) =>
    jsonDecode(bridge.getSettings().settingsJson) as Map<String, dynamic>;

Map<String, dynamic> _grpc(Map<String, dynamic> doc) =>
    doc['GrpcItem'] as Map<String, dynamic>;

Finder _field(ValueKey<String> key) =>
    find.descendant(of: find.byKey(key), matching: find.byType(TextField));

Future<void> _showGrpc(WidgetTester tester) async {
  await tester.ensureVisible(
    find.byKey(const ValueKey('settings-grpc-idle-timeout')),
  );
  await tester.pumpAndSettle();
}

void main() {
  group('FLD-CFG-052..055 gRPC entry', () {
    testWidgets('缺省渲染 60/20/false/0', (tester) async {
      final bridge = SyntheticBridgePort();
      await _pumpOption(tester, _container(bridge));
      await _showGrpc(tester);

      expect(
        tester
            .widget<TextField>(
              _field(const ValueKey('settings-grpc-idle-timeout')),
            )
            .controller
            ?.text,
        '60',
      );
      expect(
        tester
            .widget<TextField>(
              _field(const ValueKey('settings-grpc-health-timeout')),
            )
            .controller
            ?.text,
        '20',
      );
      expect(
        tester
            .widget<Checkbox>(
              find.descendant(
                of: find.byKey(
                  const ValueKey('settings-grpc-permit-without-stream'),
                ),
                matching: find.byType(Checkbox),
              ),
            )
            .value,
        isFalse,
      );
      expect(
        tester
            .widget<TextField>(
              _field(const ValueKey('settings-grpc-initial-windows-size')),
            )
            .controller
            ?.text,
        '0',
      );
      await tester.pumpWidget(const SizedBox());
    });

    testWidgets('改四项 → 保存 → 重开一致', (tester) async {
      final bridge = SyntheticBridgePort();
      await _pumpOption(tester, _container(bridge));
      await _showGrpc(tester);

      await tester.enterText(
        _field(const ValueKey('settings-grpc-idle-timeout')),
        '11',
      );
      await tester.enterText(
        _field(const ValueKey('settings-grpc-health-timeout')),
        '7',
      );
      await tester.ensureVisible(
        find.byKey(const ValueKey('settings-grpc-permit-without-stream')),
      );
      await tester.pumpAndSettle();
      await tester.tap(
        find.byKey(const ValueKey('settings-grpc-permit-without-stream')),
      );
      await tester.enterText(
        _field(const ValueKey('settings-grpc-initial-windows-size')),
        '65535',
      );
      await tester.pump();

      await tester.tap(find.byKey(const ValueKey('settings-save')));
      await tester.pumpAndSettle();
      expect(find.byType(OptionSettingWindow), findsNothing);
      final saved = _grpc(_persisted(bridge));
      expect(saved['IdleTimeout'], 11);
      expect(saved['HealthCheckTimeout'], 7);
      expect(saved['PermitWithoutStream'], isTrue);
      expect(saved['InitialWindowsSize'], 65535);

      await _reopen(tester, bridge);
      await _showGrpc(tester);
      expect(
        tester
            .widget<TextField>(
              _field(const ValueKey('settings-grpc-idle-timeout')),
            )
            .controller
            ?.text,
        '11',
      );
      expect(
        tester
            .widget<TextField>(
              _field(const ValueKey('settings-grpc-health-timeout')),
            )
            .controller
            ?.text,
        '7',
      );
      expect(
        tester
            .widget<Checkbox>(
              find.descendant(
                of: find.byKey(
                  const ValueKey('settings-grpc-permit-without-stream'),
                ),
                matching: find.byType(Checkbox),
              ),
            )
            .value,
        isTrue,
      );
      expect(
        tester
            .widget<TextField>(
              _field(const ValueKey('settings-grpc-initial-windows-size')),
            )
            .controller
            ?.text,
        '65535',
      );
      await tester.pumpWidget(const SizedBox());
    });

    testWidgets('清空即缺省 null：落盘 null，重开展示冻结默认 60', (tester) async {
      final bridge = SyntheticBridgePort();
      await _pumpOption(tester, _container(bridge));
      await _showGrpc(tester);

      await tester.enterText(
        _field(const ValueKey('settings-grpc-idle-timeout')),
        '',
      );
      await tester.pump();
      await tester.tap(find.byKey(const ValueKey('settings-save')));
      await tester.pumpAndSettle();

      expect(_grpc(_persisted(bridge))['IdleTimeout'], isNull);

      await _reopen(tester, bridge);
      await _showGrpc(tester);
      expect(
        tester
            .widget<TextField>(
              _field(const ValueKey('settings-grpc-idle-timeout')),
            )
            .controller
            ?.text,
        '60',
      );
      await tester.pumpWidget(const SizedBox());
    });

    testWidgets('非法端口阻止保存：窗口保持打开、可见报错、bridge 未写', (tester) async {
      final bridge = SyntheticBridgePort();
      await _pumpOption(tester, _container(bridge));
      final before = bridge.settingsRevision();

      await tester.enterText(
        find.descendant(
          of: find.byKey(const ValueKey('settings-local-port')),
          matching: find.byType(TextField),
        ),
        '70000',
      );
      await tester.pump();
      await tester.tap(find.byKey(const ValueKey('settings-save')));
      await tester.pumpAndSettle();

      // 可见报错 + 窗口保持打开 + 持久层未动（回滚语义）。
      expect(find.byType(OptionSettingWindow), findsOneWidget);
      expect(bridge.settingsRevision(), before);
      await tester.pumpWidget(const SizedBox());
    });
  });
}
