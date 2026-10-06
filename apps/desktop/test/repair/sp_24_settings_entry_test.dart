// SP-24 settings entries: Happy Eyeballs enabled gate, MaxSplit `1-3`
// range form, and fragment/TUN comma-list split parity.
//
// Upstream truth (`7d6a967`):
// - `OptionSettingViewModel.SaveSettingAsync` rejects the draft only when
//   `FragmentMaxSplit` is non-empty and `Utils.TryParseMaxSplit(_, 0, 10000)`
//   fails (single int or `from-to` range); `1-3` must save.
// - `V2rayDnsService.FillSockoptDomainStrategy` emits the `happyEyeballs`
//   block only while `SimpleDNSItem.EnableHappyEyeballs` is on, so the
//   retained parameter editors follow the same gate (values are kept).
// - `Utils.String2List` drops zero-length entries, so a trailing comma
//   stores no phantom item.
//
// Synthetic fixtures only; no ports, no OS writes.
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';

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
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: OptionSettingWindow())),
    ),
  );
  await tester.pump();
  await tester.pumpAndSettle();
}

Map<String, dynamic> _saved(SyntheticBridgePort bridge) =>
    jsonDecode(bridge.getSettings().settingsJson) as Map<String, dynamic>;

Finder _field(String key) => find.descendant(
  of: find.byKey(ValueKey(key)),
  matching: find.byType(TextField),
);

Future<void> _enterKeyed(WidgetTester tester, String key, String text) async {
  await tester.ensureVisible(_field(key));
  await tester.pumpAndSettle();
  await tester.enterText(_field(key), text);
  await tester.pump();
}

Future<void> _save(WidgetTester tester) async {
  await tester.ensureVisible(find.byKey(const ValueKey('settings-save')));
  await tester.pumpAndSettle();
  await tester.tap(find.byKey(const ValueKey('settings-save')));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('SP-24: MaxSplit range form 1-3 saves and survives reopen', (
    tester,
  ) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);

    await _pumpWindow(tester, container);
    await _enterKeyed(tester, 'fragment-maxsplit', '1-3');
    await _save(tester);

    expect(bridge.settingsRevision(), 1);
    final doc = _saved(bridge);
    expect(
      (doc['Fragment4RayItem'] as Map)['MaxSplit'],
      '1-3',
      reason: 'raw range text must be preserved, not normalised to 1',
    );
  });

  testWidgets('SP-24: MaxSplit rejects reversed/non-numeric ranges', (
    tester,
  ) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);

    await _pumpWindow(tester, container);
    // A rejected save keeps the window open, so all bad forms run against
    // one pump without re-mounting the window.
    for (final bad in <String>['3-1', 'abc', '1-2-3', '0-10001']) {
      await _enterKeyed(tester, 'fragment-maxsplit', bad);
      await _save(tester);

      expect(
        bridge.settingsRevision(),
        0,
        reason: 'invalid MaxSplit $bad must block the save',
      );
      // The failed save keeps the window open with the save action intact.
      expect(find.byKey(const ValueKey('settings-save')), findsOneWidget);
    }
  });

  testWidgets('SP-24: Happy parameters follow the enabled gate', (
    tester,
  ) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);

    await _pumpWindow(tester, container);
    await tester.tap(find.text('v2rayN 设置'));
    await tester.pumpAndSettle();

    // Gate off by default: the retained parameter editors stay hidden.
    await tester.ensureVisible(
      find.byKey(const ValueKey('happy-eyeballs-toggle')),
    );
    await tester.pumpAndSettle();
    expect(find.text('尝试延迟'), findsNothing);

    // Opening the gate reveals them; a value edit saves through the DTO path.
    await tester.tap(find.byKey(const ValueKey('happy-eyeballs-toggle')));
    await tester.pumpAndSettle();
    expect(find.text('尝试延迟'), findsOneWidget);
    await _enterKeyed(tester, 'happy-try-delay', '300');
    await _save(tester);

    expect(bridge.settingsRevision(), 1);
    final doc = _saved(bridge);
    expect((doc['SimpleDNSItem'] as Map)['EnableHappyEyeballs'], isTrue);
    expect((doc['HappyEyeballs4RayItem'] as Map)['TryDelayMs'], 300);
  });

  testWidgets('SP-24: closing Happy keeps stored parameter values', (
    tester,
  ) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);

    await _pumpWindow(tester, container);
    await tester.tap(find.text('v2rayN 设置'));
    await tester.pumpAndSettle();

    await tester.ensureVisible(
      find.byKey(const ValueKey('happy-eyeballs-toggle')),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('happy-eyeballs-toggle')));
    await tester.pumpAndSettle();
    await _enterKeyed(tester, 'happy-try-delay', '300');
    // Closing the gate hides the editors but must not clear the values.
    await tester.ensureVisible(
      find.byKey(const ValueKey('happy-eyeballs-toggle')),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('happy-eyeballs-toggle')));
    await tester.pumpAndSettle();
    expect(find.text('尝试延迟'), findsNothing);
    await _save(tester);

    expect(bridge.settingsRevision(), 1);
    final doc = _saved(bridge);
    expect((doc['SimpleDNSItem'] as Map)['EnableHappyEyeballs'], isFalse);
    expect(
      (doc['HappyEyeballs4RayItem'] as Map)['TryDelayMs'],
      300,
      reason: 'hidden values are kept, matching the FakeIP linkage',
    );
  });

  testWidgets('SP-24: trailing commas store no phantom list entries', (
    tester,
  ) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);

    await _pumpWindow(tester, container);
    await _enterKeyed(tester, 'fragment-lengths', '50-100,');
    await tester.tap(find.text('Tun 模式设置'));
    await tester.pumpAndSettle();
    await _enterKeyed(tester, 'tun-route-exclude', '10.0.0.0/8,');
    await _save(tester);

    expect(bridge.settingsRevision(), 1);
    final doc = _saved(bridge);
    expect((doc['Fragment4RayItem'] as Map)['Lengths'], <dynamic>['50-100']);
    expect((doc['TunModeItem'] as Map)['RouteExcludeAddress'], <dynamic>[
      '10.0.0.0/8',
    ]);
  });
}
