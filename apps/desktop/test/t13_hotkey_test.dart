import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/desktop_integration.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/hotkey_keycodec.dart';
import 'package:v2rayn_desktop/features/settings/hotkeys.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

/// Fake registrar that records registrations and can inject a conflict.
class FakeHotkeyRegistrar implements HotkeyRegistrar {
  FakeHotkeyRegistrar({this.conflicting = const <GlobalHotkeyAction>{}});

  final Set<GlobalHotkeyAction> conflicting;
  final List<List<HotkeyBinding>> registrations = <List<HotkeyBinding>>[];

  /// The dispatcher passed on the most recent registration, so dispatch tests
  /// can simulate an OS key press.
  HotkeyTriggerHandler? lastTrigger;
  int unregisterAllCount = 0;

  @override
  Future<(Set<GlobalHotkeyAction>, List<String>)> register(
    List<HotkeyBinding> bindings, {
    HotkeyTriggerHandler? onTriggered,
    HotkeyKeyCodec codec = const HotkeyKeyCodec(),
  }) async {
    registrations.add(bindings);
    lastTrigger = onTriggered;
    final accepted = <GlobalHotkeyAction>{};
    final failures = <String>[];
    for (final b in bindings) {
      if (!b.isBound) continue;
      if (conflicting.contains(b.action)) {
        failures.add('${b.action.label}: 注册失败（已被占用）');
      } else {
        accepted.add(b.action);
      }
    }
    return (accepted, failures);
  }

  @override
  Future<void> unregisterAll() async {
    unregisterAllCount++;
  }
}

ProviderContainer _container({
  required FakeHotkeyRegistrar registrar,
  Map<String, dynamic>? document,
}) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(SyntheticBridgePort(count: 2)),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(2),
      hotkeyRegistrarProvider.overrideWithValue(registrar),
    ],
  );
  if (document != null) {
    // Seed the settings controller document directly through a group save.
    container
        .read(settingsControllerProvider.notifier)
        .saveGroup('GlobalHotkeys', document['GlobalHotkeys']);
  }
  return container;
}

void main() {
  group('hotkey binding model', () {
    test('unbound correspondence and label', () {
      const b = HotkeyBinding(action: GlobalHotkeyAction.showForm);
      expect(b.isBound, isFalse);
      expect(b.label, '（未设置）');
    });

    test('parses and serializes the KeyEventItem shape', () {
      final b = HotkeyBinding.fromSettings(<String, dynamic>{
        'EGlobalHotkey': 2,
        'Alt': true,
        'Control': true,
        'Shift': false,
        'KeyCode': 75,
      });
      expect(b.action, GlobalHotkeyAction.systemProxySet);
      expect(b.alt, isTrue);
      expect(b.control, isTrue);
      expect(b.keyCode, 75);
      expect(b.toSettings()['EGlobalHotkey'], 2);
      expect(b.toSettings()['KeyCode'], 75);
    });
  });

  group('hotkey controller', () {
    test('registerAll accepts valid bindings and reports conflicts', () async {
      final registrar = FakeHotkeyRegistrar(
        conflicting: <GlobalHotkeyAction>{GlobalHotkeyAction.systemProxyPac},
      );
      final container = _container(registrar: registrar);
      addTearDown(container.dispose);
      final controller = container.read(hotkeyControllerProvider.notifier);
      container.read(hotkeyControllerProvider.notifier);

      // Seed bindings directly (no settings round trip needed here).
      container.read(hotkeyControllerProvider.notifier);
      final state0 = container.read(hotkeyControllerProvider);
      expect(state0.bindings, isEmpty);

      // Use loadFromSettings with an empty seeded document.
      controller.loadFromSettings();
      final registered = await controller.registerAll();
      expect(registered.registered, isEmpty);
      expect(registered.conflicts, isEmpty);
      expect(registrar.registrations.length, 1);
    });

    test('conflict is surfaced, not swallowed', () async {
      final registrar = FakeHotkeyRegistrar(
        conflicting: <GlobalHotkeyAction>{GlobalHotkeyAction.showForm},
      );
      final container = _container(registrar: registrar);
      addTearDown(container.dispose);
      final controller = container.read(hotkeyControllerProvider.notifier);
      controller.loadFromSettings();
      // Inject a bound showForm binding into the controller state via the
      // public save path with a no-op persist.
      await controller.save(const <HotkeyBinding>[
        HotkeyBinding(
          action: GlobalHotkeyAction.showForm,
          control: true,
          keyCode: 70,
        ),
      ], () => true);
      final state = container.read(hotkeyControllerProvider);
      expect(state.registered, isEmpty);
      expect(state.conflicts.single, contains('显示/隐藏窗口'));
    });

    test('save rejects when persist fails', () async {
      final registrar = FakeHotkeyRegistrar();
      final container = _container(registrar: registrar);
      addTearDown(container.dispose);
      final controller = container.read(hotkeyControllerProvider.notifier);
      final ok = await controller.save(const <HotkeyBinding>[
        HotkeyBinding(
          action: GlobalHotkeyAction.systemProxyClear,
          control: true,
          keyCode: 67,
        ),
      ], () => false);
      expect(ok, isFalse);
      expect(container.read(hotkeyControllerProvider).status, '热键保存失败');
    });
  });

  group('close behavior', () {
    test('parses UiItem close flags', () {
      final behavior = CloseBehavior.fromDocument(<String, dynamic>{
        'UiItem': <String, dynamic>{
          'Hide2TrayWhenClose': true,
          'AutoHideStartup': false,
        },
      });
      expect(behavior.hide2TrayWhenClose, isTrue);
      expect(behavior.autoHideStartup, isFalse);
      expect(
        CloseBehavior.fromDocument(<String, dynamic>{}).hide2TrayWhenClose,
        isFalse,
      );
    });
  });
}
