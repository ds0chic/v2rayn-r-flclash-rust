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

/// Fake occupancy probe with a fixed verdict (condition B: no real
/// `RegisterHotKey` ever runs in tests).
class _FakeProbe implements HotkeyProbe {
  const _FakeProbe({required this.free});

  final bool free;

  @override
  HotkeyProbeResult probe(HotkeyCombo combo, HotkeyKeyCodec codec) =>
      HotkeyProbeResult(free: free);
}

Future<void> _pump(
  WidgetTester tester,
  ProviderContainer container, {
  HotkeyProbe? probe,
}) async {
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(
        home: Scaffold(
          body: GlobalHotkeyWindow(probe: probe ?? const Win32HotkeyProbe()),
        ),
      ),
    ),
  );
  await tester.pump();
  await tester.pump();
}

Future<void> _recordA(WidgetTester tester) async {
  await tester.tap(find.byKey(const ValueKey<String>('hotkey-record-0')));
  await tester.pump();
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.sendKeyDownEvent(LogicalKeyboardKey.keyA);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.keyA);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await tester.pump();
}

void main() {
  testWidgets('UB2-04: hotkey rows carry helper captions', (tester) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(bridge),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(4),
      ],
    );
    addTearDown(container.dispose);
    await _pump(tester, container);

    expect(find.textContaining('防误触'), findsWidgets);
    // Close with a live container so dispose() can restore registration.
    await tester.tap(find.text('取消'));
    await tester.pumpAndSettle();
  });

  testWidgets('UB2-04: occupied combo shows a conflict note', (tester) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(bridge),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(4),
      ],
    );
    addTearDown(container.dispose);
    // The fake reports every combo occupied.
    await _pump(tester, container, probe: const _FakeProbe(free: false));
    await _recordA(tester);

    expect(find.textContaining('已被其它程序占用'), findsOneWidget);
    await tester.tap(find.text('取消'));
    await tester.pumpAndSettle();
  });

  testWidgets('UB2-04: free combo shows no conflict note', (tester) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(bridge),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(4),
      ],
    );
    addTearDown(container.dispose);
    await _pump(tester, container, probe: const _FakeProbe(free: true));
    await _recordA(tester);

    expect(find.textContaining('已被其它程序占用'), findsNothing);
    expect(find.textContaining('防误触'), findsWidgets);
    await tester.tap(find.text('取消'));
    await tester.pumpAndSettle();
  });
}
