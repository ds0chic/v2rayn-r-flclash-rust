// UC1-01..04: DNS window UX3 copy (help lines, fold states, preset note).
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/routing/dns_window.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

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

Future<void> _pumpDns(WidgetTester tester, ProviderContainer container) async {
  tester.view.physicalSize = const Size(1280, 900);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(
        home: Scaffold(
          body: Builder(
            builder: (context) => Consumer(
              builder: (context, ref, _) => TextButton(
                onPressed: () => showDnsSettingWindow(context, ref),
                child: const Text('open-dns'),
              ),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.text('open-dns'));
  await tester.pumpAndSettle();
  expect(find.byType(DnsSettingWindow), findsOneWidget);
}

void main() {
  testWidgets('basic tab shows help for all four fields and eight switches', (
    tester,
  ) async {
    await _pumpDns(tester, _container());
    for (final needle in <String>[
      '119.29.29.29',
      'Cloudflare DoH',
      '带路',
      '198.18.0.0/15',
      '14 个',
      'hosts 文件',
      '假 IP',
      '绑定查询',
      'AAAA',
      '并行',
      'ServeStale',
      'HappyEyeballs',
    ]) {
      expect(find.textContaining(needle), findsWidgets, reason: needle);
    }
  });

  testWidgets('advanced tab shows strategy/hosts/expected-ips help', (
    tester,
  ) async {
    await _pumpDns(tester, _container());
    await tester.tap(find.text('高级 DNS'));
    await tester.pumpAndSettle();
    for (final needle in <String>['跟随', '验货', '跳过']) {
      expect(find.textContaining(needle), findsWidgets, reason: needle);
    }
  });

  testWidgets('custom tabs warn about mutual exclusion and json risk', (
    tester,
  ) async {
    await _pumpDns(tester, _container());
    await tester.tap(find.text('自定义 DNS (Xray)'));
    await tester.pumpAndSettle();
    expect(find.textContaining('顶掉'), findsWidgets);
    expect(find.textContaining('先导出备份再改'), findsOneWidget);
  });

  testWidgets('preset row carries an offline-behavior note', (tester) async {
    await _pumpDns(tester, _container());
    expect(find.textContaining('离线只应用本地部分'), findsOneWidget);
  });

  testWidgets('pending-tagged copy items are all marked', (tester) async {
    await _pumpDns(tester, _container());
    expect(find.textContaining('待核实'), findsWidgets);
    await tester.tap(find.text('高级 DNS'));
    await tester.pumpAndSettle();
    expect(find.textContaining('待核实'), findsWidgets);
  });
}
