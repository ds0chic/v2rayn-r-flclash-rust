// SR-REAL-HOTKEY: real OS global-hotkey dispatch proof for SR-05 / SR-06.
//
// Opt-in only. Without `V2RAYN_R_REAL_HOTKEY=1` the test is SKIPPED, so the
// default `flutter test` never registers a real OS hotkey.
//
// Environment:
//   V2RAYN_R_REAL_HOTKEY=1                     opt-in switch (required)
//   V2RAYN_R_DATA_DIR=<isolated dir>           isolated engine/SQLite/config dir
//   V2RAYN_R_REAL_HOTKEY_EVIDENCE=<dir>        observations.json output dir
//   V2RAYN_R_REAL_HOTKEY_SCRIPT=<path>         optional override of the helper
//
// The only combination exercised is Ctrl+Alt+F12 bound to the harmless
// "显示/隐藏窗口" action. The helper injects real keys through user32
// SendInput and probes RegisterHotKey/UnregisterHotKey for OS-level residue.
// No system proxy, registry, routing or TUN is touched.
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

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  final optIn = Platform.environment['V2RAYN_R_REAL_HOTKEY'] == '1';

  testWidgets(
    'SR-REAL-HOTKEY Ctrl+Alt+F12 dispatches show-window through real OS hotkey',
    (tester) async {
      final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
      final evidenceDir = Platform.environment['V2RAYN_R_REAL_HOTKEY_EVIDENCE'];
      expect(dataDir, isNotNull, reason: 'V2RAYN_R_DATA_DIR must be set');
      expect(
        evidenceDir,
        isNotNull,
        reason: 'V2RAYN_R_REAL_HOTKEY_EVIDENCE must be set',
      );
      final ev = Directory(evidenceDir!)..createSync(recursive: true);
      final checks = <Map<String, Object?>>[];
      final failures = <String>[];
      void write({bool complete = false}) {
        File('${ev.path}/observations.json').writeAsStringSync(
          const JsonEncoder.withIndent('  ').convert(<String, Object?>{
            'task': 'SR-REAL-HOTKEY',
            'combo': 'Ctrl+Alt+F12',
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

      // Baseline: the combination must be free before the app registers it.
      final baseline = await _runHelper(script, <String>[
        '-Combo',
        'ctrl+alt+f12',
        '-Probe',
      ]);
      check('os-combo-free-before', baseline.exitCode == 0, <String, Object?>{
        'exit': baseline.exitCode,
        'stdout': (baseline.stdout as String).trim(),
        'stderr': (baseline.stderr as String).trim(),
      });

      // Seed only the proxy mode (Unchanged) so the launch restore is read-only
      // and never writes the host system proxy. The hotkey itself is written
      // through the real settings bridge below.
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
        if (elements.isEmpty) return false;
        return true;
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

      // Real settings bridge write of the binding, then the shared save path
      // re-registers it exactly like the 「全局热键」window's 保存 button.
      const entry = <String, dynamic>{
        'EGlobalHotkey': 0,
        'Alt': true,
        'Control': true,
        'Shift': false,
        'KeyCode': 101, // System.Windows.Input.Key.F12
      };
      final saved = settings.saveGroup('GlobalHotkeys', <Object?>[entry]);
      final ok = await controller.save(<HotkeyBinding>[
        HotkeyBinding.fromSettings(entry),
      ], () => saved.ok);
      final state = container.read(hotkeyControllerProvider);
      check('bridge-write-and-save', ok && saved.ok, <String, Object?>{
        'ok': ok,
        'saved': saved.ok,
      });
      check(
        'registered-showform',
        state.registered.contains(GlobalHotkeyAction.showForm) &&
            state.conflicts.isEmpty,
        <String, Object?>{
          'registered': state.registered.map((a) => a.name).toList(),
          'conflicts': state.conflicts,
        },
      );

      // The window must start visible so the toggle is observable.
      await windowManager.show();
      await windowManager.focus();
      await tester.pump(const Duration(milliseconds: 300));
      final visibleBefore = await windowManager.isVisible();
      check('window-visible-before', visibleBefore, <String, Object?>{
        'visible': visibleBefore,
      });

      // Inject real Ctrl+Alt+F12 through the OS input queue.
      final send = await _runHelper(script, <String>['-Combo', 'ctrl+alt+f12']);
      check('sendinput-1', send.exitCode == 0, <String, Object?>{
        'exit': send.exitCode,
        'stdout': (send.stdout as String).trim(),
        'stderr': (send.stderr as String).trim(),
      });

      await _pumpUntil(
        tester,
        () => fired.contains(GlobalHotkeyAction.showForm),
        tries: 25,
      );
      check(
        'callback-fired-showform',
        fired.contains(GlobalHotkeyAction.showForm),
        <String, Object?>{'fired': fired.map((a) => a.name).toList()},
      );

      final hidden = await _pumpUntilAsync(
        tester,
        () async => !(await windowManager.isVisible()),
      );
      check('window-hidden-after-press', hidden, <String, Object?>{
        'hidden': hidden,
        'fired': fired.map((a) => a.name).toList(),
      });

      // Unregister and prove no OS/plugin residue.
      await windowManager.show();
      await tester.pump(const Duration(milliseconds: 200));
      await controller.unregisterAll();
      dispatch.handler = original;
      final registryEmpty = HotKeyManager.instance.registeredHotKeyList.isEmpty;
      check('plugin-registry-empty', registryEmpty, <String, Object?>{
        'registeredList': HotKeyManager.instance.registeredHotKeyList.length,
      });

      final probe = await _runHelper(script, <String>[
        '-Combo',
        'ctrl+alt+f12',
        '-Probe',
      ]);
      check('os-combo-free-after', probe.exitCode == 0, <String, Object?>{
        'exit': probe.exitCode,
        'stdout': (probe.stdout as String).trim(),
        'stderr': (probe.stderr as String).trim(),
      });

      final firedBefore = fired.length;
      final visibleBefore2 = await windowManager.isVisible();
      final send2 = await _runHelper(script, <String>[
        '-Combo',
        'ctrl+alt+f12',
      ]);
      await tester.pump(const Duration(seconds: 1));
      final visibleAfter2 = await windowManager.isVisible();
      check(
        'no-dispatch-after-unregister',
        send2.exitCode == 0 &&
            fired.length == firedBefore &&
            visibleAfter2 == visibleBefore2,
        <String, Object?>{
          'sendExit': send2.exitCode,
          'firedBefore': firedBefore,
          'firedAfter': fired.length,
          'visibleBefore': visibleBefore2,
          'visibleAfter': visibleAfter2,
        },
      );

      write(complete: true);
      expect(failures, isEmpty, reason: 'SR-REAL-HOTKEY gaps: $failures');
    },
    skip: !optIn,
  );
}
