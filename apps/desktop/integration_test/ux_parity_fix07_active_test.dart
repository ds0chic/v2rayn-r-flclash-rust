// FIX-07 / ACT-PROF-005: real Windows window + real FRB/Rust/SQLite proof that
// "节点表设为活动" persists across a reopen and that a normal launch restores
// the active node (upstream `MainWindowViewModel.Init` -> `Reload`), without
// any test environment variable arming the apply.
//
// Run modes:
//   V2RAYN_R_FIX07_MODE=activate  (default) imports one synthetic node, sets it
//     active through the real toolbar entry, and records the active id.
//   V2RAYN_R_FIX07_MODE=reopen    launches a fresh process on the same isolated
//     data dir and asserts the active node survives and a restore is attempted.
//
// The synthetic node is loopback-only (`127.0.0.1:11998`) and is never dialed;
// the core locator is not provided, so a restore attempt is expected to surface
// a structured error rather than bind any port.
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
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
import 'package:v2rayn_desktop/features/profiles/profiles_table.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

const _frame = ValueKey('fix07-frame');

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

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets('FIX-07 set active persists and restores on reopen', (
    tester,
  ) async {
    final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
    final evidenceDir = Platform.environment['V2RAYN_R_FIX07_EVIDENCE'];
    final mode = Platform.environment['V2RAYN_R_FIX07_MODE'] ?? 'activate';
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
        const JsonEncoder.withIndent('  ').convert({
          'applicationCommit': 'c1c9c77',
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
      () => find.byKey(const ValueKey('menu-配置项')).evaluate().isNotEmpty,
    );
    final container = ProviderScope.containerOf(
      tester.element(find.byType(V2rayNRApp)),
    );
    await _wait(
      tester,
      () => container.read(settingsControllerProvider).loaded,
    );

    if (mode == 'reopen') {
      final previous = File('$evidenceDir/observations.json');
      final prior = previous.existsSync()
          ? jsonDecode(previous.readAsStringSync()) as Map
          : const <String, Object?>{};
      final expectedId = prior['activeId'] as String?;
      final persisted = container.read(bridgePortProvider).getActiveProfile();
      check(
        'reopen-active-persisted',
        expectedId != null && persisted == expectedId,
        {
          'expectedActiveId': expectedId,
          'persistedActiveId': persisted,
          'controllerActiveId': container
              .read(profilesControllerProvider)
              .activeId,
        },
      );
      // Normal startup must attempt to restore the persisted active node.
      await _wait(tester, () {
        final view = container.read(runtimeControllerProvider);
        return view.isRunning || view.isBusy || view.error != null;
      }, tries: 200);
      final view = container.read(runtimeControllerProvider);
      final attempted = container
          .read(runtimeControllerProvider.notifier)
          .restoreAttempted;
      check('reopen-restore-attempted', attempted, {
        'restoreAttempted': attempted,
        'state': view.state,
        'error': view.error?.code,
        'ports': view.ports,
        'sessionId': view.sessionId,
        'desiredRevision': view.desiredRevision?.toString(),
        'appliedRevision': view.appliedRevision?.toString(),
      });
      await _shot(tester, evidenceDir, 'reopen-window');
      write(complete: true);
      expect(failures, isEmpty, reason: 'FIX-07 reopen gaps: $failures');
      return;
    }

    // Persist a safe inbound port (>=11808) so a reopen restore never binds
    // the user's live 10808 even if the developer core is discoverable.
    var safePort = 0;
    for (var candidate = 11841; candidate <= 11860; candidate++) {
      try {
        final probe = await ServerSocket.bind(
          InternetAddress.loopbackIPv4,
          candidate,
        );
        await probe.close();
        safePort = candidate;
        break;
      } on SocketException {
        continue;
      }
    }
    expect(safePort, greaterThanOrEqualTo(11808));
    final settings = container.read(settingsControllerProvider.notifier);
    final draft = settings.draft();
    final inbound = (draft['Inbound'] as List?)?.first;
    if (inbound is Map) inbound['LocalPort'] = safePort;
    settings.saveDocument(draft);
    await tester.pump(const Duration(milliseconds: 400));
    final persistedInbound = container
        .read(settingsControllerProvider)
        .document['Inbound'];
    final persistedPort =
        (persistedInbound is List && persistedInbound.isNotEmpty)
        ? (persistedInbound.first as Map)['LocalPort']
        : null;
    check('safe-inbound-port-saved', persistedPort == safePort, {
      'safePort': safePort,
      'persistedPort': persistedPort,
    });

    final bridge = container.read(bridgePortProvider);
    final subs = container.read(subsControllerProvider.notifier);
    final savedSub = subs.save(
      c.SubItemDto(
        id: '',
        remarks: 'FIX07合成订阅',
        url: 'http://127.0.0.1:11998/not-requested',
        moreUrl: '',
        enabled: true,
        userAgent: '',
        sort: 1,
        autoUpdateInterval: 0,
        updateTime: 0,
      ),
    );
    final subId = savedSub.item?.id ?? '';
    const link =
        'vless://00000007-7777-7777-7777-777777777777@127.0.0.1:11998?encryption=none#FIX07-ACTIVE';
    final imported = await bridge.importFromText(link, subid: subId);
    container.read(profilesControllerProvider.notifier).reload();
    await _wait(
      tester,
      () => container
          .read(profilesControllerProvider)
          .profiles
          .any((p) => p.remarks.contains('FIX07-ACTIVE')),
    );
    check('node-imported', imported.ok && imported.imported == 1, {
      'ok': imported.ok,
      'imported': imported.imported,
      'subId': subId,
      'error': imported.error?.code,
    });
    final state = container.read(profilesControllerProvider);
    final node = state.profiles.firstWhere(
      (p) => p.remarks.contains('FIX07-ACTIVE'),
      orElse: () => imported.profiles.isNotEmpty
          ? imported.profiles.first
          : state.profiles.first,
    );
    extra['activeId'] = node.indexId;
    extra['nodeRemarks'] = node.remarks;
    extra['profileCount'] = state.profiles.length;

    // Select the imported node through the controller (fixture setup), then
    // drive the real context-menu 设为活动 entry.
    container.read(profilesControllerProvider.notifier).selectRow(node.indexId);
    await tester.pump(const Duration(milliseconds: 200));
    final tableRect = tester.getRect(find.byType(ProfilesTable));
    await tester.tapAt(
      Offset(tableRect.right - 24, tableRect.bottom - 24),
      kind: PointerDeviceKind.mouse,
      buttons: kSecondaryButton,
    );
    await tester.pump(const Duration(milliseconds: 300));
    await tester.tap(find.byKey(const ValueKey('ctx-设为活动')));
    await tester.pump(const Duration(milliseconds: 300));
    check(
      'set-active-persisted',
      bridge.getActiveProfile() == node.indexId &&
          container.read(profilesControllerProvider).activeId == node.indexId,
      {
        'expectedActiveId': node.indexId,
        'persistedActiveId': bridge.getActiveProfile(),
        'controllerActiveId': container
            .read(profilesControllerProvider)
            .activeId,
      },
    );
    await _shot(tester, evidenceDir, 'activate-window');
    write(complete: true);
    expect(failures, isEmpty, reason: 'FIX-07 activate gaps: $failures');
  });
}
