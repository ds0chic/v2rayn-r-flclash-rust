// R4-15 DNS draft protection + upstream reload contract.
//
// Contract (docs/repair/tasks/R4-15.md):
//  - A successful save triggers the upstream `Reload` even when the optional
//    project-specific 应用 was not pressed (D17); a failed save changes no
//    in-memory/persisted DNS state and does not reload.
//  - The draft baseline is keyed by field identity, not by shared text, so
//    editing one field to another field's stored value survives a preset
//    refresh (D18); cancel persists nothing.
//
// Synthetic only: in-memory bridge / runtime / platform and a plain widget. No
// native library, no kernel, no port, no host proxy/TUN/registry, no user data.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
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

/// A bridge whose SimpleDNS write always fails, to drive the save-error branch
/// without touching persisted state.
class _FailingSimpleDnsBridge extends SyntheticBridgePort {
  @override
  d.SimpleDnsDtoResult saveSimpleDns(d.SimpleDnsDto draft, int revision) {
    return d.SimpleDnsDtoResult(
      ok: false,
      revision: BigInt.from(revision),
      error: const c.ErrorDto(
        code: 'E_IO',
        messageKey: 'error.io',
        retryable: true,
      ),
    );
  }
}

ProviderContainer _container({BridgePort? bridge}) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge ?? SyntheticBridgePort()),
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
              key: const ValueKey('r4-15-open-dns'),
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
  await tester.tap(find.byKey(const ValueKey('r4-15-open-dns')));
  await tester.pumpAndSettle();
  expect(find.byType(DnsSettingWindow), findsOneWidget);
}

String _fieldText(WidgetTester tester, String key) {
  final field = tester.widget<TextField>(find.byKey(ValueKey(key)));
  return field.controller?.text ?? '';
}

void main() {
  testWidgets(
    'R4-15/D18: draft equal to another field stored value survives preset',
    (tester) async {
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
      await tester.enterText(
        find.byKey(const ValueKey('dns-direct')),
        '8.8.8.8',
      );
      await tester.pump();
      await tester.tap(find.byKey(const ValueKey('dns-apply-preset')));
      await tester.pumpAndSettle();

      expect(_fieldText(tester, 'dns-direct'), '8.8.8.8');
      // The untouched field still mirrors storage.
      expect(_fieldText(tester, 'dns-remote'), '8.8.8.8');
    },
  );

  testWidgets(
    'R4-15/D17: normal save (applyAfter=false) triggers the upstream reload',
    (tester) async {
      final container = _container();
      container.read(dnsControllerProvider.notifier).reload();
      await _pumpDns(tester, container);
      await _openDns(tester);
      await tester.enterText(
        find.byKey(const ValueKey('dns-direct')),
        '119.29.29.29',
      );
      await tester.pump();
      expect(container.read(runtimeControllerProvider).state, isNot('Running'));

      await tester.tap(find.byKey(const ValueKey('dns-save')));
      await tester.pumpAndSettle();

      expect(find.byType(DnsSettingWindow), findsNothing);
      expect(container.read(runtimeControllerProvider).state, 'Running');
    },
  );

  testWidgets(
    'R4-15: failed save keeps the window open, writes nothing, no reload',
    (tester) async {
      final bridge = _FailingSimpleDnsBridge();
      final container = _container(bridge: bridge);
      container.read(dnsControllerProvider.notifier).reload();
      await _pumpDns(tester, container);
      await _openDns(tester);
      await tester.enterText(
        find.byKey(const ValueKey('dns-direct')),
        '119.29.29.29',
      );
      await tester.pump();

      await tester.tap(find.byKey(const ValueKey('dns-save')));
      await tester.pump();

      expect(find.byType(DnsSettingWindow), findsOneWidget);
      expect(find.textContaining('保存失败'), findsOneWidget);
      expect(container.read(runtimeControllerProvider).state, isNot('Running'));
      expect(bridge.loadSimpleDns().item?.directDns, isNot('119.29.29.29'));
    },
  );

  testWidgets('R4-15: cancel persists nothing and does not reload', (
    tester,
  ) async {
    final container = _container();
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    container.read(dnsControllerProvider.notifier).reload();
    await _pumpDns(tester, container);
    await _openDns(tester);
    await tester.enterText(
      find.byKey(const ValueKey('dns-direct')),
      '119.29.29.29',
    );
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('dns-cancel')));
    await tester.pumpAndSettle();

    expect(find.byType(DnsSettingWindow), findsNothing);
    expect(bridge.loadSimpleDns().item?.directDns, isNot('119.29.29.29'));
    expect(container.read(runtimeControllerProvider).state, isNot('Running'));
  });

  testWidgets(
    'R4-15: a toggled boolean draft survives repeated preset refreshes',
    (tester) async {
      final container = _container();
      container.read(dnsControllerProvider.notifier).reload();
      await _pumpDns(tester, container);
      await _openDns(tester);

      final toggle = find.byKey(const ValueKey('dns-use-system-hosts'));
      await tester.ensureVisible(toggle);
      await tester.tap(toggle);
      await tester.pump();
      expect(tester.widget<Switch>(toggle).value, isTrue);

      for (var i = 0; i < 2; i++) {
        await tester.tap(find.byKey(const ValueKey('dns-apply-preset')));
        await tester.pumpAndSettle();
        expect(tester.widget<Switch>(toggle).value, isTrue);
      }
    },
  );
}
