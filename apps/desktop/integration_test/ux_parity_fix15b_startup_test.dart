// FIX-15B: real Windows window proof for `UiItem.AutoHideStartup`.
//
// Run modes (isolated data dir via V2RAYN_R_DATA_DIR):
//   V2RAYN_R_FIX15B_MODE=seed   (default): launch, persist AutoHideStartup=true
//     through the real settings engine, and assert the save succeeded.
//   V2RAYN_R_FIX15B_MODE=hidden: relaunch on the same data dir and assert the
//     window is hidden to the tray at startup (`windowManager.isVisible()` is
//     false) while the tray-backed app keeps running.
//
// No kernel is started, no port is bound, and the host proxy / Run key are
// never touched.
//
// STATUS: written but NOT run in FIX-15B (the round forbids
// `flutter build windows`, which `flutter test -d windows` needs). Tracked as
// the remaining `verified` gate for this card.
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:window_manager/window_manager.dart';
import 'package:v2rayn_desktop/app/app.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

Future<void> _wait(WidgetTester tester, bool Function() ready) async {
  for (var i = 0; i < 200 && !ready(); i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets('FIX-15B AutoHideStartup hides the window on reopen', (
    tester,
  ) async {
    final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
    final evidenceDir = Platform.environment['V2RAYN_R_FIX15B_EVIDENCE'];
    final mode = Platform.environment['V2RAYN_R_FIX15B_MODE'] ?? 'seed';
    expect(dataDir, isNotNull);
    expect(evidenceDir, isNotNull);
    final ev = Directory(evidenceDir!)..createSync(recursive: true);
    final checks = <Map<String, Object?>>[];
    final failures = <String>[];
    void write({bool complete = false}) {
      File(
        '${ev.path}/${mode == 'hidden' ? 'hidden' : 'seed'}-observations.json',
      ).writeAsStringSync(
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
    runApp(const ProviderScope(child: V2rayNRApp()));
    final container = ProviderScope.containerOf(
      tester.element(find.byType(V2rayNRApp)),
    );
    await _wait(
      tester,
      () => container.read(settingsControllerProvider).loaded,
    );

    if (mode == 'hidden') {
      final visible = await windowManager.isVisible();
      check('startup-window-hidden', visible == false, {'visible': visible});
      write(complete: true);
      expect(failures, isEmpty, reason: 'FIX-15B hidden gaps: $failures');
      return;
    }

    final controller = container.read(settingsControllerProvider.notifier);
    final draft = controller.draft();
    final ui = draft['UiItem'];
    if (ui is Map<String, dynamic>) {
      ui['AutoHideStartup'] = true;
    } else {
      draft['UiItem'] = <String, dynamic>{'AutoHideStartup': true};
    }
    final result = controller.saveDocument(draft);
    check('persist-auto-hide-startup', result.ok, {
      'ok': result.ok,
      'revision': container.read(settingsControllerProvider).revision,
    });
    write(complete: true);
    expect(failures, isEmpty, reason: 'FIX-15B seed gaps: $failures');
  });
}
