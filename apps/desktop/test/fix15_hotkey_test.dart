import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/global_hotkey_window.dart';
import 'package:v2rayn_desktop/features/settings/hotkey_keycodec.dart';
import 'package:v2rayn_desktop/features/settings/hotkeys.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

/// Captures the dispatcher so a simulated OS key press can be routed.
class _RecordingRegistrar implements HotkeyRegistrar {
  HotkeyTriggerHandler? onTriggered;
  int registerCount = 0;

  @override
  Future<(Set<GlobalHotkeyAction>, List<String>)> register(
    List<HotkeyRegistration> registrations, {
    HotkeyTriggerHandler? onTriggered,
    HotkeyKeyCodec codec = const HotkeyKeyCodec(),
  }) async {
    registerCount++;
    this.onTriggered = onTriggered;
    return (
      <GlobalHotkeyAction>{
        for (final registration in registrations) ...registration.actions,
      },
      const <String>[],
    );
  }

  @override
  Future<void> unregisterAll() async {}
}

void main() {
  group('HotkeyKeyCodec WPF Key <-> VK <-> Flutter', () {
    const codec = HotkeyKeyCodec();

    test('known WPF Key values map to the upstream virtual keys', () {
      expect(codec.virtualKeyFromWpf(44), 0x41); // Key.A -> VK_A
      expect(codec.virtualKeyFromWpf(90), 0x70); // Key.F1 -> VK_F1
      expect(codec.virtualKeyFromWpf(34), 0x30); // Key.D0 -> VK_0
      expect(codec.virtualKeyFromWpf(18), 0x20); // Key.Space -> VK_SPACE
    });

    test('VK round-trips back to the persisted WPF Key', () {
      expect(codec.wpfFromVirtualKey(0x41), 44);
      expect(codec.wpfFromVirtualKey(0x70), 90);
      expect(codec.wpfFromVirtualKey(0x30), 34);
    });

    test('WPF Key resolves to a registerable Flutter physical key', () {
      expect(codec.physicalFromWpf(44), PhysicalKeyboardKey.keyA);
      expect(codec.physicalFromWpf(90), PhysicalKeyboardKey.f1);
      expect(codec.labelForWpf(54), 'K');
      expect(codec.labelForWpf(HotkeyKeyCodec.wpfNone), isNull);
    });

    test('unsupported keys never guess', () {
      expect(codec.virtualKeyFromWpf(1), isNull);
      expect(codec.physicalFromWpf(1), isNull);
      expect(codec.wpfFromVirtualKey(0x99), HotkeyKeyCodec.wpfNone);
    });

    test('modifier detection matches the recording rule', () {
      expect(
        HotkeyKeyCodec.isModifierKey(LogicalKeyboardKey.controlLeft),
        isTrue,
      );
      expect(
        HotkeyKeyCodec.isModifierKey(LogicalKeyboardKey.shiftRight),
        isTrue,
      );
      expect(HotkeyKeyCodec.isModifierKey(LogicalKeyboardKey.keyA), isFalse);
    });
  });

  group('hotkey dispatch', () {
    test('registered trigger reaches the shell dispatcher', () async {
      final registrar = _RecordingRegistrar();
      final container = ProviderContainer(
        overrides: [
          bridgePortProvider.overrideWithValue(SyntheticBridgePort(count: 2)),
          uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
          profileRowCountProvider.overrideWithValue(2),
          hotkeyRegistrarProvider.overrideWithValue(registrar),
        ],
      );
      addTearDown(container.dispose);

      final fired = <GlobalHotkeyAction>[];
      container.read(hotkeyDispatchProvider).handler = fired.add;
      final controller = container.read(hotkeyControllerProvider.notifier);
      await controller.save(const <HotkeyBinding>[
        HotkeyBinding(
          action: GlobalHotkeyAction.showForm,
          control: true,
          keyCode: 54, // WPF Key.K
        ),
      ], () => true);

      expect(registrar.onTriggered, isNotNull);
      registrar.onTriggered!(GlobalHotkeyAction.showForm);
      expect(fired, <GlobalHotkeyAction>[GlobalHotkeyAction.showForm]);
    });

    test('a failed persist does not re-register or dispatch', () async {
      final registrar = _RecordingRegistrar();
      final container = ProviderContainer(
        overrides: [
          bridgePortProvider.overrideWithValue(SyntheticBridgePort(count: 2)),
          uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
          profileRowCountProvider.overrideWithValue(2),
          hotkeyRegistrarProvider.overrideWithValue(registrar),
        ],
      );
      addTearDown(container.dispose);
      final controller = container.read(hotkeyControllerProvider.notifier);
      final ok = await controller.save(const <HotkeyBinding>[
        HotkeyBinding(action: GlobalHotkeyAction.showForm, keyCode: 44),
      ], () => false);
      expect(ok, isFalse);
      expect(registrar.registerCount, 0);
    });
  });

  group('hotkey window recording', () {
    testWidgets(
      'a modifier does not terminate recording, then key sets WPF Key',
      (tester) async {
        final bridge = SyntheticBridgePort(count: 4);
        final registrar = _RecordingRegistrar();
        final container = ProviderContainer(
          overrides: [
            bridgePortProvider.overrideWithValue(bridge),
            uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
            profileRowCountProvider.overrideWithValue(4),
            hotkeyRegistrarProvider.overrideWithValue(registrar),
          ],
        );
        addTearDown(container.dispose);

        await tester.pumpWidget(
          UncontrolledProviderScope(
            container: container,
            child: const MaterialApp(
              home: Scaffold(body: GlobalHotkeyWindow()),
            ),
          ),
        );
        await tester.pump();

        await tester.tap(find.byKey(const ValueKey<String>('hotkey-record-1')));
        await tester.pump();
        await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
        await tester.pump();

        // Still recording after the modifier (SET-15 regression guard).
        expect(find.text('按下组合…'), findsOneWidget);

        await tester.sendKeyDownEvent(LogicalKeyboardKey.keyK);
        await tester.sendKeyUpEvent(LogicalKeyboardKey.keyK);
        await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
        await tester.pump();

        expect(
          tester
              .widget<Text>(
                find.byKey(const ValueKey<String>('hotkey-label-1')),
              )
              .data,
          'Ctrl + K',
        );

        await tester.tap(find.text('保存'));
        await tester.pumpAndSettle();

        final document = container.read(settingsControllerProvider).document;
        final hotkeys = (document['GlobalHotkeys'] as List)
            .cast<Map<String, dynamic>>();
        final clear = hotkeys.firstWhere(
          (h) => (h['EGlobalHotkey'] as num?)?.toInt() == 1,
        );
        expect(clear['Control'], isTrue);
        expect(clear['KeyCode'], 54); // WPF Key.K, not the Flutter key id
        expect(registrar.registerCount, 1);
      },
    );
  });
}
