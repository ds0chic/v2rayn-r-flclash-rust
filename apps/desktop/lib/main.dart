import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/app.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/perf/t18_bench.dart';

/// Evidence/release-driver hook: when true, [V2rayNRApp]'s `_RuntimeBootstrap`
/// applies the persisted runtime plan on launch, so a packaged, clean-
/// environment smoke can exercise the real UI->Rust->net-host->core path
/// without scripted clicks. Requires a seeded data directory; a no-op in normal
/// runs. Never sets the system proxy or a fixed 10808 port.
bool get applyOnLaunch {
  final v = Platform.environment['V2RAYN_R_AUTO_SMOKE'];
  return v == '1' || v == 'true';
}

Future<void> main() async {
  T18Bench.markMainEntry();
  WidgetsFlutterBinding.ensureInitialized();
  RustBridgeInit.configure(RustLib.init);
  await RustBridgeInit.init();
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
