// FIX-08 (SET-11): DNS drafts. Import-default only previews (cancel leaves
// storage untouched); Save passes GlobalFakeIp through and validates custom
// texts before the first write; Apply saves the visible draft first.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/dns.dart' as d;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/routing/dns_controller.dart';
import 'package:v2rayn_desktop/features/routing/dns_window.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
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
        // The Scaffold mirrors the production shell so SnackBars present.
        home: Scaffold(
          body: Consumer(
            builder: (context, ref, _) => TextButton(
              key: const ValueKey('fix08-open-dns'),
              onPressed: () => showDnsSettingWindow(context, ref),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

Future<void> _openDns(WidgetTester tester) async {
  await tester.tap(find.byKey(const ValueKey('fix08-open-dns')));
  await tester.pumpAndSettle();
  expect(find.byType(DnsSettingWindow), findsOneWidget);
}

Future<void> _openXrayTab(WidgetTester tester) async {
  await tester.tap(find.text('自定义 DNS (Xray)'));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('dns import-default previews only; cancel persists nothing', (
    tester,
  ) async {
    final container = _container();
    final dns = container.read(dnsControllerProvider.notifier);
    dns.reload();
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    final before = bridge.listDns().items.map((e) => e.normalDns).toList();

    await _pumpDns(tester, container);
    await _openDns(tester);
    await _openXrayTab(tester);
    await tester.tap(find.byKey(const ValueKey('dns-import-v2ray')));
    await tester.pump();
    // The preview filled the tab fields...
    final normalField = tester.widget<TextField>(
      find.byKey(const ValueKey('dns-custom-normal')),
    );
    expect(normalField.controller?.text ?? '', isNotEmpty);
    // ...but nothing was persisted yet.
    final during = bridge.listDns().items.map((e) => e.normalDns).toList();
    expect(during, before);

    await tester.tap(find.byKey(const ValueKey('dns-cancel')));
    await tester.pumpAndSettle();
    expect(find.byType(DnsSettingWindow), findsNothing);
    final after = bridge.listDns().items.map((e) => e.normalDns).toList();
    expect(after, before);
  });

  testWidgets(
    'dns save keeps GlobalFakeIp; bad custom text blocks first save',
    (tester) async {
      final container = _container();
      final dns = container.read(dnsControllerProvider.notifier);
      dns.reload();
      // Stored explicit false with a direct DNS value.
      final seeded = dns.saveSimple(
        const d.SimpleDnsDto(directDns: '9.9.9.9', globalFakeIp: false),
      );
      expect(seeded.ok, isTrue);

      await _pumpDns(tester, container);
      await _openDns(tester);
      await tester.enterText(
        find.byKey(const ValueKey('dns-direct')),
        '119.29.29.29',
      );
      await tester.pump();
      await tester.tap(find.byKey(const ValueKey('dns-save')));
      await tester.pumpAndSettle();

      expect(find.byType(DnsSettingWindow), findsNothing);
      final simple = container.read(dnsControllerProvider).simple;
      expect(simple?.directDns, '119.29.29.29');
      // The window has no GlobalFakeIp control: the stored value survived an
      // unrelated edit instead of being cleared.
      expect(simple?.globalFakeIp, false);

      // Reopen and try to save an invalid Xray custom text: no stage may be
      // written and the window must stay open with an error.
      await _openDns(tester);
      await _openXrayTab(tester);
      await tester.enterText(
        find.byKey(const ValueKey('dns-custom-normal')),
        '{"hosts": {}}',
      );
      await tester.pump();
      final revisionBefore = (container.read(
        bridgePortProvider,
      ) as SyntheticBridgePort).loadSimpleDns().revision;
      await tester.tap(find.byKey(const ValueKey('dns-save')));
      await tester.pump();
      expect(find.byType(DnsSettingWindow), findsOneWidget);
      expect(find.text('请填写正确的 DNS 文本'), findsOneWidget);
      final revisionAfter = (container.read(
        bridgePortProvider,
      ) as SyntheticBridgePort).loadSimpleDns().revision;
      expect(revisionAfter, revisionBefore);
    },
  );

  testWidgets('dns apply saves the visible draft before applying', (
    tester,
  ) async {
    final container = _container();
    container.read(dnsControllerProvider.notifier).reload();
    await _pumpDns(tester, container);
    await _openDns(tester);
    await tester.enterText(
      find.byKey(const ValueKey('dns-direct')),
      '119.29.29.29',
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('dns-apply')));
    await tester.pumpAndSettle();

    expect(find.byType(DnsSettingWindow), findsNothing);
    expect(
      container.read(dnsControllerProvider).simple?.directDns,
      '119.29.29.29',
    );
    expect(container.read(runtimeControllerProvider).state, 'Running');
  });
}
