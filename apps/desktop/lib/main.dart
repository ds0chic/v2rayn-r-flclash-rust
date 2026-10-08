import 'dart:io';

import 'package:flutter/semantics.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/app.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/routing/routing_windows.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window_entry.dart';
import 'package:v2rayn_desktop/perf/import_synth_hook.dart';
import 'package:v2rayn_desktop/perf/t18_bench.dart';

/// Keeps the armed-only forced-semantics handle alive for the whole process so
/// the Windows UIA bridge exposes the semantics tree to UI-automation clients.
SemanticsHandle? _forcedSemanticsHandle;

/// Armed-only UIA bridge switch: `V2RAYN_R_ENABLE_SEMANTICS=1` forces the
/// Flutter semantics tree on (needed because the packaged release exposes only
/// the FLUTTERVIEW pane to UIA otherwise). Unarmed/official builds ignore it.
bool get forceSemantics {
  if (!smokeArmed) return false;
  final v = Platform.environment['V2RAYN_R_ENABLE_SEMANTICS'];
  return v == '1' || v == 'true';
}

/// Compile-time arming flag for release-only evidence/benchmark hooks.
///
/// Packaged smoke/benchmark drivers must build with
/// `--dart-define=V2RAYN_R_SMOKE_ARMED=true`. Default release builds ignore
/// every V2RAYN_R_* automation environment variable that could change state
/// (ISSUE-08); `V2RAYN_R_OPEN_*` navigation hooks stay UI-only.
const bool smokeArmed = bool.fromEnvironment(
  'V2RAYN_R_SMOKE_ARMED',
  defaultValue: false,
);

/// Evidence/release-driver hook: when armed and true, [V2rayNRApp]'s
/// `_RuntimeBootstrap` applies the persisted runtime plan on launch, so a
/// packaged, clean-environment smoke can exercise the real
/// UI->Rust->net-host->core path without scripted clicks. Requires a seeded
/// data directory; a no-op in normal runs. Never sets the system proxy or a
/// fixed 10808 port.
bool get applyOnLaunch {
  if (!smokeArmed) return false;
  final v = Platform.environment['V2RAYN_R_AUTO_SMOKE'];
  return v == '1' || v == 'true';
}

Future<void> main() async {
  T18Bench.markMainEntry();
  WidgetsFlutterBinding.ensureInitialized();
  RustBridgeInit.configure(RustLib.init);
  await RustBridgeInit.init();
  // Armed-only UIA bridge: force the semantics tree so the Windows UIA client
  // can see stable selectors (AutomationId/Name/ControlType).
  if (forceSemantics && _forcedSemanticsHandle == null) {
    _forcedSemanticsHandle = SemanticsBinding.instance.ensureSemantics();
  }
  // Armed-only FLD-CFG-001/002 evidence hook: synthetic import via the real
  // path, then write import-result.json and exit. No-op unless the build is
  // armed AND V2RAYN_R_IMPORT_SYNTHETIC is set; unarmed builds ignore it.
  if (ImportSynthHook.enabled) {
    await ImportSynthHook.runAndExit();
    return;
  }
  if (T18Bench.enabled) {
    WidgetsBinding.instance.addPostFrameCallback(
      (_) => T18Bench.onFirstFrame(),
    );
  }
  runApp(
    ProviderScope(
      overrides: T18Bench.enabled
          ? [
              bridgePortProvider.overrideWithValue(
                T18BenchBridgePort(T18Bench.rows),
              ),
              profileRowCountProvider.overrideWithValue(T18Bench.rows),
              applyPlanOnLaunchProvider.overrideWithValue(true),
            ]
          : applyOnLaunch
          ? [applyPlanOnLaunchProvider.overrideWithValue(true)]
          : const [],
      child: const V2rayNRApp(),
    ),
  );
}

/// Entrypoint of the independent option settings window engine. The native
/// window host (`SettingsWindow`) creates a second Flutter engine with
/// `set_dart_entrypoint("settingsWindowMain")`. Must stay a top-level function
/// in this library to be reachable in AOT builds.
@pragma('vm:entry-point')
void settingsWindowMain() => runOptionSettingWindow();

/// Entrypoint of the independent routing settings window engine. The native
/// window host (`RoutingWindow`) creates a second Flutter engine with
/// `set_dart_entrypoint("routingWindowMain")`. Must stay a top-level function
/// in this library to be reachable in AOT builds.
@pragma('vm:entry-point')
void routingWindowMain() => runRoutingWindow();
