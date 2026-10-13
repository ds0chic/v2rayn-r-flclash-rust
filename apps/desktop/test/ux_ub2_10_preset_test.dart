import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/preset_dialog.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

ProviderContainer _container(SyntheticBridgePort bridge) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
    profileRowCountProvider.overrideWithValue(4),
  ],
);

Future<void> _pumpWindow(
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
      child: const MaterialApp(home: Scaffold(body: OptionSettingWindow())),
    ),
  );
  await tester.pump();
  await tester.pumpAndSettle();
}

Map<String, dynamic> _persisted(SyntheticBridgePort bridge) =>
    jsonDecode(bridge.getSettings().settingsJson) as Map<String, dynamic>;

/// Seed deviations from the scenario-A table so the preview has real diffs.
/// (Synthetic values only; never real credentials or subscription URLs.)
Map<String, dynamic> _seedDeviations() {
  final seed = defaultSettingsJson();
  (seed['SimpleDNSItem'] as Map<String, dynamic>)['RemoteDNS'] = '8.8.8.8';
  ((seed['Inbound'] as List<dynamic>).first
          as Map<String, dynamic>)['SniffingEnabled'] =
      false;
  (seed['SystemProxyItem'] as Map<String, dynamic>)['SysProxyType'] = 1;
  (seed['TunModeItem'] as Map<String, dynamic>)['EnableTun'] = true;
  return seed;
}

Future<void> _openDialog(WidgetTester tester) async {
  await tester.tap(find.byKey(const ValueKey('settings-preset-open')));
  await tester.pumpAndSettle();
}

Future<void> _pickScenario(WidgetTester tester, String id) async {
  await tester.tap(find.byKey(ValueKey('settings-preset-scenario-$id')));
  await tester.pumpAndSettle();
}

void main() {
  test('UB2-10: scenario tables cover A/B/C/D with no forbidden writes', () {
    final scenarios = presetScenarios();
    expect(scenarios.map((s) => s.id).toList(), ['A', 'B', 'C', 'D']);
    for (final s in scenarios) {
      expect(
        s.manualSteps,
        isNotEmpty,
        reason: '${s.id} must list manual steps',
      );
      for (final patch in s.writes.values) {
        expect(patch.containsKey('SysProxyType'), isFalse);
        expect(patch.containsKey('LocalPort'), isFalse);
        expect(patch.containsKey('User'), isFalse);
        expect(patch.containsKey('Pass'), isFalse);
      }
    }
    // Condition E: TUN is never turned on by any preset.
    for (final s in scenarios) {
      final tun = s.writes['TunModeItem'];
      if (tun != null && tun.containsKey('EnableTun')) {
        expect(tun['EnableTun'], isFalse);
      }
    }
  });

  testWidgets('UB2-10: toolbar entry opens the dialog with four scenarios', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);

    await _openDialog(tester);
    // Exact titles: the settings form behind the dialog shares substrings
    // (e.g. the UdpEnabled help mentions 游戏）, so never match loosely here.
    expect(find.text('日常浏览 · 绕过大陆（默认推荐）'), findsOneWidget);
    expect(find.text('全局代理'), findsOneWidget);
    expect(find.text('游戏 / 低延迟'), findsOneWidget);
    expect(find.text('仅浏览器 · 不改系统'), findsOneWidget);
  });

  testWidgets('UB2-10: scenario A preview shows real diffs, never sysproxy', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 4);
    bridge.saveSettingsJson(jsonEncode(_seedDeviations()), 0);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);

    await _openDialog(tester);
    await _pickScenario(tester, 'A');

    expect(
      find.byKey(
        const ValueKey('settings-preset-diff-SimpleDNSItem.RemoteDNS'),
      ),
      findsOneWidget,
    );
    expect(
      find.byKey(
        const ValueKey('settings-preset-diff-Inbound.SniffingEnabled'),
      ),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey('settings-preset-diff-TunModeItem.EnableTun')),
      findsOneWidget,
    );
    // SysProxyType is a manual step, never a diff row.
    expect(
      find.byKey(
        const ValueKey('settings-preset-diff-SystemProxyItem.SysProxyType'),
      ),
      findsNothing,
    );
    expect(find.textContaining('手动'), findsWidgets);
  });

  testWidgets('UB2-10: apply writes diffs, leaves sysproxy/ports/creds alone', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 4);
    bridge.saveSettingsJson(jsonEncode(_seedDeviations()), 0);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);

    await _openDialog(tester);
    await _pickScenario(tester, 'A');
    await tester.tap(find.byKey(const ValueKey('settings-preset-apply')));
    // The apply runs several sequential group saves plus a route pop; an
    // immediate pumpAndSettle never settles here (frames keep arriving while
    // the saves interleave), so drain with bounded pumps first. Probes showed
    // the writes + pop fully complete; only the settle strategy matters.
    for (var i = 0; i < 30; i++) {
      await tester.pump(const Duration(milliseconds: 100));
    }
    await tester.pumpAndSettle();

    final saved = _persisted(bridge);
    expect(
      (saved['SimpleDNSItem'] as Map<String, dynamic>)['RemoteDNS'],
      'https://cloudflare-dns.com/dns-query',
    );
    expect(
      ((saved['Inbound'] as List<dynamic>).first
          as Map<String, dynamic>)['SniffingEnabled'],
      isTrue,
    );
    // TUN was on: the scenario table says off, preview showed it, now off.
    expect(
      (saved['TunModeItem'] as Map<String, dynamic>)['EnableTun'],
      isFalse,
    );
    // Never auto-written: sysproxy type, main port, LAN credentials.
    expect(
      (saved['SystemProxyItem'] as Map<String, dynamic>)['SysProxyType'],
      1,
    );
    expect(
      ((saved['Inbound'] as List<dynamic>).first
          as Map<String, dynamic>)['LocalPort'],
      10808,
    );
    expect(
      ((saved['Inbound'] as List<dynamic>).first
          as Map<String, dynamic>)['User'],
      '',
    );
    expect(find.textContaining('已应用'), findsOneWidget);
  });

  testWidgets('UB2-10: undo stays disabled with the UX3-12 snapshot note', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);

    await _openDialog(tester);
    await _pickScenario(tester, 'B');
    final undo = tester.widget<ButtonStyleButton>(
      find.byKey(const ValueKey('settings-preset-undo')),
    );
    expect(undo.enabled, isFalse);
    expect(find.textContaining('UX3-12'), findsOneWidget);
  });
}
