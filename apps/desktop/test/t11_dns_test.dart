// T11: DNS controller (CRUD, import-default, SimpleDNS, presets) plus the
// DNS settings window render/cancel through the synthetic bridge.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/dns.dart' as d;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/routing/dns_controller.dart';
import 'package:v2rayn_desktop/features/routing/dns_window.dart';

import 'support/profiles_harness.dart';

void main() {
  test('dns controller seeds one row per core', () {
    final container = makeContainer();
    addTearDown(container.dispose);
    container.read(dnsControllerProvider.notifier).reload();
    final state = container.read(dnsControllerProvider);
    expect(state.items, hasLength(2));
    expect(state.forCore(CoreType.xray), isNotNull);
    expect(state.forCore(CoreType.singBox), isNotNull);
  });

  test('dns save rejects empty remarks', () {
    final container = makeContainer();
    addTearDown(container.dispose);
    final controller = container.read(dnsControllerProvider.notifier);
    final result = controller.save(
      const d.DnsProfileDto(
        id: '',
        remarks: '',
        enabled: false,
        coreType: CoreType.xray,
        useSystemHosts: false,
      ),
    );
    expect(result.ok, isFalse);
    expect(result.error?.fieldPath, 'remarks');
  });

  test('dns import-default fills the embedded template text', () {
    final container = makeContainer();
    addTearDown(container.dispose);
    final controller = container.read(dnsControllerProvider.notifier);
    controller.reload();
    final filled = controller.importDefault(CoreType.singBox);
    expect(filled.ok, isTrue);
    expect(filled.item?.normalDns, isNotEmpty);
    expect(filled.item?.tunDns, isNotEmpty);
  });

  test('dns simple round-trips and presets report pending offline', () {
    final container = makeContainer();
    addTearDown(container.dispose);
    final controller = container.read(dnsControllerProvider.notifier);
    controller.reload();
    final saved = controller.saveSimple(
      const d.SimpleDnsDto(directDns: '119.29.29.29', fakeIp: true),
    );
    expect(saved.ok, isTrue);
    expect(
      container.read(dnsControllerProvider).simple?.directDns,
      '119.29.29.29',
    );
    final preset = controller.applyPreset('Russia');
    expect(preset.ok, isTrue);
    expect(preset.preset, 'Russia');
    final back = controller.applyPreset('Default');
    expect(back.ok, isTrue);
    expect(back.pendingUrls, isEmpty);
  });

  testWidgets('dns window renders four tabs and cancel discards', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 900);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final container = makeContainer();
    addTearDown(container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: DnsSettingWindow())),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('dns-setting-window')), findsOneWidget);
    expect(find.text('基础 DNS'), findsOneWidget);
    expect(find.text('高级 DNS'), findsOneWidget);
    expect(find.text('自定义 DNS (Xray)'), findsOneWidget);
    expect(find.text('自定义 DNS (sing-box)'), findsOneWidget);
    expect(find.byKey(const ValueKey('dns-import-v2ray')), findsNothing);
    // Switch to the Xray tab: the import-default button appears.
    await tester.tap(find.text('自定义 DNS (Xray)'));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('dns-import-v2ray')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('dns-cancel')));
    await tester.pumpAndSettle();
  });
}
