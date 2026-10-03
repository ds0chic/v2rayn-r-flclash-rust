// FIX-03: existing special nodes reopen in dedicated editors through the real
// Windows window + real FRB/Rust/SQLite.
//
// Flow: seed two synthetic leaves (loopback, never connected) + one Custom
// node + one PolicyGroup node via the real bridge, then for Custom and
// PolicyGroup: right-click -> 编辑 opens the dedicated editor (not the generic
// one), save persists, reopen shows the same editor with the same values,
// cancel leaves the database untouched. No kernel is started, no port is
// bound, no system proxy/TUN is touched; test ports are >= 11808 and the
// well-known 10808 is never used.
//
// Run:
//   $env:V2RAYN_R_DATA_DIR=<isolated temp>\fix03-data
//   $env:V2RAYN_R_FIX03_EVIDENCE=docs\evidence\UX-PARITY-FIX-03
//   flutter test integration_test/ux_parity_fix03_test.dart -d windows
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
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

const _frame = ValueKey('fix03-frame');

Future<void> _settle(WidgetTester tester) =>
    tester.pump(const Duration(milliseconds: 450));

Future<void> _shot(WidgetTester tester, String dir, String name) async {
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

String? _initial(WidgetTester tester, String key) {
  final finder = find.byKey(ValueKey(key));
  if (finder.evaluate().isEmpty) return null;
  final widget = tester.widget(finder);
  if (widget is TextFormField) return widget.initialValue;
  return null;
}

c.ProfileDto _leaf(String remarks, int port) => c.ProfileDto(
  indexId: '',
  configType: ConfigType.vless,
  coreType: CoreType.xray,
  configVersion: 4,
  subid: '',
  isSub: false,
  displayLog: true,
  remarks: remarks,
  address: '127.0.0.1',
  port: port,
  password: '',
  username: '',
  network: 'raw',
  security: const c.SecurityDto(),
  protoExtra: const c.ProtocolExtraDto(extraJson: '{}'),
  transportExtra: const c.TransportExtraDto(extraJson: '{}'),
  extraJson: '{}',
);

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets('FIX-03 special nodes edit/save/reopen in dedicated editors', (
    tester,
  ) async {
    final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
    final dir = Platform.environment['V2RAYN_R_FIX03_EVIDENCE'];
    expect(dataDir, isNotNull);
    expect(dir, isNotNull);
    await Directory(dir!).create(recursive: true);
    final records = <Map<String, Object?>>[];
    final failures = <String>[];
    void write({bool complete = false}) {
      File('$dir/observations.json').writeAsStringSync(
        const JsonEncoder.withIndent('  ').convert({
          'applicationCommit': 'FIX-03',
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

    c.ProfileDto seed(c.ProfileDto draft) {
      final result = bridge.saveProfile(draft, bridge.profileRevision());
      expect(result.ok, isTrue, reason: result.error?.messageKey);
      return result.profile!;
    }

    Future<void> openEdit(String indexId) async {
      profiles.reload();
      await _settle(tester);
      final cell = find.byKey(ValueKey('cell-$indexId-Remarks'));
      expect(cell, findsOneWidget, reason: 'row $indexId visible');
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
    }

    try {
      final leaf1 = seed(_leaf('fix03-HK-01', 11981));
      final leaf2 = seed(_leaf('fix03-US-01', 11982));
      check('leaves-seeded', true, {
        'leaf1': leaf1.indexId,
        'leaf2': leaf2.indexId,
      });

      final custom = seed(
        c.ProfileDto(
          indexId: '',
          configType: ConfigType.custom,
          coreType: CoreType.xray,
          configVersion: 4,
          subid: '',
          isSub: false,
          preSocksPort: 11820,
          displayLog: true,
          remarks: 'fix03-custom',
          address: 'fix03-custom.yaml',
          port: 0,
          password: '',
          username: '',
          network: 'raw',
          security: const c.SecurityDto(),
          protoExtra: const c.ProtocolExtraDto(
            extraJson:
                '{"customConfigText": "{\\"tag\\": \\"proxy\\"}",'
                ' "futureFlag": true}',
          ),
          transportExtra: const c.TransportExtraDto(
            extraJson: '{"futureTransport": 7}',
          ),
          // Top-level extras survive SQLite only through legacy columns
          // (upstream has no column for free-form top-level keys).
          extraJson: '{"HeaderType": "none"}',
        ),
      );
      final group = seed(
        c.ProfileDto(
          indexId: '',
          configType: ConfigType.policyGroup,
          coreType: CoreType.xray,
          configVersion: 4,
          subid: '',
          isSub: false,
          displayLog: true,
          remarks: 'fix03-group',
          address: '',
          port: 0,
          password: '',
          username: '',
          network: '',
          security: const c.SecurityDto(),
          protoExtra: c.ProtocolExtraDto(
            groupType: 'PolicyGroup',
            childItems: '${leaf1.indexId},${leaf2.indexId}',
            filter: '^fix03',
            multipleLoad: 3,
            extraJson: '{}',
          ),
          transportExtra: const c.TransportExtraDto(extraJson: '{}'),
          extraJson: '{}',
        ),
      );
      check('specials-seeded', true, {
        'customId': custom.indexId,
        'groupId': group.indexId,
      });

      // Custom: context-menu edit must open the dedicated editor.
      await openEdit(custom.indexId);
      final customRouted =
          find.byKey(const ValueKey('custom-editor')).evaluate().isNotEmpty &&
          find.byKey(const ValueKey('profile-editor')).evaluate().isEmpty;
      check('custom-edit-opens-custom-editor', customRouted, {
        'customEditor': find
            .byKey(const ValueKey('custom-editor'))
            .evaluate()
            .length,
        'genericEditor': find
            .byKey(const ValueKey('profile-editor'))
            .evaluate()
            .length,
      });
      await _shot(tester, dir, '01-custom-editor');
      await tester.enterText(
        find.byKey(const ValueKey('custom-remarks')),
        'fix03-custom-renamed',
      );
      await tester.tap(find.byKey(const ValueKey('custom-save')));
      await _settle(tester);
      final storedCustom = bridge.getProfile(custom.indexId)!;
      final storedProto =
          jsonDecode(storedCustom.protoExtra.extraJson) as Map<String, dynamic>;
      final storedTransport = jsonDecode(
        storedCustom.transportExtra.extraJson,
      ) as Map<String, dynamic>;
      final storedTop =
          jsonDecode(storedCustom.extraJson) as Map<String, dynamic>;
      final customText = storedProto['customConfigText'] as String?;
      check(
        'custom-save-keeps-text-and-extras',
        storedCustom.remarks == 'fix03-custom-renamed' &&
            storedCustom.preSocksPort == 11820 &&
            (customText ?? '').contains('proxy') &&
            storedProto['futureFlag'] == true &&
            storedTransport['futureTransport'] == 7 &&
            storedTop['HeaderType'] == 'none',
        {
          'remarks': storedCustom.remarks,
          'preSocksPort': storedCustom.preSocksPort,
          'customConfigText': customText,
          'futureFlag': storedProto['futureFlag'],
          'futureTransport': storedTransport['futureTransport'],
          'headerType': storedTop['HeaderType'],
        },
      );
      await _shot(tester, dir, '02-custom-saved');

      // Reopen: same dedicated editor, same values.
      await openEdit(custom.indexId);
      check(
        'custom-reopen-same-editor-and-values',
        find.byKey(const ValueKey('custom-editor')).evaluate().isNotEmpty &&
            _initial(tester, 'custom-remarks') == 'fix03-custom-renamed' &&
            _initial(tester, 'custom-address') == 'fix03-custom.yaml',
        {
          'reopenRemarks': _initial(tester, 'custom-remarks'),
          'reopenAddress': _initial(tester, 'custom-address'),
        },
      );
      await _shot(tester, dir, '03-custom-reopen');
      // Cancel must not touch the database.
      await tester.enterText(
        find.byKey(const ValueKey('custom-remarks')),
        'fix03-custom-cancelled-edit',
      );
      await tester.tap(find.byKey(const ValueKey('custom-cancel')));
      await _settle(tester);
      final afterCancel = bridge.getProfile(custom.indexId)!;
      check(
        'custom-cancel-keeps-database',
        afterCancel.remarks == 'fix03-custom-renamed',
        {'remarks': afterCancel.remarks},
      );

      // PolicyGroup: context-menu edit must open the group editor.
      await openEdit(group.indexId);
      final groupRouted =
          find.byKey(const ValueKey('group-editor')).evaluate().isNotEmpty &&
          find.byKey(const ValueKey('profile-editor')).evaluate().isEmpty;
      check('group-edit-opens-group-editor', groupRouted, {
        'groupEditor': find
            .byKey(const ValueKey('group-editor'))
            .evaluate()
            .length,
        'genericEditor': find
            .byKey(const ValueKey('profile-editor'))
            .evaluate()
            .length,
      });
      final tilesPresent =
          find
              .byKey(ValueKey('group-child-${leaf1.indexId}'))
              .evaluate()
              .isNotEmpty &&
          find
              .byKey(ValueKey('group-child-${leaf2.indexId}'))
              .evaluate()
              .isNotEmpty;
      check('group-editor-shows-ordered-children', tilesPresent, {
        'leaf1Tile': find
            .byKey(ValueKey('group-child-${leaf1.indexId}'))
            .evaluate()
            .length,
        'leaf2Tile': find
            .byKey(ValueKey('group-child-${leaf2.indexId}'))
            .evaluate()
            .length,
      });
      await _shot(tester, dir, '04-group-editor');
      await tester.enterText(
        find.byKey(const ValueKey('group-remarks')),
        'fix03-group-renamed',
      );
      await tester.tap(find.byKey(const ValueKey('group-save')));
      await _settle(tester);
      final storedGroup = bridge.getProfile(group.indexId)!;
      check(
        'group-save-keeps-children-mode-filter',
        storedGroup.remarks == 'fix03-group-renamed' &&
            storedGroup.protoExtra.childItems ==
                '${leaf1.indexId},${leaf2.indexId}' &&
            storedGroup.protoExtra.multipleLoad == 3 &&
            storedGroup.protoExtra.filter == '^fix03' &&
            storedGroup.protoExtra.groupType == 'PolicyGroup',
        {
          'remarks': storedGroup.remarks,
          'childItems': storedGroup.protoExtra.childItems,
          'multipleLoad': storedGroup.protoExtra.multipleLoad,
          'filter': storedGroup.protoExtra.filter,
          'groupType': storedGroup.protoExtra.groupType,
        },
      );
      await _shot(tester, dir, '05-group-saved');

      // Reopen: same group editor with the same children.
      await openEdit(group.indexId);
      check(
        'group-reopen-same-editor-and-children',
        find.byKey(const ValueKey('group-editor')).evaluate().isNotEmpty &&
            find
                .byKey(ValueKey('group-child-${leaf1.indexId}'))
                .evaluate()
                .isNotEmpty &&
            find
                .byKey(ValueKey('group-child-${leaf2.indexId}'))
                .evaluate()
                .isNotEmpty,
        {
          'groupEditor': find
              .byKey(const ValueKey('group-editor'))
              .evaluate()
              .length,
        },
      );
      await _shot(tester, dir, '06-group-reopen');
      await tester.enterText(
        find.byKey(const ValueKey('group-remarks')),
        'fix03-group-cancelled-edit',
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('group-cancel')));
      await _settle(tester);
      final groupAfterCancel = bridge.getProfile(group.indexId)!;
      check(
        'group-cancel-keeps-database',
        groupAfterCancel.remarks == 'fix03-group-renamed',
        {'remarks': groupAfterCancel.remarks},
      );

      write(complete: true);
      expect(failures, isEmpty, reason: 'FIX-03 differences: $failures');
    } finally {
      await tester.pumpWidget(const SizedBox.shrink());
      await _settle(tester);
    }
  });
}
