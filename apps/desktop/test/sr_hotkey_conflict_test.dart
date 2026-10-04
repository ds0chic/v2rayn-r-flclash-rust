import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hotkey_manager/hotkey_manager.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/global_hotkey_window.dart';
import 'package:v2rayn_desktop/features/settings/hotkey_keycodec.dart';
import 'package:v2rayn_desktop/features/settings/hotkeys.dart';

/// Probe stub: returns a fixed result and records the combos it saw.
class _FakeProbe implements HotkeyProbe {
  _FakeProbe(this.result);

  final HotkeyProbeResult result;
  final List<HotkeyCombo> probed = <HotkeyCombo>[];

  @override
  HotkeyProbeResult probe(HotkeyCombo combo, HotkeyKeyCodec codec) {
    probed.add(combo);
    return result;
  }
}

/// Plugin stub: records what reached the native backend.
class _FakePlugin implements HotkeyPlugin {
  final List<HotKey> registered = <HotKey>[];
  int unregisterAllCount = 0;

  @override
  Future<void> unregisterAll() async => unregisterAllCount++;

  @override
  Future<void> register(
    HotKey hotKey, {
    void Function(HotKey)? keyDownHandler,
  }) async {
    registered.add(hotKey);
  }
}

HotkeyRegistration _ctrlAltF11(List<GlobalHotkeyAction> actions) =>
    HotkeyRegistration(
      combo: const HotkeyCombo(keyCode: 100, control: true, alt: true),
      actions: actions,
    );

ProviderContainer _windowContainer(HotkeyRegistrar registrar) =>
    ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(SyntheticBridgePort(count: 2)),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(2),
        hotkeyRegistrarProvider.overrideWithValue(registrar),
      ],
    );

Widget _window(ProviderContainer container) => UncontrolledProviderScope(
  container: container,
  child: const MaterialApp(home: Scaffold(body: GlobalHotkeyWindow())),
);

Future<void> _recordCtrlAltF11(WidgetTester tester) async {
  await tester.tap(find.byKey(const ValueKey<String>('hotkey-record-0')));
  await tester.pump();
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.sendKeyDownEvent(LogicalKeyboardKey.altLeft);
  await tester.sendKeyDownEvent(LogicalKeyboardKey.f11);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.f11);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.altLeft);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await tester.pump();
}

void main() {
  group('SR-HOTKEY-CONFLICT error mapping', () {
    test('1409 is reported as an external owner, other codes kept raw', () {
      expect(
        hotkeyConflictNote(const <GlobalHotkeyAction>[
          GlobalHotkeyAction.showForm,
        ], errorHotkeyAlreadyRegistered),
        contains('组合已被其它程序占用'),
      );
      // A missing code (dart:ffi does not always preserve GetLastError) still
      // means occupancy for a failed, validated combination.
      expect(
        hotkeyConflictNote(const <GlobalHotkeyAction>[
          GlobalHotkeyAction.showForm,
        ], null),
        contains('组合已被其它程序占用'),
      );
      final other = hotkeyConflictNote(const <GlobalHotkeyAction>[
        GlobalHotkeyAction.showForm,
      ], 5);
      expect(other, contains('Win32 5'));
      expect(other, isNot(contains('占用')));
    });
  });

  group('PluginHotkeyRegistrar probe gate', () {
    test('occupied combo is reported and never handed to the plugin', () async {
      final plugin = _FakePlugin();
      final probe = _FakeProbe(
        const HotkeyProbeResult(
          free: false,
          errorCode: errorHotkeyAlreadyRegistered,
        ),
      );
      final registrar = PluginHotkeyRegistrar(probe: probe, plugin: plugin);

      final (accepted, failures) = await registrar.register(
        <HotkeyRegistration>[
          _ctrlAltF11(<GlobalHotkeyAction>[GlobalHotkeyAction.showForm]),
        ],
      );

      expect(accepted, isEmpty);
      expect(plugin.registered, isEmpty);
      expect(probe.probed.single.keyCode, 100);
      expect(failures.single, contains('显示/隐藏窗口'));
      expect(failures.single, contains('组合已被其它程序占用'));
    });

    test(
      'free combo is probed then registered; accepted actions returned',
      () async {
        final plugin = _FakePlugin();
        final probe = _FakeProbe(const HotkeyProbeResult(free: true));
        final registrar = PluginHotkeyRegistrar(probe: probe, plugin: plugin);

        final (accepted, failures) = await registrar.register(
          <HotkeyRegistration>[
            _ctrlAltF11(<GlobalHotkeyAction>[GlobalHotkeyAction.showForm]),
          ],
        );

        expect(failures, isEmpty);
        expect(accepted, <GlobalHotkeyAction>[GlobalHotkeyAction.showForm]);
        expect(plugin.unregisterAllCount, greaterThanOrEqualTo(1));
        expect(plugin.registered.single.identifier, contains('v2rayn-100'));
      },
    );
  });

  testWidgets('conflict save keeps the window open and names the combo', (
    tester,
  ) async {
    final plugin = _FakePlugin();
    final registrar = PluginHotkeyRegistrar(
      probe: _FakeProbe(
        const HotkeyProbeResult(
          free: false,
          errorCode: errorHotkeyAlreadyRegistered,
        ),
      ),
      plugin: plugin,
    );
    final container = _windowContainer(registrar);
    addTearDown(container.dispose);

    await tester.pumpWidget(_window(container));
    await tester.pump();
    await _recordCtrlAltF11(tester);
    expect(
      tester
          .widget<Text>(find.byKey(const ValueKey<String>('hotkey-label-0')))
          .data,
      'Ctrl + Alt + F11',
    );

    await tester.tap(find.text('保存'));
    await tester.pumpAndSettle();

    // The window stays open and the status names the conflicting combination.
    expect(find.text('全局热键设置'), findsOneWidget);
    expect(plugin.registered, isEmpty);
    final texts = tester
        .widgetList<Text>(find.byType(Text))
        .map((t) => t.data ?? '')
        .toList();
    expect(
      texts.any((s) => s.contains('组合已被其它程序占用')),
      isTrue,
      reason: 'texts=$texts',
    );
  });
}
