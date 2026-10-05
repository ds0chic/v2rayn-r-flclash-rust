// R4-26 repro: the global-hotkey pause flag was not a dispatch gate.
//
// Upstream `HotkeyManager.OnThreadPreProcessMessage` checks `IsPause` *at the
// dispatch point*: while the hotkey editor is recording, a combination that
// fires must reach the editor, not run its saved window/proxy action. Before
// the fix, `HotkeyController.registerAll` handed the raw shell handler to the
// registrar, so a trigger invoked the saved action even after `beginEdit`
// paused the editor. This test asserts the contract and intentionally fails on
// the pre-fix implementation (the assertions are NOT bent to the bug).
//
// The D30 "copy proxy command" dispatch gap and the tray-icon applied-fact
// mapping are covered by test/r4_26_contract_test.dart.
//
// Synthetic only: fake registrar, no native library, no OS registration, no
// port, no host proxy/TUN/Run key, no user data.
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/hotkey_keycodec.dart';
import 'package:v2rayn_desktop/features/settings/hotkeys.dart';

class _FakeRegistrar implements HotkeyRegistrar {
  HotkeyTriggerHandler? onTriggered;

  @override
  Future<(Set<GlobalHotkeyAction>, List<String>)> register(
    List<HotkeyRegistration> registrations, {
    HotkeyTriggerHandler? onTriggered,
    HotkeyKeyCodec codec = const HotkeyKeyCodec(),
  }) async {
    this.onTriggered = onTriggered;
    return (const <GlobalHotkeyAction>{}, const <String>[]);
  }

  @override
  Future<void> unregisterAll() async {}
}

void main() {
  test(
    'a hotkey fired while the editor is paused must not run its action',
    () async {
      final fake = _FakeRegistrar();
      final container = ProviderContainer(
        overrides: [hotkeyRegistrarProvider.overrideWithValue(fake)],
      );
      addTearDown(container.dispose);

      final fired = <GlobalHotkeyAction>[];
      container.read(hotkeyDispatchProvider).handler = fired.add;
      final controller = container.read(hotkeyControllerProvider.notifier);

      await controller.registerAll();
      expect(fake.onTriggered, isNotNull);

      // Editor opened: native dispatch paused.
      await controller.beginEdit();
      expect(controller.isPaused, isTrue);

      // A queued/racing combination still reaches the registrar callback.
      fake.onTriggered!(GlobalHotkeyAction.systemProxySet);

      expect(
        fired,
        isEmpty,
        reason: 'a paused hotkey must not run its saved proxy/window action',
      );
    },
  );
}
