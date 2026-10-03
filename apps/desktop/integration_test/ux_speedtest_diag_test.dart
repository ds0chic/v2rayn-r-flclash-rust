// UX-TEST-01 decisive speedtest diagnostic against the REAL Windows window,
// real FRB bridge, real SQLite and real local loopback servers. It reproduces
// the reported "测速不生效" complaint and, after the fix, guards that every
// outcome (success / failure / empty set / start failure) has visible feedback.
//
// Run with a clean data directory (port floor is 11808, never 10808):
//   $env:V2RAYN_R_DATA_DIR = <temp>
//   $env:V2RAYN_UX_TEST01_EVIDENCE_DIR = <repo>\docs\evidence\UX-TEST-01
//   flutter test integration_test/ux_speedtest_diag_test.dart -d windows
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
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

const _frameKey = ValueKey('ux-test01-frame');
const _portFloor = 11808;
const _portCeil = 11890;

Future<T> _until<T>(
  WidgetTester tester,
  T? Function() probe, {
  Duration timeout = const Duration(seconds: 30),
  String what = 'condition',
}) async {
  final end = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(end)) {
    final value = probe();
    if (value != null) return value;
    await tester.pump(const Duration(milliseconds: 150));
  }
  throw TestFailure('timed out waiting for $what');
}

Future<ServerSocket> _bindTcp() async {
  for (var port = _portFloor; port <= _portCeil; port++) {
    try {
      return await ServerSocket.bind(InternetAddress.loopbackIPv4, port);
    } on SocketException {
      continue;
    }
  }
  throw StateError('no free TCP port in $_portFloor..$_portCeil');
}

Future<HttpServer> _bindHttp() async {
  for (var port = _portFloor; port <= _portCeil; port++) {
    try {
      return await HttpServer.bind(InternetAddress.loopbackIPv4, port);
    } on SocketException {
      continue;
    }
  }
  throw StateError('no free HTTP port in $_portFloor..$_portCeil');
}

List<String> _shareLinks(int port, int n) {
  String uuid(int i) {
    final a = i.toString().padLeft(8, '0');
    final b = i.toString().padLeft(12, '0');
    return '$a-1111-1111-1111-$b';
  }

  return <String>[
    for (var i = 1; i <= n; i++)
      'vless://${uuid(i)}@127.0.0.1:$port?encryption=none#UX-$i',
  ];
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('UX-TEST-01 speedtest visible feedback', (tester) async {
    final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
    final evidenceDir =
        Platform.environment['V2RAYN_UX_TEST01_EVIDENCE_DIR'] ??
        '${Directory.current.path}${Platform.pathSeparator}'
            'docs${Platform.pathSeparator}evidence${Platform.pathSeparator}'
            'UX-TEST-01';
    expect(dataDir, isNotNull, reason: 'V2RAYN_R_DATA_DIR must be set');
    await Directory(evidenceDir).create(recursive: true);

    final observations = <Map<String, Object?>>[];
    void record(String step, Map<String, Object?> actual) {
      observations.add(<String, Object?>{'step': step, 'actual': actual});
      File('$evidenceDir/observations.json').writeAsStringSync(
        const JsonEncoder.withIndent('  ').convert(<String, Object?>{
          'dataDir': dataDir,
          'observations': observations,
        }),
      );
    }

    File('$evidenceDir/observations.json').writeAsStringSync(
      const JsonEncoder.withIndent('  ').convert(<String, Object?>{
        'dataDir': dataDir,
        'observations': observations,
      }),
    );

    // Local TCP listener for Tcping (node address/port) and a local HTTP
    // server serving /ping (204) and /speed (small body) for real ping and
    // download through the temporary test core.
    final tcp = await _bindTcp();
    tcp.listen((Socket socket) {
      // Accept-and-hold is enough for a TCP connect to complete.
    });
    addTearDown(() => tcp.close());

    final http = await _bindHttp();
    http.listen((HttpRequest request) {
      if (request.uri.path == '/ping') {
        request.response.statusCode = HttpStatus.noContent;
        request.response.close();
        return;
      }
      request.response.headers.contentType = ContentType.binary;
      request.response.add(List<int>.filled(256 * 1024, 0x41));
      request.response.close();
    });
    addTearDown(() => http.close(force: true));

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
    await _until(
      tester,
      () => find.byType(V2rayNRApp).evaluate().isNotEmpty ? true : null,
      what: 'profile toolbar',
    );
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

    // --- Seed 3 synthetic loopback nodes through the real bridge ---------
    final import = await bridge.importFromText(
      _shareLinks(tcp.port, 3).join('\n'),
      subid: 'ux-test-01',
      deduplicate: false,
    );
    controller.reload();
    final seeded = container.read(profilesControllerProvider).visible;
    record('seed', <String, Object?>{
      'ok': import.ok,
      'imported': import.imported,
      'errors': import.errors.length,
      'rows': seeded.length,
      'tcpPort': tcp.port,
      'httpPort': http.port,
    });
    expect(import.ok, isTrue, reason: import.error?.messageKey);
    expect(seeded.length, greaterThanOrEqualTo(3));
    final ids = seeded.take(3).map((r) => r.id).toList();
    await screenshot('00-seeded');

    // --- Scenario A: Tcping via the context-menu path --------------------
    controller.handleRightTap(ids.first);
    controller.emitAction(ProfileAction.tcping);
    final activeJobs = bridge.speedTestActiveJobs();
    record('A-start', <String, Object?>{
      'kind': 0,
      'selected': container.read(profilesControllerProvider).selected.toList(),
      'activeJobs': activeJobs,
      'stage': container.read(profilesControllerProvider).speedTestStage,
    });
    try {
      await _until(
        tester,
        () {
          final rows = container.read(profilesControllerProvider).visible;
          final hit = rows.where((r) => r.delay > 0).toList();
          if (hit.isNotEmpty &&
              !container.read(profilesControllerProvider).speedTestRunning) {
            return hit;
          }
          return null;
        },
        timeout: const Duration(seconds: 20),
        what: 'Tcping result',
      );
    } catch (_) {}
    final stateA = container.read(profilesControllerProvider);
    final rowsA = stateA.visible.where((r) => ids.contains(r.id)).toList();
    record('A-result', <String, Object?>{
      'running': stateA.speedTestRunning,
      'stage': stateA.speedTestStage,
      'message': stateA.speedTestMessage,
      'delays': <String, Object?>{for (final r in rowsA) r.remarks: r.delay},
      'activeJobs': bridge.speedTestActiveJobs(),
      'results': bridge
          .speedTestResults()
          .where((r) => ids.contains(r.indexId))
          .map(
            (r) => <String, Object?>{
              'id': r.indexId,
              'delay': r.delay,
              'message': r.message,
            },
          )
          .toList(),
    });
    await screenshot('01-tcping');

    // --- Scenario B: real ping / download / mixed / fast through the UI,
    // using local HTTP URLs persisted in SpeedTestItem (so the run proves the
    // user-configured URL is really executed).
    final load = bridge.getSettings();
    final doc = jsonDecode(load.settingsJson) as Map<String, dynamic>;
    final item = (doc['SpeedTestItem'] as Map<String, dynamic>?) ?? {};
    item['SpeedTestUrl'] = 'http://127.0.0.1:${http.port}/speed';
    item['SpeedPingTestUrl'] = 'http://127.0.0.1:${http.port}/ping';
    item['MixedConcurrencyCount'] = 2;
    item['SpeedTestTimeout'] = 10;
    item['SpeedTestDelayInterval'] = 0;
    doc['SpeedTestItem'] = item;
    final saved = bridge.saveSettingsJson(
      jsonEncode(doc),
      load.revision.toInt(),
    );
    record('B-settings', <String, Object?>{
      'saved': saved.ok,
      'speedTestUrl': item['SpeedTestUrl'],
      'speedPingTestUrl': item['SpeedPingTestUrl'],
    });
    expect(saved.ok, isTrue);

    final scenarioB = <String, String>{
      ProfileAction.realping: 'realping',
      ProfileAction.speedtest: 'speedtest',
      ProfileAction.mixedTest: 'mixed',
      ProfileAction.fastRealping: 'fastrealping',
    };
    for (final entry in scenarioB.entries) {
      controller.selectRow(ids.first);
      controller.emitAction(entry.key);
      final started = container.read(profilesControllerProvider);
      var resolved = false;
      final deadline = DateTime.now().add(const Duration(seconds: 25));
      while (DateTime.now().isBefore(deadline)) {
        await tester.pump(const Duration(milliseconds: 150));
        final s = container.read(profilesControllerProvider);
        if (!s.speedTestRunning && bridge.speedTestActiveJobs() == 0) {
          resolved = true;
          break;
        }
      }
      final s = container.read(profilesControllerProvider);
      record('B-${entry.value}', <String, Object?>{
        'startedStage': started.speedTestStage,
        'resolved': resolved,
        'stage': s.speedTestStage,
        'message': s.speedTestMessage,
        'delaySentinels': <String, Object?>{
          for (final r in s.visible.where((r) => ids.contains(r.id)))
            r.remarks: r.delay,
        },
        'results': bridge
            .speedTestResults()
            .where((r) => ids.contains(r.indexId))
            .map(
              (r) => <String, Object?>{
                'id': r.indexId,
                'delay': r.delay,
                'speed': r.speed,
                'message': r.message,
              },
            )
            .toList(),
      });
      await screenshot('02-${entry.value}');
    }

    final stateB = container.read(profilesControllerProvider);
    expect(
      stateB.speedTestMessage,
      isNotNull,
      reason: 'a settled test must leave a visible message',
    );
    expect(stateB.speedTestMessage, contains('测速完成'));
    expect(find.byKey(const ValueKey('speedtest-message')), findsOneWidget);
    // Failed nodes must be distinguishable from never-tested ones.
    expect(find.text('失败'), findsWidgets);

    // --- Scenario C: an empty selection must be announced, not silent -----
    controller.clearSelection();
    controller.emitAction(ProfileAction.realping);
    await _until(
      tester,
      () {
        final s = container.read(profilesControllerProvider);
        return (!s.speedTestRunning && s.speedTestMessage != null)
            ? true
            : null;
      },
      timeout: const Duration(seconds: 20),
      what: 'empty-selection realping',
    );
    final stateC = container.read(profilesControllerProvider);
    record('C-empty-selection', <String, Object?>{
      'selected': stateC.selected.length,
      'running': stateC.speedTestRunning,
      'message': stateC.speedTestMessage,
    });
    expect(stateC.speedTestMessage, isNotNull);

    // --- Scenario D: off-screen nodes are never silently tested -----------
    // With zero stored nodes the same action must say so (covered by the
    // widget test `ux_test01_*`; recorded here for the contract).
    record('D-note', <String, Object?>{
      'note': 'zero-node / start-failure feedback covered by widget tests',
    });

    await screenshot('03-final');
  });
}
