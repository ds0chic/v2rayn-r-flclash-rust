import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/global_hotkey_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

void main() {
  testWidgets('hotkey window records a combo and persists it', (tester) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(bridge),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(4),
      ],
    );
    addTearDown(container.dispose);

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: GlobalHotkeyWindow())),
      ),
    );
    await tester.pump();

    // Five rows are present.
    for (var action = 0; action < 5; action++) {
      expect(
        find.byKey(ValueKey<String>('hotkey-record-$action')),
        findsOneWidget,
      );
    }

    await tester.tap(find.byKey(const ValueKey<String>('hotkey-record-0')));
    await tester.pump();
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.keyA);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.keyA);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pump();

    expect(
      tester
          .widget<Text>(find.byKey(const ValueKey<String>('hotkey-label-0')))
          .data!,
      contains('Ctrl'),
    );

    await tester.tap(find.text('保存'));
    await tester.pumpAndSettle();

    final document = container.read(settingsControllerProvider).document;
    final hotkeys = (document['GlobalHotkeys'] as List)
        .cast<Map<String, dynamic>>();
    final showForm = hotkeys.firstWhere(
      (h) => (h['EGlobalHotkey'] as num?)?.toInt() == 0,
    );
    expect(showForm['Control'], isTrue);
    expect(showForm['KeyCode'], isNotNull);
  });
}
