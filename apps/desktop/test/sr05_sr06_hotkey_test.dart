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

/// Fake OS registrar: models a real registration table so a "combo press" only
/// dispatches while the combination is actually registered. No host hotkey is
/// touched.
class _FakeOsRegistrar implements HotkeyRegistrar {
  _FakeOsRegistrar({this.failing = const <GlobalHotkeyAction>{}});

  final Set<GlobalHotkeyAction> failing;
  final List<List<HotkeyRegistration>> registrations =
      <List<HotkeyRegistration>>[];
  HotkeyTriggerHandler? trigger;
  int unregisterCount = 0;

  List<HotkeyRegistration> get last =>
      registrations.isEmpty ? const <HotkeyRegistration>[] : registrations.last;

  @override
  Future<(Set<GlobalHotkeyAction>, List<String>)> register(
    List<HotkeyRegistration> regs, {
    HotkeyTriggerHandler? onTriggered,
    HotkeyKeyCodec codec = const HotkeyKeyCodec(),
  }) async {
    registrations.add(regs);
    trigger = onTriggered;
    final accepted = <GlobalHotkeyAction>{};
    final failures = <String>[];
    for (final registration in regs) {
      for (final action in registration.actions) {
        if (failing.contains(action)) {
          failures.add('${action.label}: 注册失败（已被占用）');
        } else {
          accepted.add(action);
        }
      }
    }
    return (accepted, failures);
  }

  @override
  Future<void> unregisterAll() async {
    unregisterCount++;
    trigger = null;
  }

  /// Simulate the OS delivering a registered combination.
  void pressCombo(
    int keyCode, {
    bool control = false,
    bool alt = false,
    bool shift = false,
  }) {
    final handler = trigger;
    if (handler == null) return;
    for (final registration in last) {
      if (registration.combo.keyCode == keyCode &&
          registration.combo.control == control &&
          registration.combo.alt == alt &&
          registration.combo.shift == shift) {
        for (final action in registration.actions) {
          handler(action);
        }
      }
    }
  }
}

ProviderContainer _container(_FakeOsRegistrar registrar) => ProviderContainer(
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

Future<void> _recordCtrlKey(
  WidgetTester tester,
  int action,
  int wpfLetterKey,
) async {
  await tester.tap(find.byKey(ValueKey<String>('hotkey-record-$action')));
  await tester.pump();
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.pump();
  await tester.sendKeyDownEvent(
    wpfLetterKey == 54 ? LogicalKeyboardKey.keyK : LogicalKeyboardKey.keyJ,
  );
  await tester.sendKeyUpEvent(
    wpfLetterKey == 54 ? LogicalKeyboardKey.keyK : LogicalKeyboardKey.keyJ,
  );
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await tester.pump();
}

void main() {
  group('SR-06 groupHotkeyBindings', () {
    test('same combo with two actions collapses to one registration', () {
      final groups = groupHotkeyBindings(const <HotkeyBinding>[
        HotkeyBinding(
          action: GlobalHotkeyAction.showForm,
          control: true,
          keyCode: 54,
        ),
        HotkeyBinding(
          action: GlobalHotkeyAction.systemProxyClear,
          control: true,
          keyCode: 54,
        ),
      ]);
      expect(groups.length, 1);
      expect(groups.single.actions, <GlobalHotkeyAction>[
        GlobalHotkeyAction.showForm,
        GlobalHotkeyAction.systemProxyClear,
      ]);
    });

    test(
      'different combos stay separate, duplicates collapse, unbound skipped',
      () {
        final groups = groupHotkeyBindings(const <HotkeyBinding>[
          HotkeyBinding(
            action: GlobalHotkeyAction.showForm,
            control: true,
            keyCode: 54,
          ),
          HotkeyBinding(
            action: GlobalHotkeyAction.showForm,
            control: true,
            keyCode: 54,
          ),
          HotkeyBinding(
            action: GlobalHotkeyAction.systemProxySet,
            alt: true,
            keyCode: 44,
          ),
          HotkeyBinding(action: GlobalHotkeyAction.systemProxyPac),
        ]);
        expect(groups.length, 2);
        expect(groups.first.actions.length, 1);
        expect(groups.last.combo.keyCode, 44);
        expect(groups.last.combo.alt, isTrue);
      },
    );
  });

  test('SR-06 one registration dispatches every action of the combo', () async {
    final registrar = _FakeOsRegistrar();
    final container = _container(registrar);
    addTearDown(container.dispose);
    final fired = <GlobalHotkeyAction>[];
    container.read(hotkeyDispatchProvider).handler = fired.add;
    final controller = container.read(hotkeyControllerProvider.notifier);

    final ok = await controller.save(const <HotkeyBinding>[
      HotkeyBinding(
        action: GlobalHotkeyAction.showForm,
        control: true,
        keyCode: 54,
      ),
      HotkeyBinding(
        action: GlobalHotkeyAction.systemProxyClear,
        control: true,
        keyCode: 54,
      ),
    ], () => true);

    expect(ok, isTrue);
    expect(registrar.last.length, 1);
    expect(registrar.last.single.actions.length, 2);
    registrar.pressCombo(54, control: true);
    expect(fired, <GlobalHotkeyAction>[
      GlobalHotkeyAction.showForm,
      GlobalHotkeyAction.systemProxyClear,
    ]);
  });

  testWidgets(
    'SR-05 open pauses the old binding, records it without firing, cancel restores',
    (tester) async {
      final registrar = _FakeOsRegistrar();
      final container = _container(registrar);
      addTearDown(container.dispose);
      final fired = <GlobalHotkeyAction>[];
      container.read(hotkeyDispatchProvider).handler = fired.add;
      final controller = container.read(hotkeyControllerProvider.notifier);
      await controller.save(const <HotkeyBinding>[
        HotkeyBinding(
          action: GlobalHotkeyAction.showForm,
          control: true,
          keyCode: 54,
        ),
      ], () => true);
      registrar.pressCombo(54, control: true);
      expect(fired, <GlobalHotkeyAction>[GlobalHotkeyAction.showForm]);
      fired.clear();

      await tester.pumpWidget(_window(container));
      await tester.pump();

      // Editor open: the native registration is dropped (IsPause), so the old
      // combo can no longer fire its action.
      expect(registrar.unregisterCount, greaterThanOrEqualTo(1));
      expect(registrar.trigger, isNull);
      registrar.pressCombo(54, control: true);
      expect(fired, isEmpty);

      // Recording the old combo only captures it in the editor.
      await _recordCtrlKey(tester, 0, 54);
      expect(
        tester
            .widget<Text>(find.byKey(const ValueKey<String>('hotkey-label-0')))
            .data,
        'Ctrl + K',
      );
      expect(fired, isEmpty);

      // Cancel (dismiss the widget) restores the pre-edit registration.
      await tester.pumpWidget(const SizedBox());
      await tester.pump();
      await tester.pump();
      expect(registrar.trigger, isNotNull);
      registrar.pressCombo(54, control: true);
      expect(fired, <GlobalHotkeyAction>[GlobalHotkeyAction.showForm]);
    },
  );

  testWidgets('SR-05 save registers the new combo and dispatches it', (
    tester,
  ) async {
    final registrar = _FakeOsRegistrar();
    final container = _container(registrar);
    addTearDown(container.dispose);
    final fired = <GlobalHotkeyAction>[];
    container.read(hotkeyDispatchProvider).handler = fired.add;

    await tester.pumpWidget(_window(container));
    await tester.pump();
    await _recordCtrlKey(tester, 2, 53); // systemProxySet = Ctrl + J
    await tester.tap(find.text('保存'));
    await tester.pumpAndSettle();

    expect(find.text('全局热键设置'), findsNothing);
    registrar.pressCombo(53, control: true);
    expect(fired, <GlobalHotkeyAction>[GlobalHotkeyAction.systemProxySet]);
  });

  testWidgets('SR-06 a failed registration keeps the window open', (
    tester,
  ) async {
    final registrar = _FakeOsRegistrar(
      failing: <GlobalHotkeyAction>{GlobalHotkeyAction.showForm},
    );
    final container = _container(registrar);
    addTearDown(container.dispose);
    final controller = container.read(hotkeyControllerProvider.notifier);

    final ok = await controller.save(const <HotkeyBinding>[
      HotkeyBinding(
        action: GlobalHotkeyAction.showForm,
        control: true,
        keyCode: 54,
      ),
    ], () => true);
    expect(ok, isFalse);
    expect(container.read(hotkeyControllerProvider).conflicts, isNotEmpty);

    await tester.pumpWidget(_window(container));
    await tester.pump();
    await _recordCtrlKey(tester, 0, 54);
    await tester.tap(find.text('保存'));
    await tester.pumpAndSettle();

    expect(find.text('全局热键设置'), findsOneWidget);
    expect(
      tester
          .widgetList<Text>(find.byType(Text))
          .map((t) => t.data ?? '')
          .any((s) => s.contains('注册冲突')),
      isTrue,
    );
  });
}
