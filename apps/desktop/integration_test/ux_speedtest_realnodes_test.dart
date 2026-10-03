// UX-TEST-01 real-node comparison. Uses an already-authorized data directory
// (a COPY of the user's temp `t21f_data`) and records only desensitized facts:
// node count, config-type histogram, per-action stage/delay/result code and
// elapsed time. Addresses, remarks, credentials and URLs are never written.
//
//   $env:V2RAYN_R_DATA_DIR = <copy of t21f_data>
//   $env:V2RAYN_UX_TEST01_REALNODE_EVIDENCE_DIR = <repo>\docs\evidence\UX-TEST-01
//   flutter test integration_test/ux_speedtest_realnodes_test.dart -d windows
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
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

const _frameKey = ValueKey('ux-realnodes-frame');

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('UX-TEST-01 real-node comparison', (tester) async {
    final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
    final evidenceDir =
        Platform.environment['V2RAYN_UX_TEST01_REALNODE_EVIDENCE_DIR'] ??
        '${Directory.current.path}${Platform.pathSeparator}'
            'docs${Platform.pathSeparator}evidence${Platform.pathSeparator}'
            'UX-TEST-01';
    expect(dataDir, isNotNull);
    await Directory(evidenceDir).create(recursive: true);

    final observations = <Map<String, Object?>>[];
    final only = Platform.environment['V2RAYN_UX_TEST01_ACTION'];
    void record(String step, Map<String, Object?> actual) {
      observations.add(<String, Object?>{'step': step, 'actual': actual});
      File('$evidenceDir/real-nodes${only == null ? '' : '-$only'}.json')
          .writeAsStringSync(
            const JsonEncoder.withIndent('  ').convert(<String, Object?>{
              'dataDir': dataDir,
              'observations': observations,
            }),
          );
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
        child: const RepaintBoundary(key: _frameKey, child: V2rayNRApp()),
      ),
    );
    final end = DateTime.now().add(const Duration(seconds: 30));
    while (DateTime.now().isBefore(end) &&
        find.byType(V2rayNRApp).evaluate().isEmpty) {
      await tester.pump(const Duration(milliseconds: 200));
    }
    final container = ProviderScope.containerOf(
      tester.element(find.byType(V2rayNRApp)),
    );
    final bridge = container.read(bridgePortProvider);
    final controller = container.read(profilesControllerProvider.notifier);

    Future<void> screenshot(String name) async {
      await tester.pump(const Duration(milliseconds: 200));
      final boundary = tester.renderObject<RenderRepaintBoundary>(
        find.byKey(_frameKey),
      );
      final image = await boundary.toImage(pixelRatio: 1);
      final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
      if (bytes != null) {
        await File('$evidenceDir/$name.png')
            .writeAsBytes(bytes.buffer.asUint8List());
      }
      image.dispose();
    }

    var state = container.read(profilesControllerProvider);
    final typeHistogram = <String, int>{};
    for (final r in state.all) {
      final key = r.configType.name;
      typeHistogram[key] = (typeHistogram[key] ?? 0) + 1;
    }
    record('inventory', <String, Object?>{
      'total': state.all.length,
      'visible': state.visible.length,
      'configTypes': typeHistogram,
    });
    if (state.all.isEmpty) {
      record('result', <String, Object?>{'note': 'no stored nodes'});
      return;
    }

    // Prefer a simple, directly testable node; fall back to the first row.
    const complexTypes = <ConfigType>{
      ConfigType.policyGroup,
      ConfigType.proxyChain,
      ConfigType.outbound,
      ConfigType.custom,
    };
    final simple = state.all
        .where((r) => !complexTypes.contains(r.configType))
        .toList();
    final target = (simple.isNotEmpty ? simple : state.all).first;
    controller.selectRow(target.id);

    Future<Map<String, Object?>> runAction(String action, int seconds) async {
      final started = DateTime.now();
      controller.emitAction(action);
      final deadline = DateTime.now().add(Duration(seconds: seconds));
      var cancelled = false;
      while (DateTime.now().isBefore(deadline)) {
        await tester.pump(const Duration(milliseconds: 200));
        if (!container.read(profilesControllerProvider).speedTestRunning &&
            bridge.speedTestActiveJobs() == 0) {
          break;
        }
      }
      if (container.read(profilesControllerProvider).speedTestRunning) {
        controller.cancelSpeedTest();
        cancelled = true;
        await tester.pump(const Duration(milliseconds: 400));
      }
      final s = container.read(profilesControllerProvider);
      final row = s.all.firstWhere((r) => r.id == target.id);
      final result = bridge
          .speedTestResults()
          .where((r) => r.indexId == target.id)
          .toList();
      return <String, Object?>{
        'configType': target.configType.name,
        'elapsedMs': DateTime.now().difference(started).inMilliseconds,
        'cancelled': cancelled,
        'stage': s.speedTestStage,
        'message': s.speedTestMessage,
        'delaySentinel': row.delay,
        'speed': row.speed,
        'resultCode': result.isEmpty ? null : result.first.message,
        'resultDelay': result.isEmpty ? null : result.first.delay,
      };
    }

    // Mixed/Fast expand to every stored node through the controller; scope the
    // real-node probe to the single target via the bridge so the run stays
    // bounded (39 nodes x temporary core churn is not useful for one sample).
    Future<Map<String, Object?>> runKindDirect(int kind, int seconds) async {
      final started = DateTime.now();
      final res = bridge.startSpeedTest(kind, <String>[target.id]);
      final deadline = DateTime.now().add(Duration(seconds: seconds));
      var cancelled = false;
      while (DateTime.now().isBefore(deadline)) {
        await tester.pump(const Duration(milliseconds: 200));
        if (bridge.speedTestActiveJobs() == 0) break;
      }
      if (bridge.speedTestActiveJobs() != 0) {
        if (res.jobId != null) bridge.cancelSpeedTest(res.jobId!);
        cancelled = true;
        await tester.pump(const Duration(milliseconds: 400));
      }
      final result = bridge
          .speedTestResults()
          .where((r) => r.indexId == target.id)
          .toList();
      return <String, Object?>{
        'configType': target.configType.name,
        'ok': res.ok,
        'elapsedMs': DateTime.now().difference(started).inMilliseconds,
        'cancelled': cancelled,
        'resultCode': result.isEmpty ? null : result.first.message,
        'resultDelay': result.isEmpty ? null : result.first.delay,
        'resultSpeed': result.isEmpty ? null : result.first.speed,
      };
    }

    Future<void> runOne(
      String name,
      Future<Map<String, Object?>> Function() action,
    ) async {
      if (only != null && only != name) return;
      record(name, await action());
      await screenshot('real-$name');
    }

    await runOne('tcping', () => runAction(ProfileAction.tcping, 20));
    await runOne('realping', () => runAction(ProfileAction.realping, 25));
    await runOne('download', () => runAction(ProfileAction.speedtest, 30));
    await runOne('mixed', () => runKindDirect(4, 30));
    await runOne('fast', () => runKindDirect(5, 30));
  });
}
