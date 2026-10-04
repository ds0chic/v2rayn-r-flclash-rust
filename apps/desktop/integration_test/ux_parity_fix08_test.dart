// FIX-08 first flow ("参数修改 → 应用"): real Windows window + real
// FRB/Rust/SQLite proof that editing a parameter then 取消 persists nothing
// while 应用 saves the visible draft; a separate reopen process sees the
// applied value.
//
// Run modes (sequential, same isolated data dir):
//   V2RAYN_R_FIX08_MODE=cancel  edit the local port, cancel, assert unchanged.
//   V2RAYN_R_FIX08_MODE=apply   edit the local port, apply, assert saved.
//   V2RAYN_R_FIX08_MODE=reopen  fresh process, assert the applied value stuck.
// No kernel is started by the test itself (apply may attempt the real plan
// with zero profiles; its structured outcome is recorded, never asserted as
// success). No autostart toggle is touched (no Run-key writes), no system
// proxy is modified, no port is bound (the port value is only stored).
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:v2rayn_desktop/app/app.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

const _frame = ValueKey('fix08-frame');

Future<void> _settle(WidgetTester tester) =>
    tester.pump(const Duration(milliseconds: 500));

Future<void> _wait(
  WidgetTester tester,
  bool Function() ready, {
  int tries = 100,
}) async {
  for (var i = 0; i < tries && !ready(); i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
}

Future<void> _shot(WidgetTester tester, String dir, String name) async {
  await _settle(tester);
  final boundary = tester.renderObject<RenderRepaintBoundary>(
    find.byKey(_frame),
  );
  final image = await boundary.toImage(pixelRatio: 1);
  final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
  if (bytes != null) {
    await File('$dir/$name.png').writeAsBytes(bytes.buffer.asUint8List());
  }
  image.dispose();
}

Finder _portField() => find.descendant(
  of: find.byKey(const ValueKey('settings-local-port')),
  matching: find.byType(TextField),
);

int _currentPort(ProviderContainer container) {
  final doc = container.read(settingsControllerProvider).document;
  final inbound = (doc['Inbound'] as List).first as Map;
  return (inbound['LocalPort'] as num).toInt();
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets('FIX-08 settings draft apply/cancel/reopen', (tester) async {
    final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
    final evidenceDir = Platform.environment['V2RAYN_R_FIX08_EVIDENCE'];
    final mode = Platform.environment['V2RAYN_R_FIX08_MODE'] ?? 'cancel';
    expect(dataDir, isNotNull);
    expect(evidenceDir, isNotNull);
    await Directory(evidenceDir!).create(recursive: true);
    final checks = <Map<String, Object?>>[];
    final failures = <String>[];
    final extra = <String, Object?>{'mode': mode};
    void write({bool complete = false}) {
      File('$evidenceDir/$mode-observations.json').writeAsStringSync(
        const JsonEncoder.withIndent('  ').convert({
          'recordingComplete': complete,
          ...extra,
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
        child: const RepaintBoundary(key: _frame, child: V2rayNRApp()),
      ),
    );
    await _wait(
      tester,
      () => find.byType(OptionSettingWindow).evaluate().isNotEmpty,
    );
    final container = ProviderScope.containerOf(
      tester.element(find.byType(V2rayNRApp)),
    );
    check(
      'settings-window-open',
      find.byType(OptionSettingWindow).evaluate().isNotEmpty,
      {},
    );

    if (mode == 'reopen') {
      final prior = File('$evidenceDir/apply-observations.json');
      var expected = 11822;
      if (prior.existsSync()) {
        final applied = jsonDecode(prior.readAsStringSync()) as Map;
        final checks0 = (applied['checks'] as List?) ?? const [];
        for (final entry in checks0) {
          final map = entry as Map;
          if (map['step'] == 'apply-saved-port') {
            expected = (map['actual'] as Map)['savedPort'] as int;
          }
        }
      }
      final port = _currentPort(container);
      check('reopen-port-persisted', port == expected, {
        'port': port,
        'expected': expected,
      });
      await _shot(tester, evidenceDir, 'reopen-window');
      write(complete: true);
      expect(failures, isEmpty, reason: 'FIX-08 reopen gaps: $failures');
      return;
    }

    final beforePort = _currentPort(container);
    final beforeRevision = container.read(settingsControllerProvider).revision;
    const probe = 11821;
    const applied = 11822;
    final target = mode == 'apply' ? applied : probe;
    check('baseline-read', beforePort > 0, {
      'port': beforePort,
      'revision': beforeRevision,
    });

    await tester.enterText(_portField(), '$target');
    await tester.pump();
    await _shot(tester, evidenceDir, '$mode-edited');

    if (mode == 'cancel') {
      await tester.tap(find.text('取消'));
      // Bounded pumps only: the live shell owns periodic timers, so
      // pumpAndSettle would never settle (same reason FIX-01 uses _settle).
      await _settle(tester);
      await _settle(tester);
      final port = _currentPort(container);
      final revision = container.read(settingsControllerProvider).revision;
      check(
        'cancel-window-closed',
        find.byType(OptionSettingWindow).evaluate().isEmpty,
        {},
      );
      check('cancel-port-unchanged', port == beforePort, {
        'port': port,
        'beforePort': beforePort,
      });
      check('cancel-revision-unchanged', revision == beforeRevision, {
        'revision': revision,
        'beforeRevision': beforeRevision,
      });
      await _shot(tester, evidenceDir, 'cancel-after');
      write(complete: true);
      expect(failures, isEmpty, reason: 'FIX-08 cancel gaps: $failures');
      return;
    }

    // apply mode: the dialog button (keyed), not the toolbar homonym.
    await tester.tap(find.byKey(const ValueKey('settings-save')));
    await _settle(tester);
    await _settle(tester);
    await _settle(tester);
    final port = _currentPort(container);
    final revision = container.read(settingsControllerProvider).revision;
    check(
      'apply-window-closed',
      find.byType(OptionSettingWindow).evaluate().isEmpty,
      {},
    );
    check('apply-saved-port', port == applied, {
      'savedPort': port,
      'expected': applied,
    });
    check('apply-revision-bumped', revision > beforeRevision, {
      'revision': revision,
      'beforeRevision': beforeRevision,
    });
    final runtime = container.read(runtimeControllerProvider);
    extra['applyRuntimeState'] = runtime.state;
    extra['applyRuntimeError'] = runtime.error?.messageKey;
    check('apply-no-crash', true, {
      'runtimeState': runtime.state,
      'runtimeError': runtime.error?.messageKey,
    });
    await _shot(tester, evidenceDir, 'apply-after');
    write(complete: true);
    expect(failures, isEmpty, reason: 'FIX-08 apply gaps: $failures');
  });
}
