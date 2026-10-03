// FIX-06 real-window integration: add a remarks-only plain group (empty URL)
// through the real Flutter window + FRB + Rust + SQLite, save it, confirm the
// top group chip appears, reopen the settings window and confirm it persists.
//
// The target ports used by test fixtures (if any) are >= 11808; 10808 and the
// host system proxy are never touched. No subscription is downloaded and no
// kernel is started.
//
//   $env:V2RAYN_R_DATA_DIR = <fresh temp dir>
//   $env:V2RAYN_R_FIX06_EVIDENCE = <repo>/docs/evidence/UX-PARITY-FIX-06
//   $env:V2RAYN_R_FIX06_IMAGES = 1   # optional screenshots
//   flutter test integration_test/ux_parity_fix06_test.dart -d windows
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
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

const _frame = ValueKey('fix06-frame');

Future<void> _settle(WidgetTester tester) =>
    tester.pump(const Duration(milliseconds: 450));

Future<void> _pumpUntil(
  WidgetTester tester,
  bool Function() done, {
  Duration timeout = const Duration(seconds: 20),
  String? reason,
}) async {
  final end = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(end)) {
    await tester.pump(const Duration(milliseconds: 150));
    if (done()) return;
  }
  throw TestFailure(reason ?? 'condition not met within $timeout');
}

Future<void> _menu(WidgetTester tester, String group, String item) async {
  await tester.tap(find.byKey(ValueKey('menu-$group')));
  await _settle(tester);
  await tester.tap(find.byKey(ValueKey('menu-item-$item')));
  await _settle(tester);
}

Future<void> _shot(WidgetTester tester, String dir, String name) async {
  if (Platform.environment['V2RAYN_R_FIX06_IMAGES'] != '1') return;
  await _settle(tester);
  final boundary = tester.renderObject<RenderRepaintBoundary>(
    find.byKey(_frame),
  );
  final frame = await boundary.toImage(pixelRatio: 1);
  final bytes = await frame.toByteData(format: ui.ImageByteFormat.png);
  if (bytes != null) {
    await File('$dir/$name.png').writeAsBytes(bytes.buffer.asUint8List());
  }
  frame.dispose();
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets('plain remarks-only group saves, syncs chips and persists', (
    tester,
  ) async {
    final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
    final dir = Platform.environment['V2RAYN_R_FIX06_EVIDENCE'];
    expect(dataDir, isNotNull, reason: 'V2RAYN_R_DATA_DIR must be set');
    expect(dir, isNotNull, reason: 'V2RAYN_R_FIX06_EVIDENCE must be set');
    await Directory(dir!).create(recursive: true);
    final records = <Map<String, Object?>>[];
    final failures = <String>[];
    void write({bool complete = false}) {
      File('$dir/observations.json').writeAsStringSync(
        const JsonEncoder.withIndent('  ').convert({
          'task': 'FIX-06',
          'recordingComplete': complete,
          'failures': failures,
          'records': records,
        }),
      );
    }

    void check(String step, bool passed, Map<String, Object?> actual) {
      records.add({'step': step, 'passed': passed, 'actual': actual});
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
    await _pumpUntil(
      tester,
      () => find.byKey(const ValueKey('menu-订阅分组')).evaluate().isNotEmpty,
      reason: 'main window never appeared',
    );
    final container = ProviderScope.containerOf(
      tester.element(find.byType(V2rayNRApp)),
    );
    check(
      'isolated-first-launch',
      container.read(subsControllerProvider).items.isEmpty,
      {
        'subscriptionCount': container
            .read(subsControllerProvider)
            .items
            .length,
      },
    );

    try {
      // Add a remarks-only plain group through the real settings window.
      await _menu(tester, '订阅分组', '订阅分组设置');
      await _pumpUntil(
        tester,
        () => find
            .byKey(const ValueKey('sub-setting-window'))
            .evaluate()
            .isNotEmpty,
        reason: 'settings window did not open',
      );
      await tester.tap(find.byKey(const ValueKey('sub-add')));
      await _pumpUntil(
        tester,
        () =>
            find.byKey(const ValueKey('sub-edit-window')).evaluate().isNotEmpty,
        reason: 'editor did not open',
      );
      await tester.enterText(
        find.byKey(const ValueKey('sub-field-remarks')),
        '仅备注普通分组',
      );
      await tester.tap(find.byKey(const ValueKey('sub-edit-save')));
      await _pumpUntil(
        tester,
        () =>
            find.byKey(const ValueKey('sub-edit-window')).evaluate().isEmpty &&
            container.read(subsControllerProvider).items.length == 1,
        reason: 'plain group save did not close the editor',
      );
      final saved = container.read(subsControllerProvider).items.single;
      check(
        'plain-group-saves-with-empty-url',
        saved.url.isEmpty && saved.remarks == '仅备注普通分组',
        {
          'remarks': saved.remarks,
          'url': saved.url,
          'subId': saved.id,
          'editorStillOpen': find
              .byKey(const ValueKey('sub-edit-window'))
              .evaluate()
              .isNotEmpty,
        },
      );
      await _shot(tester, dir, '01-plain-group-saved');

      // Close the settings window; the top group chip must be present.
      await tester.tap(find.byKey(const ValueKey('sub-close')));
      await _settle(tester);
      await _pumpUntil(
        tester,
        () => find
            .byKey(ValueKey('group-filter-${saved.id}'))
            .evaluate()
            .isNotEmpty,
        reason: 'top group chip did not appear',
      );
      final chipFound = find
          .byKey(ValueKey('group-filter-${saved.id}'))
          .evaluate()
          .isNotEmpty;
      check('top-group-chip-appears-after-save', chipFound, {
        'groupId': saved.id,
        'chipFound': chipFound,
        'nodeCount': container.read(profilesControllerProvider).totalCount,
      });
      await _shot(tester, dir, '02-top-group-chip');

      // Reopen the settings window: the group is still listed and readable
      // from the persisted store.
      await _menu(tester, '订阅分组', '订阅分组设置');
      await _pumpUntil(
        tester,
        () => find
            .byKey(const ValueKey('sub-setting-window'))
            .evaluate()
            .isNotEmpty,
        reason: 'settings window did not reopen',
      );
      await _pumpUntil(
        tester,
        () => find.textContaining('仅备注普通分组').evaluate().isNotEmpty,
        reason: 'plain group missing after reopen',
      );
      final reopened = container
          .read(subsControllerProvider)
          .items
          .where((s) => s.id == saved.id)
          .single;
      check(
        'plain-group-persists-after-reopen',
        reopened.url.isEmpty && reopened.remarks == '仅备注普通分组',
        {'remarks': reopened.remarks, 'url': reopened.url},
      );
      await _shot(tester, dir, '03-reopen-persisted');
      await tester.tap(find.byKey(const ValueKey('sub-close')));
      await _settle(tester);

      write(complete: true);
      expect(failures, isEmpty, reason: 'FIX-06 failures: $failures');
    } finally {
      await tester.pumpWidget(const SizedBox.shrink());
      await _settle(tester);
    }
  });
}
