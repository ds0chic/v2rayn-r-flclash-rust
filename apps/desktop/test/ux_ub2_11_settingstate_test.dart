import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';
import 'package:v2rayn_desktop/features/settings/settings_window_host.dart';

import 'support/fake_platform_bridge.dart';
import 'support/synthetic_runtime_bridge.dart';

ProviderContainer _container(
  SyntheticBridgePort bridge, {
  SyntheticRuntimeBridge? runtime,
}) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
    profileRowCountProvider.overrideWithValue(4),
    platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
    runtimeBridgeProvider.overrideWithValue(
      runtime ?? SyntheticRuntimeBridge(),
    ),
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

Finder _portField() => find.descendant(
  of: find.byKey(const ValueKey('settings-local-port')),
  matching: find.byType(TextField),
);

/// Pump a shell, then open the settings window as a real dialog route via
/// the production `show()` path. Dialog-mode Esc needs a route to pop —
/// an inline AlertDialog body has none, so the Esc tests cannot reuse
/// [_pumpWindow].
Future<void> _pumpRouted(
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
        home: Scaffold(
          body: TextButton(
            key: const ValueKey('ub2-11-open'),
            onPressed: () {},
            child: const Text('open settings'),
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
  unawaited(
    OptionSettingWindow.show(
      tester.element(find.byKey(const ValueKey('ub2-11-open'))),
    ),
  );
  await tester.pumpAndSettle();
}

Map<String, dynamic> _persisted(SyntheticBridgePort bridge) =>
    jsonDecode(bridge.getSettings().settingsJson) as Map<String, dynamic>;

/// In-memory [SettingsEditorHost] mirroring the r3_wpf fake: the host path
/// shares the Esc/Enter/tri-state handlers under test.
class _FakeHost implements SettingsEditorHost {
  _FakeHost({required this.snapshot});

  final Map<String, dynamic> snapshot;
  int saveCalls = 0;
  int closeCalls = 0;

  @override
  Future<Map<String, dynamic>> loadSnapshot() async => snapshot;

  @override
  Future<SettingsEditorOutcome> save(Map<String, dynamic> draft) async {
    saveCalls++;
    return const SettingsEditorOutcome(ok: true);
  }

  @override
  Future<void> close() async {
    closeCalls++;
  }
}

void main() {
  test('UB2-11: countDraftDifferences counts leaf edits', () {
    final base = defaultSettingsJson();
    Map<String, dynamic> copy() =>
        jsonDecode(jsonEncode(base)) as Map<String, dynamic>;
    expect(countDraftDifferences(base, copy()), 0);

    final oneEdit = copy();
    ((oneEdit['Inbound'] as List).first as Map<String, dynamic>)['UdpEnabled'] =
        false;
    expect(countDraftDifferences(oneEdit, base), 1);

    final twoGroups = copy();
    ((twoGroups['Inbound'] as List).first
            as Map<String, dynamic>)['UdpEnabled'] =
        false;
    (twoGroups['TunModeItem'] as Map<String, dynamic>)['AutoRoute'] = false;
    expect(countDraftDifferences(twoGroups, base), 2);

    // A missing key reads as its null default, not as a difference.
    final missingNull = copy();
    (missingNull['SimpleDNSItem'] as Map<String, dynamic>).remove(
      'Strategy4Freedom',
    );
    expect(countDraftDifferences(missingNull, base), 0);
  });

  test('UB2-11: enterTriggersSave matrix', () {
    expect(
      enterTriggersSave(
        focusedIsTextField: true,
        focusedIsMultiline: false,
        focusedIsSearch: false,
      ),
      isTrue,
    );
    expect(
      enterTriggersSave(
        focusedIsTextField: true,
        focusedIsMultiline: true,
        focusedIsSearch: false,
      ),
      isFalse,
    );
    expect(
      enterTriggersSave(
        focusedIsTextField: true,
        focusedIsMultiline: false,
        focusedIsSearch: true,
      ),
      isFalse,
    );
    expect(
      enterTriggersSave(
        focusedIsTextField: false,
        focusedIsMultiline: false,
        focusedIsSearch: false,
      ),
      isFalse,
    );
  });

  testWidgets('UB2-11: clean open shows the applied revision', (tester) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);

    expect(find.byKey(const ValueKey('settings-tristate')), findsOneWidget);
    expect(find.textContaining('rev:'), findsOneWidget);
  });

  testWidgets('UB2-11: one edit shows草稿未保存 1 处', (tester) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);

    await tester.tap(find.text('开启 UDP'));
    await tester.pump();

    expect(find.text('草稿未保存 1 处'), findsOneWidget);
  });

  testWidgets('UB2-11: Esc when clean closes with no confirm', (tester) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpRouted(tester, container);

    // Key events need focus: tap a field (no edit, stays clean) first.
    await tester.tap(_portField());
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();

    expect(find.byType(OptionSettingWindow), findsNothing);
    expect(bridge.settingsRevision(), 0);
  });

  testWidgets('UB2-11: Esc when dirty confirms; 确认 discards', (tester) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpRouted(tester, container);

    await tester.tap(find.text('开启 UDP'));
    await tester.pump();
    // Checkbox taps may not take focus; focus a field (no edit) so the
    // Esc key event reaches the dialog route.
    await tester.tap(_portField());
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey('settings-discard-confirm')),
      findsOneWidget,
    );
    await tester.tap(find.byKey(const ValueKey('settings-discard-confirm')));
    await tester.pumpAndSettle();

    expect(find.byType(OptionSettingWindow), findsNothing);
    expect(bridge.settingsRevision(), 0);
  });

  testWidgets('UB2-11: Esc when dirty can keep editing', (tester) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpRouted(tester, container);

    await tester.tap(find.text('开启 UDP'));
    await tester.pump();
    await tester.tap(_portField());
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey('settings-discard-cancel')));
    await tester.pumpAndSettle();

    expect(find.byType(OptionSettingWindow), findsOneWidget);
    expect(find.text('草稿未保存 1 处'), findsOneWidget);
    expect(bridge.settingsRevision(), 0);
  });

  testWidgets('UB2-11: Enter in a single-line field saves', (tester) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);

    await tester.enterText(_portField(), '11809');
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    for (var i = 0; i < 30; i++) {
      await tester.pump(const Duration(milliseconds: 100));
    }
    await tester.pumpAndSettle();

    expect(find.byType(OptionSettingWindow), findsNothing);
    final saved = _persisted(bridge);
    expect(
      ((saved['Inbound'] as List).first as Map<String, dynamic>)['LocalPort'],
      11809,
    );
  });

  testWidgets('UB2-11: Enter in search runs search, never saves', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);

    await tester.enterText(find.byKey(const ValueKey('settings-search')), '端口');
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();

    expect(find.byType(OptionSettingWindow), findsOneWidget);
    expect(bridge.settingsRevision(), 0);
  });

  testWidgets('UB2-11: saved but unapplied shows 已保存，未应用', (tester) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(
      bridge,
      runtime: SyntheticRuntimeBridge(failApply: true),
    );
    addTearDown(container.dispose);
    await _pumpWindow(tester, container);

    await tester.tap(find.text('开启 UDP'));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('settings-save')));
    for (var i = 0; i < 30; i++) {
      await tester.pump(const Duration(milliseconds: 100));
    }
    await tester.pumpAndSettle();

    // Saved (persisted) but the apply failed: the window stays open.
    expect(find.byType(OptionSettingWindow), findsOneWidget);
    expect(bridge.settingsRevision(), 1);
    expect(find.text('已保存，未应用'), findsOneWidget);
  });

  testWidgets('UB2-11: host path shares Esc-dirty-confirm', (tester) async {
    final host = _FakeHost(snapshot: defaultSettingsJson());
    tester.view.physicalSize = const Size(1200, 900);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);
    // Standalone host mode (r3_wpf pattern): autofocus carries Esc without
    // tap-to-focus. Inline non-routed dialogs have no route and no autofocus,
    // a configuration production never uses (dialogs always go through
    // show()).
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
    await tester.pump();
    await tester.pumpAndSettle();

    await tester.tap(find.text('开启 UDP'));
    await tester.pump();
    expect(find.text('草稿未保存 1 处'), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('settings-discard-confirm')));
    await tester.pumpAndSettle();

    expect(host.saveCalls, 0);
    expect(host.closeCalls, 1);
  });
}
