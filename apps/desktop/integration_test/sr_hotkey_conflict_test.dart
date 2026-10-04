// SR-HOTKEY-CONFLICT: real OS proof that an externally-owned combination is
// reported as a conflict instead of faked as success.
//
// Opt-in only. Without `V2RAYN_R_REAL_HOTKEY=1` the test is SKIPPED, so the
// default `flutter test` never registers a real OS hotkey.
//
// Environment:
//   V2RAYN_R_REAL_HOTKEY=1                          opt-in switch (required)
//   V2RAYN_R_DATA_DIR=<isolated dir>                isolated engine/config dir
//   V2RAYN_R_HOTKEY_CONFLICT_EVIDENCE=<dir>         observations.json output dir
//   V2RAYN_R_REAL_HOTKEY_SCRIPT=<path>              optional helper override
//
// Flow: a helper process holds Ctrl+Alt+F11 -> the app's own user32 probe sees
// ERROR_HOTKEY_ALREADY_REGISTERED and the save reports the conflict (window
// stays open) -> the holder releases -> the app registers and dispatches
// display/hide via real SendInput -> both unregister -> the combination is free.
// Only the harmless show/hide action and the allowed Ctrl+Alt+F11 combo are
// used. No system proxy, registry, routing or TUN is touched.
import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hotkey_manager/hotkey_manager.dart';
import 'package:integration_test/integration_test.dart';
import 'package:window_manager/window_manager.dart';
import 'package:v2rayn_desktop/app/app.dart';
import 'package:v2rayn_desktop/app/shell/desktop_integration.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/hotkeys.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

const String _combo = 'ctrl+alt+f11';
const int _wpfF11 = 100;

Future<void> _pumpUntil(
  WidgetTester tester,
  bool Function() ready, {
  int tries = 150,
  int stepMs = 200,
}) async {
  for (var i = 0; i < tries && !ready(); i++) {
    await tester.pump(Duration(milliseconds: stepMs));
  }
}

Future<bool> _pumpUntilAsync(
  WidgetTester tester,
  Future<bool> Function() ready, {
  int tries = 25,
  int stepMs = 200,
}) async {
  for (var i = 0; i < tries; i++) {
    if (await ready()) return true;
    await tester.pump(Duration(milliseconds: stepMs));
  }
  return await ready();
}

String _resolveSendInputScript() {
  final fromEnv = Platform.environment['V2RAYN_R_REAL_HOTKEY_SCRIPT'];
  if (fromEnv != null && File(fromEnv).existsSync()) return fromEnv;
  var dir = Directory.current;
  for (var i = 0; i < 8; i++) {
    final candidate = File(
      '${dir.path}${Platform.pathSeparator}tools'
      '${Platform.pathSeparator}real_hotkey_sendinput.ps1',
    );
    if (candidate.existsSync()) return candidate.path;
    final parent = dir.parent;
    if (parent.path == dir.path) break;
    dir = parent;
  }
  throw StateError(
    'tools/real_hotkey_sendinput.ps1 not found from ${Directory.current.path}',
  );
}

Future<ProcessResult> _runHelper(String script, List<String> args) =>
    Process.run('powershell.exe', <String>[
      '-NoProfile',
      '-ExecutionPolicy',
      'Bypass',
      '-File',
      script,
      ...args,
    ]);

/// Start an external owner of the combination; resolves once it reports held.
Future<Process> _startHolder(String script) async {
  final process = await Process.start('powershell.exe', <String>[
    '-NoProfile',
    '-ExecutionPolicy',
    'Bypass',
    '-File',
    script,
    '-Combo',
    _combo,
    '-Hold',
  ]);
  final ready = Completer<bool>();
  final stderrBuf = StringBuffer();
  process.stdout.transform(utf8.decoder).transform(const LineSplitter()).listen(
    (line) {
      if (ready.isCompleted) return;
      if (line.contains('"held":true')) ready.complete(true);
      if (line.contains('"held":false')) ready.complete(false);
    },
  );
  process.stderr.transform(utf8.decoder).listen(stderrBuf.write);
  final ok = await ready.future.timeout(
    const Duration(seconds: 25),
    onTimeout: () => false,
  );
  if (!ok) {
    process.kill();
    throw StateError('helper failed to hold $_combo: ${stderrBuf.toString()}');
  }
  return process;
}

/// Release the external owner by closing its stdin and waiting for exit.
Future<void> _releaseHolder(Process process) async {
  try {
    await process.stdin.close();
  } on Object {
    // already closed
  }
  await process.exitCode.timeout(
    const Duration(seconds: 15),
    onTimeout: () {
      process.kill();
      return -1;
    },
  );
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  final optIn = Platform.environment['V2RAYN_R_REAL_HOTKEY'] == '1';

  testWidgets(
    'SR-HOTKEY-CONFLICT occupied combo is reported, then registers after release',
    (tester) async {
      final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
      final evidenceDir =
          Platform.environment['V2RAYN_R_HOTKEY_CONFLICT_EVIDENCE'];
      expect(dataDir, isNotNull, reason: 'V2RAYN_R_DATA_DIR must be set');
      expect(
        evidenceDir,
        isNotNull,
        reason: 'V2RAYN_R_HOTKEY_CONFLICT_EVIDENCE must be set',
      );
      final ev = Directory(evidenceDir!)..createSync(recursive: true);
      final checks = <Map<String, Object?>>[];
      final failures = <String>[];
      void write({bool complete = false}) {
        File('${ev.path}/observations.json').writeAsStringSync(
          const JsonEncoder.withIndent('  ').convert(<String, Object?>{
            'task': 'SR-HOTKEY-CONFLICT',
            'combo': 'Ctrl+Alt+F11',
            'action': 'showForm (显示/隐藏窗口)',
            'realHotkey': true,
            'recordingComplete': complete,
            'failures': failures,
            'checks': checks,
          }),
        );
      }

      void check(String step, bool passed, Map<String, Object?> actual) {
        checks.add(<String, Object?>{
          'step': step,
          'passed': passed,
          'actual': actual,
        });
        if (!passed) failures.add(step);
        write();
      }

      final script = _resolveSendInputScript();
      Process? holder;
      addTearDown(() {
        try {
          holder?.kill();
        } on Object {
          // ignore
        }
      });

      // Baseline: free before anyone registers.
      final baseline = await _runHelper(script, <String>[
        '-Combo',
        _combo,
        '-Probe',
      ]);
      check('os-combo-free-before', baseline.exitCode == 0, <String, Object?>{
        'exit': baseline.exitCode,
        'stdout': (baseline.stdout as String).trim(),
        'stderr': (baseline.stderr as String).trim(),
      });

      // External owner holds the combination for the whole conflict phase.
      holder = await _startHolder(script);
      final probeWhileHeld = await _runHelper(script, <String>[
        '-Combo',
        _combo,
        '-Probe',
      ]);
      check(
        'os-combo-occupied-externally',
        probeWhileHeld.exitCode == 2 &&
            (probeWhileHeld.stdout as String).contains('1409'),
        <String, Object?>{
          'exit': probeWhileHeld.exitCode,
          'stdout': (probeWhileHeld.stdout as String).trim(),
          'stderr': (probeWhileHeld.stderr as String).trim(),
        },
      );

      // Seed only the proxy mode (Unchanged) so the launch restore is read-only.
      final dir = Directory(dataDir!)..createSync(recursive: true);
      final config = File(
        '${dir.path}${Platform.pathSeparator}guiNConfig.json',
      );
      config.writeAsStringSync(
        const JsonEncoder.withIndent('  ').convert(<String, Object?>{
          'SystemProxyItem': <String, Object?>{
            'SysProxyType': 2,
            'NotProxyLocalAddress': true,
          },
        }),
      );

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

      await _pumpUntil(tester, () {
        final elements = find.byType(V2rayNRApp).evaluate();
        return elements.isNotEmpty;
      }, tries: 200);
      final container = ProviderScope.containerOf(
        tester.element(find.byType(V2rayNRApp)),
      );
      await _pumpUntil(
        tester,
        () => container.read(settingsControllerProvider).loaded,
      );
      await _pumpUntil(
        tester,
        () => container.read(desktopIntegrationProvider).value != null,
      );

      final settings = container.read(settingsControllerProvider.notifier);
      final controller = container.read(hotkeyControllerProvider.notifier);
      final dispatch = container.read(hotkeyDispatchProvider);
      final original = dispatch.handler;
      final fired = <GlobalHotkeyAction>[];
      dispatch.handler = (action) {
        fired.add(action);
        original?.call(action);
      };

      const entry = <String, dynamic>{
        'EGlobalHotkey': 0,
        'Alt': true,
        'Control': true,
        'Shift': false,
        'KeyCode': _wpfF11,
      };
      final saved = settings.saveGroup('GlobalHotkeys', <Object?>[entry]);
      final okConflict = await controller.save(<HotkeyBinding>[
        HotkeyBinding.fromSettings(entry),
      ], () => saved.ok);
      final conflictState = container.read(hotkeyControllerProvider);
      check(
        'conflict-save-reports-occupied',
        !okConflict &&
            conflictState.registered.isEmpty &&
            conflictState.conflicts.isNotEmpty &&
            conflictState.conflicts.any((c) => c.contains('组合已被其它程序占用')),
        <String, Object?>{
          'ok': okConflict,
          'registered': conflictState.registered.map((a) => a.name).toList(),
          'conflicts': conflictState.conflicts,
          'isPaused': controller.isPaused,
        },
      );

      // Release the external owner; the combination must be free again.
      expect(holder, isNotNull);
      await _releaseHolder(holder);
      holder = null;
      final probeAfterRelease = await _runHelper(script, <String>[
        '-Combo',
        _combo,
        '-Probe',
      ]);
      check(
        'os-combo-free-after-release',
        probeAfterRelease.exitCode == 0,
        <String, Object?>{
          'exit': probeAfterRelease.exitCode,
          'stdout': (probeAfterRelease.stdout as String).trim(),
          'stderr': (probeAfterRelease.stderr as String).trim(),
        },
      );

      // Same binding now registers for real.
      final okRegistered = await controller.save(<HotkeyBinding>[
        HotkeyBinding.fromSettings(entry),
      ], () => saved.ok);
      final registeredState = container.read(hotkeyControllerProvider);
      check(
        'release-then-save-registers',
        okRegistered &&
            registeredState.conflicts.isEmpty &&
            registeredState.registered.contains(GlobalHotkeyAction.showForm),
        <String, Object?>{
          'ok': okRegistered,
          'registered': registeredState.registered.map((a) => a.name).toList(),
          'conflicts': registeredState.conflicts,
        },
      );

      await windowManager.show();
      await windowManager.focus();
      await tester.pump(const Duration(milliseconds: 300));
      final visibleBefore = await windowManager.isVisible();
      final send = await _runHelper(script, <String>['-Combo', _combo]);
      await _pumpUntil(
        tester,
        () => fired.contains(GlobalHotkeyAction.showForm),
        tries: 25,
      );
      final hidden = await _pumpUntilAsync(
        tester,
        () async => !(await windowManager.isVisible()),
      );
      check(
        'real-key-dispatches-showform',
        send.exitCode == 0 &&
            fired.contains(GlobalHotkeyAction.showForm) &&
            visibleBefore &&
            hidden,
        <String, Object?>{
          'sendExit': send.exitCode,
          'fired': fired.map((a) => a.name).toList(),
          'visibleBefore': visibleBefore,
          'hidden': hidden,
        },
      );

      // Unregister the app's hotkey and prove the OS combination is free.
      if (await windowManager.isVisible() == false) {
        await windowManager.show();
        await tester.pump(const Duration(milliseconds: 200));
      }
      await controller.unregisterAll();
      dispatch.handler = original;
      check(
        'plugin-registry-empty',
        HotKeyManager.instance.registeredHotKeyList.isEmpty,
        <String, Object?>{
          'registeredList': HotKeyManager.instance.registeredHotKeyList.length,
        },
      );
      final probeFinal = await _runHelper(script, <String>[
        '-Combo',
        _combo,
        '-Probe',
      ]);
      check('os-combo-free-after', probeFinal.exitCode == 0, <String, Object?>{
        'exit': probeFinal.exitCode,
        'stdout': (probeFinal.stdout as String).trim(),
        'stderr': (probeFinal.stderr as String).trim(),
      });

      write(complete: true);
      expect(failures, isEmpty, reason: 'SR-HOTKEY-CONFLICT gaps: $failures');
    },
    skip: !optIn,
  );
}
