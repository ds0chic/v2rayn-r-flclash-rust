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
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

const _frame = ValueKey('fix02-frame');

Future<void> _settle(WidgetTester tester) =>
    tester.pump(const Duration(milliseconds: 450));

Future<void> _menu(WidgetTester tester, String group, String item) async {
  await tester.tap(find.byKey(ValueKey('menu-$group')));
  await _settle(tester);
  await tester.tap(find.byKey(ValueKey('menu-item-$item')));
  await _settle(tester);
}

String? _initial(WidgetTester tester, String key) {
  final finder = find.byKey(ValueKey(key));
  if (finder.evaluate().isEmpty) return null;
  final widget = tester.widget(finder);
  if (widget is TextFormField) return widget.initialValue;
  return null;
}

Future<void> _shot(WidgetTester tester, String dir, String name) async {
  if (Platform.environment['V2RAYN_R_FIX02_IMAGES'] != '1') return;
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
  testWidgets('FIX-02 TUIC add/save/reopen through the real Windows UI', (
    tester,
  ) async {
    final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
    final dir = Platform.environment['V2RAYN_R_FIX02_EVIDENCE'];
    expect(dataDir, isNotNull);
    expect(dir, isNotNull);
    await Directory(dir!).create(recursive: true);
    final records = <Map<String, Object?>>[];
    final failures = <String>[];
    void write({bool complete = false}) {
      File('$dir/observations.json').writeAsStringSync(
        const JsonEncoder.withIndent('  ').convert({
          'applicationCommit': '1251cbc+FIX-02',
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
    for (
      var i = 0;
      i < 80 && find.byKey(const ValueKey('menu-配置项')).evaluate().isEmpty;
      i++
    ) {
      await tester.pump(const Duration(milliseconds: 100));
    }
    final container = ProviderScope.containerOf(
      tester.element(find.byType(V2rayNRApp)),
    );
    final profiles = container.read(profilesControllerProvider.notifier);
    final bridge = container.read(bridgePortProvider);

    try {
      const uuid = '11111111-2222-3333-4444-555555555555';
      await _menu(tester, '配置项', '添加 [TUIC]');
      check(
        'tuic-distinct-uuid-and-password-inputs',
        find.byKey(const ValueKey('field-username')).evaluate().isNotEmpty &&
            find.byKey(const ValueKey('field-password')).evaluate().isNotEmpty,
        {
          'usernameFieldCount': find
              .byKey(const ValueKey('field-username'))
              .evaluate()
              .length,
          'passwordFieldCount': find
              .byKey(const ValueKey('field-password'))
              .evaluate()
              .length,
        },
      );
      await _shot(tester, dir, '01-tuic-fields');

      await tester.enterText(
        find.byKey(const ValueKey('field-remarks')),
        'fix02-tuic',
      );
      await tester.enterText(
        find.byKey(const ValueKey('field-address')),
        '192.0.2.77',
      );
      await tester.enterText(
        find.byKey(const ValueKey('field-username')),
        uuid,
      );
      await tester.enterText(
        find.byKey(const ValueKey('field-password')),
        'tuic-pass',
      );
      await tester.tap(find.byKey(const ValueKey('editor-save')));
      await _settle(tester);

      final stored = profiles.profileById(
        container
            .read(profilesControllerProvider)
            .profiles
            .firstWhere((p) => p.remarks == 'fix02-tuic')
            .indexId,
      )!;
      check(
        'tuic-save-maps-uuid-to-username',
        stored.username == uuid && stored.password == 'tuic-pass',
        {
          'username': stored.username,
          'passwordIsSet': stored.password.isNotEmpty,
          'streamSecurity': stored.security.streamSecurity,
          'nodeCount': container.read(profilesControllerProvider).totalCount,
        },
      );
      await _shot(tester, dir, '02-tuic-saved');

      // Reopen through the real context-menu edit entry.
      final cell = find.byKey(ValueKey('cell-${stored.indexId}-Remarks'));
      await tester.tap(cell);
      await _settle(tester);
      await tester.tapAt(
        tester.getCenter(cell),
        kind: PointerDeviceKind.mouse,
        buttons: kSecondaryButton,
      );
      await _settle(tester);
      await tester.tap(find.byKey(const ValueKey('ctx-编辑')));
      await _settle(tester);
      check(
        'tuic-reopen-keeps-both-fields',
        _initial(tester, 'field-username') == uuid &&
            _initial(tester, 'field-password') == 'tuic-pass',
        {
          'reopenUsername': _initial(tester, 'field-username'),
          'reopenPasswordMatches':
              _initial(tester, 'field-password') == 'tuic-pass',
        },
      );
      await _shot(tester, dir, '03-tuic-reopen');

      // Remarks-only edit must not reset security/transport.
      await tester.enterText(
        find.byKey(const ValueKey('field-remarks')),
        'fix02-tuic-renamed',
      );
      await tester.tap(find.byKey(const ValueKey('editor-save')));
      await _settle(tester);
      final renamed = bridge.getProfile(stored.indexId)!;
      check(
        'tuic-remarks-only-keeps-security',
        renamed.remarks == 'fix02-tuic-renamed' &&
            renamed.security.streamSecurity == stored.security.streamSecurity &&
            renamed.network == stored.network &&
            renamed.transportExtra.path == stored.transportExtra.path,
        {
          'remarks': renamed.remarks,
          'streamSecurity': renamed.security.streamSecurity,
          'network': renamed.network,
          'path': renamed.transportExtra.path,
        },
      );
      await _shot(tester, dir, '04-tuic-remarks-only');
      write(complete: true);
      expect(failures, isEmpty, reason: 'FIX-02 differences: $failures');
    } finally {
      await tester.pumpWidget(const SizedBox.shrink());
      await _settle(tester);
    }
  });
}
