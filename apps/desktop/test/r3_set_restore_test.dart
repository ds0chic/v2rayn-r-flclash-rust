import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/backup/backup_controller.dart';
import 'package:v2rayn_desktop/features/backup/backup_picker.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/settings/hotkey_keycodec.dart';
import 'package:v2rayn_desktop/features/settings/hotkeys.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

/// R3-SET-04: restoring a backup must reload and re-register the restored
/// GlobalHotkeys so native dispatch uses the new combos immediately and after
/// the editor is opened/cancelled. Uses a fake registrar; no OS hotkey runs.

class _FakePicker implements BackupPicker {
  const _FakePicker({this.archive});
  final String? archive;
  @override
  Future<String?> pickArchive() async => archive;
  @override
  Future<String?> pickDirectory() async => null;
}

class _FakeRegistrar implements HotkeyRegistrar {
  List<HotkeyRegistration> last = const <HotkeyRegistration>[];
  int unregisterCount = 0;

  @override
  Future<(Set<GlobalHotkeyAction>, List<String>)> register(
    List<HotkeyRegistration> regs, {
    HotkeyTriggerHandler? onTriggered,
    HotkeyKeyCodec codec = const HotkeyKeyCodec(),
  }) async {
    last = regs;
    final accepted = <GlobalHotkeyAction>{};
    for (final registration in regs) {
      accepted.addAll(registration.actions);
    }
    return (accepted, const <String>[]);
  }

  @override
  Future<void> unregisterAll() async {
    unregisterCount++;
  }
}

/// A settings controller whose `document` can be swapped to model the config
/// that the Rust restore has written to disk before the Dart reload runs.
class _FakeSettingsController extends SettingsController {
  _FakeSettingsController(this._doc);
  Map<String, dynamic> _doc;

  void setRestored(Map<String, dynamic> doc) => _doc = doc;

  @override
  SettingsViewState build() => SettingsViewState(loaded: true, document: _doc);

  @override
  SettingsViewState load() {
    state = SettingsViewState(loaded: true, document: _doc);
    return state;
  }
}

Map<String, dynamic> _docWithShowFormKey(int wpfKey) => <String, dynamic>{
  'GlobalHotkeys': <Map<String, dynamic>>[
    <String, dynamic>{
      'EGlobalHotkey': GlobalHotkeyAction.showForm.value,
      'Alt': false,
      'Control': true,
      'Shift': false,
      'KeyCode': wpfKey,
    },
  ],
};

void main() {
  test(
    'restore reloads the restored hotkey and cancel keeps it (R3-SET-04)',
    () async {
      final registrar = _FakeRegistrar();
      // Pre-restore native binding is Ctrl+K (WPF 54).
      final settings = _FakeSettingsController(_docWithShowFormKey(54));
      final container = ProviderContainer(
        overrides: [
          bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
          backupPickerProvider.overrideWithValue(
            const _FakePicker(archive: '/tmp/backup.zip'),
          ),
          hotkeyRegistrarProvider.overrideWithValue(registrar),
          settingsControllerProvider.overrideWith(() => settings),
        ],
      );
      addTearDown(container.dispose);

      // The shell installs the live dispatcher before native registration.
      container.read(hotkeyDispatchProvider).handler = (_) {};

      final hotkeys = container.read(hotkeyControllerProvider.notifier);
      // The old binding is registered before the restore.
      await hotkeys.save(const <HotkeyBinding>[
        HotkeyBinding(
          action: GlobalHotkeyAction.showForm,
          control: true,
          keyCode: 54,
        ),
      ], () => true);
      expect(registrar.last.single.combo.keyCode, 54);

      // The backup carries Ctrl+J (WPF 53); the Rust restore writes it and the
      // Dart reload must pick it up.
      settings.setRestored(_docWithShowFormKey(53));
      await container
          .read(backupControllerProvider.notifier)
          .restoreFromArchive();

      expect(
        registrar.last.single.combo.keyCode,
        53,
        reason: 'restore must re-register the restored combo',
      );
      expect(
        container.read(hotkeyControllerProvider).bindings.single.keyCode,
        53,
      );

      // Opening the editor drops the live registration; cancelling restores the
      // same (restored) combo, not the pre-restore one.
      await hotkeys.beginEdit();
      expect(registrar.unregisterCount, greaterThanOrEqualTo(1));
      await hotkeys.cancelEdit();
      expect(
        registrar.last.single.combo.keyCode,
        53,
        reason: 'cancel must re-register the restored combo',
      );
    },
  );
}
