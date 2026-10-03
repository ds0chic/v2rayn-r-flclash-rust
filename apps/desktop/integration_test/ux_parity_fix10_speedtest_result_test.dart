// FIX-10: real Windows window + real FRB/Rust/SQLite proof of the primary
// user flow "选中测速 → 结果 → 按结果处理 → 重开".
//
// Flow (run modes):
//   V2RAYN_R_FIX10_MODE=run    (default) create a synthetic group with three
//     loopback VLESS nodes (two reachable, one with a dead port), select them,
//     run a Tcping test, verify the per-node results, then "按测试结果移除无效"
//     deletes the failed ProfileItem. Records the surviving ids.
//   V2RAYN_R_FIX10_MODE=reopen launch a fresh process on the same data dir and
//     assert the failed node is still gone and the two live nodes persist.
//
// No kernel is started, no port below 11808 is touched, and the reserved live
// proxy port 10808 is never used. The loopback TCP listener only accepts.
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:v2rayn_desktop/app/app.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

const _frame = ValueKey('fix10-frame');
const _portFloor = 11808;
const _portCeil = 11890;

Future<void> _settle(WidgetTester tester) =>
    tester.pump(const Duration(milliseconds: 300));

Future<void> _until(
  WidgetTester tester,
  bool Function() ready, {
  Duration timeout = const Duration(seconds: 25),
}) async {
  final end = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(end)) {
    if (ready()) return;
    await tester.pump(const Duration(milliseconds: 150));
  }
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

/// Reserve then release a loopback port so a connect attempt fails (dead port).
Future<int> _deadPort(ServerSocket live) async {
  for (var port = _portFloor; port <= _portCeil; port++) {
    if (port == live.port) continue;
    try {
      final probe = await ServerSocket.bind(InternetAddress.loopbackIPv4, port);
      await probe.close();
      return port;
    } on SocketException {
      continue;
    }
  }
  throw StateError('no dead port available');
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

c.SubItemDto _subDraft(String remarks) => c.SubItemDto(
  id: '',
  remarks: remarks,
  url: 'http://127.0.0.1:11998/not-requested',
  moreUrl: '',
  enabled: true,
  userAgent: '',
  sort: 1,
  autoUpdateInterval: 0,
  updateTime: 0,
);

String _uuid(int i) =>
    '${i.toString().padLeft(8, '0')}-1111-1111-1111-'
    '${i.toString().padLeft(12, '0')}';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('FIX-10 select-test-remove-invalid-reopen', (tester) async {
    final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
    final evidenceDir = Platform.environment['V2RAYN_R_FIX10_EVIDENCE'];
    final mode = Platform.environment['V2RAYN_R_FIX10_MODE'] ?? 'run';
    expect(dataDir, isNotNull);
    expect(evidenceDir, isNotNull);
    await Directory(evidenceDir!).create(recursive: true);

    final checks = <Map<String, Object?>>[];
    final failures = <String>[];
    final extra = <String, Object?>{'mode': mode};
    final resultName = mode == 'reopen'
        ? 'reopen-observations.json'
        : 'observations.json';
    void write({bool complete = false}) {
      File('$evidenceDir/$resultName').writeAsStringSync(
        const JsonEncoder.withIndent('  ').convert(<String, Object?>{
          'applicationCommit': 'c1c9c77',
          'recordingComplete': complete,
          ...extra,
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

    if (mode == 'reopen') {
      final previous = File('$evidenceDir/observations.json');
      final prior = previous.existsSync()
          ? jsonDecode(previous.readAsStringSync()) as Map
          : const <String, Object?>{};
      final survivedIds =
          (prior['survivedIds'] as List?)?.whereType<String>().toSet() ??
          const <String>{};
      final removedId = prior['removedId'] as String?;
      final subId = prior['subId'] as String?;

      await _until(tester, () => find.byType(V2rayNRApp).evaluate().isNotEmpty);
      final container = ProviderScope.containerOf(
        tester.element(find.byType(V2rayNRApp)),
      );
      final controller = container.read(profilesControllerProvider.notifier);
      controller.reload();
      await _settle(tester);
      final profiles = container.read(profilesControllerProvider).profiles;
      final present = profiles.map((p) => p.indexId).toSet();

      check(
        'reopen-removed-node-absent',
        removedId == null || !present.contains(removedId),
        <String, Object?>{
          'removedId': removedId,
          'present': present.contains(removedId),
        },
      );
      check(
        'reopen-survived-nodes-exist',
        survivedIds.isNotEmpty && survivedIds.every(present.contains),
        <String, Object?>{
          'survivedIds': survivedIds.toList(),
          'presentCount': survivedIds.where(present.contains).length,
          'subId': subId,
        },
      );
      await _shot(tester, evidenceDir, 'reopen-window');
      write(complete: true);
      expect(failures, isEmpty, reason: 'FIX-10 reopen gaps: $failures');
      return;
    }

    await _until(tester, () => find.byType(V2rayNRApp).evaluate().isNotEmpty);
    final container = ProviderScope.containerOf(
      tester.element(find.byType(V2rayNRApp)),
    );
    final bridge = container.read(bridgePortProvider);
    final controller = container.read(profilesControllerProvider.notifier);
    final subs = container.read(subsControllerProvider.notifier);

    final live = await _bindTcp();
    live.listen((Socket socket) {});
    addTearDown(() => live.close());
    final dead = await _deadPort(live);

    final saved = subs.save(_subDraft('FIX10合成订阅'));
    final subId = saved.item?.id ?? '';
    check(
      'subscription-created',
      saved.ok && subId.isNotEmpty,
      <String, Object?>{
        'subId': subId,
        'ok': saved.ok,
        'error': saved.error?.code,
      },
    );
    extra['subId'] = subId;

    final links = <String>[
      'vless://${_uuid(1)}@127.0.0.1:${live.port}?encryption=none#FIX10-LIVE-1',
      'vless://${_uuid(2)}@127.0.0.1:${live.port}?encryption=none#FIX10-LIVE-2',
      'vless://${_uuid(3)}@127.0.0.1:$dead?encryption=none#FIX10-DEAD',
    ].join('\n');
    final imported = await bridge.importFromText(links, subid: subId);
    controller.reload();
    controller.setGroupSubId(subId);
    await _settle(tester);
    final visible = container.read(profilesControllerProvider).visible;
    check(
      'nodes-imported-into-group',
      imported.ok && imported.imported == 3 && visible.length == 3,
      <String, Object?>{
        'imported': imported.imported,
        'visible': visible.length,
        'subId': subId,
        'error': imported.error?.code,
      },
    );
    if (visible.length < 3) {
      write(complete: true);
      expect(failures, isEmpty, reason: 'FIX-10 seed gaps: $failures');
      return;
    }
    final ids = visible.map((r) => r.id).toList();

    // Select all three and run a real Tcping (direct TCP, no kernel).
    controller.selectRow(ids[0]);
    for (final id in ids.skip(1)) {
      controller.selectRow(id, ctrl: true);
    }
    check(
      'selection-ready',
      container.read(profilesControllerProvider).selected.length == 3,
      <String, Object?>{
        'selected': container.read(profilesControllerProvider).selected.length,
      },
    );
    controller.emitAction(ProfileAction.tcping);
    await _until(tester, () {
      final s = container.read(profilesControllerProvider);
      return !s.speedTestRunning && bridge.speedTestActiveJobs() == 0;
    }, timeout: const Duration(seconds: 25));
    await _settle(tester);

    final results = <String, int>{
      for (final r in bridge.speedTestResults()) r.indexId: r.delay,
    };
    final liveResults = ids.take(2).map((id) => results[id] ?? 0).toList();
    final deadResult = results[ids[2]] ?? 0;
    check(
      'tcping-results-measured',
      liveResults.every((d) => d > 0) && deadResult == -1,
      <String, Object?>{
        'liveDelays': liveResults,
        'deadDelay': deadResult,
        'resultIds': results.keys.toList(),
      },
    );
    await _shot(tester, evidenceDir, '01-results');

    // "按测试结果移除无效": the failed ProfileItem is really deleted.
    final removedId = ids[2];
    final removed = controller.removeInvalidResults();
    await _settle(tester);
    final remaining = container
        .read(profilesControllerProvider)
        .profiles
        .where((p) => p.subid == subId)
        .map((p) => p.indexId)
        .toSet();
    final survivedIds = remaining.where((id) => id != removedId).toList();
    check(
      'remove-invalid-deletes-profile',
      removed == 1 && !remaining.contains(removedId) && survivedIds.length == 2,
      <String, Object?>{
        'removed': removed,
        'removedId': removedId,
        'remaining': remaining.toList(),
      },
    );
    extra['removedId'] = removedId;
    extra['survivedIds'] = survivedIds;
    await _shot(tester, evidenceDir, '02-after-remove-invalid');

    write(complete: true);
    expect(failures, isEmpty, reason: 'FIX-10 run gaps: $failures');
  });
}
