import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/app.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/perf/t18_bench.dart';

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
            ]
          : const [],
      child: const V2rayNRApp(),
    ),
  );
}
