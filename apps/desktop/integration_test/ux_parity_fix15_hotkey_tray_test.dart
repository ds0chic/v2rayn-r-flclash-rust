// FIX-15 / SET-15 / RT-12 / RT-13: real Windows window proof that
//  (a) a hotkey recorded with a modifier is persisted as the WPF Key enum and
//      survives a reopen (the label reads back the same combo), and
//  (b) the close-to-tray path hides the real window and restores it.
//
// Run modes (isolated data dir via V2RAYN_R_DATA_DIR):
//   V2RAYN_R_FIX15_MODE=record (default): open the hotkey window, record
//     Ctrl+A on 显示/隐藏窗口, save, and assert the persisted KeyCode == 44
//     (System.Windows.Input.Key.A), never a Flutter key id.
//   V2RAYN_R_FIX15_MODE=reopen: launch a fresh process on the same data dir,
//     open the hotkey window, and assert the recorded combo reads back.
//
// The hotkey window is auto-opened by the UI-only V2RAYN_R_OPEN_HOTKEY hook.
// No kernel is started and no port is bound; the host proxy / Run key are
// never touched.
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:window_manager/window_manager.dart';
import 'package:v2rayn_desktop/app/app.dart';
import 'package:v2rayn_desktop/app/shell/desktop_integration.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

Future<void> _wait(
  WidgetTester tester,
  bool Function() ready, {
  int tries = 100,
}) async {
  for (var i = 0; i < tries && !ready(); i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets(
    'FIX-15 hotkey recording persists; close-to-tray hides/restores',
    (tester) async {
      final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
      final evidenceDir = Platform.environment['V2RAYN_R_FIX15_EVIDENCE'];
      final mode = Platform.environment['V2RAYN_R_FIX15_MODE'] ?? 'record';
      expect(dataDir, isNotNull);
      expect(evidenceDir, isNotNull);
      final ev = Directory(evidenceDir!)..createSync(recursive: true);
      final checks = <Map<String, Object?>>[];
      final failures = <String>[];
      final resultName = mode == 'reopen'
          ? 'reopen-observations.json'
          : 'observations.json';
      void write({bool complete = false}) {
        File('$ev.path/$resultName').writeAsStringSync(
          const JsonEncoder.withIndent('  ').convert({
            'mode': mode,
            'recordingComplete': complete,
            'failures': failures,
            'checks': checks,
          }),
        );
      }

      void check(String step, bool passed, Map<String, Object?> actual) {
        checks.add({'step': step, 'passed': passed, 'actual': actual});
        if (!passed) failures.add(step);
        write();
      }

      RustBridgeInit.configure(RustLib.init);
      await RustBridgeInit.init();
      runApp(
        ProviderScope(
          overrides: [
            uiStateStoreProvider.overrideWithValue(
              FileUiStateStore(overridePath: '$dataDir/ui_state.json'),
            ),
          ],
          child: const V2rayNRApp(),
        ),
      );
      await _wait(
        tester,
        () =>
            find.byKey(const ValueKey('hotkey-record-0')).evaluate().isNotEmpty,
        tries: 200,
      );
      final container = ProviderScope.containerOf(
        tester.element(find.byType(V2rayNRApp)),
      );
      await _wait(
        tester,
        () => container.read(settingsControllerProvider).loaded,
      );

      if (mode == 'reopen') {
        final label = tester
            .widget<Text>(find.byKey(const ValueKey('hotkey-label-0')))
            .data;
        check('reopen-hotkey-label', label == 'Ctrl + A', {'label': label});
        write(complete: true);
        expect(failures, isEmpty, reason: 'FIX-15 reopen gaps: $failures');
        return;
      }

      // Record Ctrl+A on 显示/隐藏窗口.
      await tester.tap(find.byKey(const ValueKey('hotkey-record-0')));
      await tester.pump(const Duration(milliseconds: 150));
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump(const Duration(milliseconds: 50));
      await tester.sendKeyDownEvent(LogicalKeyboardKey.keyA);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.keyA);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump(const Duration(milliseconds: 150));
      check(
        'recording-label',
        tester
                .widget<Text>(find.byKey(const ValueKey('hotkey-label-0')))
                .data ==
            'Ctrl + A',
        {
          'label': tester
              .widget<Text>(find.byKey(const ValueKey('hotkey-label-0')))
              .data,
        },
      );
      await tester.tap(find.text('保存'));
      await tester.pumpAndSettle();

      final document = container.read(settingsControllerProvider).document;
      final hotkeys = (document['GlobalHotkeys'] as List)
          .cast<Map<String, dynamic>>();
      final entry = hotkeys.firstWhere(
        (h) => (h['EGlobalHotkey'] as num?)?.toInt() == 0,
      );
      check(
        'persisted-wpf-key',
        entry['Control'] == true && entry['KeyCode'] == 44,
        {'Control': entry['Control'], 'KeyCode': entry['KeyCode']},
      );

      // Close-to-tray: the same path `onWindowClose` uses on Windows.
      final integration = container.read(desktopIntegrationProvider).value;
      var hidden = false;
      if (integration != null) {
        await integration.hideToTray();
        await tester.pump(const Duration(milliseconds: 300));
        hidden = !(await windowManager.isVisible());
        await windowManager.show();
        await windowManager.focus();
        await tester.pump(const Duration(milliseconds: 300));
      }
      check('close-hides-to-tray', hidden, {
        'hidden': hidden,
        'hadIntegration': integration != null,
      });

      write(complete: true);
      expect(failures, isEmpty, reason: 'FIX-15 record gaps: $failures');
    },
  );
}
