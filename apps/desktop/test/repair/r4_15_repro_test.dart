// R4-15 DNS draft-baseline collision repro.
//
// Copied from the audit harness
// docs/evidence/user-flow-audit-2026-10-05/dns-baseline-collision-repro.dart
// (settings-audit.md UFS-05 / plan D18). The first test intentionally fails on
// the pre-fix implementation: the draft baseline was keyed by shared text
// content, so editing `direct` to `remote`'s stored value let the preset
// refresh treat it as untouched and overwrite it. The assertions are NOT
// adjusted to the buggy behavior.
//
// Synthetic only: in-memory bridge / runtime / platform and a plain widget. No
// native library, no kernel, no port, no host proxy/TUN/registry, no user data.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/dns.dart' as d;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/routing/dns_controller.dart';
import 'package:v2rayn_desktop/features/routing/dns_window.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import '../support/fake_platform_bridge.dart';
import '../support/synthetic_runtime_bridge.dart';

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
          body: Consumer(
            builder: (context, ref, _) => TextButton(
              key: const ValueKey('fix08b-open-dns'),
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
  await tester.tap(find.byKey(const ValueKey('fix08b-open-dns')));
  await tester.pumpAndSettle();
  expect(find.byType(DnsSettingWindow), findsOneWidget);
}

Future<void> _tab(WidgetTester tester, String label) async {
  await tester.tap(find.text(label));
  await tester.pumpAndSettle();
}

String _fieldText(WidgetTester tester, String key) {
  final field = tester.widget<TextField>(find.byKey(ValueKey(key)));
  return field.controller?.text ?? '';
}

void main() {
  testWidgets('audit dirty DNS equal to other baseline survives preset', (
    tester,
  ) async {
    final container = _container();
    final dns = container.read(dnsControllerProvider.notifier);
    dns.reload();
    expect(
      dns
          .saveSimple(
            const d.SimpleDnsDto(directDns: '1.1.1.1', remoteDns: '8.8.8.8'),
          )
          .ok,
      isTrue,
    );
    await _pumpDns(tester, container);
    await _openDns(tester);
    await tester.enterText(find.byKey(const ValueKey('dns-direct')), '8.8.8.8');
    await tester.pump();
    expect(_fieldText(tester, 'dns-direct'), '8.8.8.8');
    await tester.tap(find.byKey(const ValueKey('dns-apply-preset')));
    await tester.pumpAndSettle();
    expect(_fieldText(tester, 'dns-direct'), '8.8.8.8');
  });

  testWidgets('import-default previews, save persists, cancel leaves none', (
    tester,
  ) async {
    final container = _container();
    container.read(dnsControllerProvider.notifier).reload();
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;

    await _pumpDns(tester, container);
    await _openDns(tester);
    await _tab(tester, '自定义 DNS (Xray)');
    await tester.tap(find.byKey(const ValueKey('dns-import-v2ray')));
    await tester.pump();
    expect(_fieldText(tester, 'dns-custom-normal'), isNotEmpty);
    // Preview only: nothing persisted yet.
    final xrayBefore = bridge
        .listDns()
        .items
        .firstWhere((e) => e.coreType == CoreType.xray)
        .normalDns;
    expect(xrayBefore, isNull);

    // Save persists the preview.
    await tester.tap(find.byKey(const ValueKey('dns-save')));
    await tester.pumpAndSettle();
    expect(find.byType(DnsSettingWindow), findsNothing);
    final xrayAfter = bridge
        .listDns()
        .items
        .firstWhere((e) => e.coreType == CoreType.xray)
        .normalDns;
    expect(xrayAfter, isNotNull);
    expect(xrayAfter, isNotEmpty);
  });

  testWidgets('preset apply does not discard other-page drafts', (
    tester,
  ) async {
    final container = _container();
    container.read(dnsControllerProvider.notifier).reload();
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;

    await _pumpDns(tester, container);
    await _openDns(tester);
    // Dirty the Xray custom page (a value independent of the preset change).
    await _tab(tester, '自定义 DNS (Xray)');
    await tester.enterText(
      find.byKey(const ValueKey('dns-custom-normal')),
      '{"servers": [{"tag":"draft","type":"tcp","server":"9.9.9.9"}]}',
    );
    await tester.pump();

    // Apply a preset from the shared footer while the draft tab is inactive.
    await tester.tap(find.byKey(const ValueKey('dns-apply-preset')));
    await tester.pumpAndSettle();

    // The dirty draft survived the refresh.
    await _tab(tester, '自定义 DNS (Xray)');
    expect(
      _fieldText(tester, 'dns-custom-normal'),
      '{"servers": [{"tag":"draft","type":"tcp","server":"9.9.9.9"}]}',
    );

    // Cancel discards the draft and persists nothing.
    final before = bridge.listDns().items.map((e) => e.normalDns).toList();
    await tester.tap(find.byKey(const ValueKey('dns-cancel')));
    await tester.pumpAndSettle();
    expect(find.byType(DnsSettingWindow), findsNothing);
    expect(bridge.listDns().items.map((e) => e.normalDns).toList(), before);
  });

  testWidgets('both custom enabled disables the simple DNS area', (
    tester,
  ) async {
    final container = _container();
    container.read(dnsControllerProvider.notifier).reload();
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    bridge.saveDns(
      const d.DnsProfileDto(
        id: 'syn-dns-xray',
        remarks: 'V2ray',
        enabled: true,
        coreType: CoreType.xray,
        useSystemHosts: false,
      ),
    );
    bridge.saveDns(
      const d.DnsProfileDto(
        id: 'syn-dns-sbox',
        remarks: 'sing-box',
        enabled: true,
        coreType: CoreType.singBox,
        useSystemHosts: false,
      ),
    );

    await _pumpDns(tester, container);
    await _openDns(tester);
    expect(
      find.byKey(const ValueKey('dns-simple-disabled-hint')),
      findsOneWidget,
    );
    await tester.tap(find.byKey(const ValueKey('dns-cancel')));
    await tester.pumpAndSettle();
  });

  testWidgets('one custom row enabled keeps simple DNS enabled', (
    tester,
  ) async {
    final container = _container();
    container.read(dnsControllerProvider.notifier).reload();
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    bridge.saveDns(
      const d.DnsProfileDto(
        id: 'syn-dns-xray',
        remarks: 'V2ray',
        enabled: true,
        coreType: CoreType.xray,
        useSystemHosts: false,
      ),
    );
    bridge.saveDns(
      const d.DnsProfileDto(
        id: 'syn-dns-sbox',
        remarks: 'sing-box',
        enabled: false,
        coreType: CoreType.singBox,
        useSystemHosts: false,
      ),
    );

    await _pumpDns(tester, container);
    await _openDns(tester);
    expect(
      find.byKey(const ValueKey('dns-simple-disabled-hint')),
      findsNothing,
    );
  });
}
