import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:v2rayn_desktop/app/app.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

const _frame = ValueKey('parity-review-frame');

Future<void> _settle(WidgetTester tester) =>
    tester.pump(const Duration(milliseconds: 450));

Future<void> _menu(WidgetTester tester, String group, String item) async {
  await tester.tap(find.byKey(ValueKey('menu-$group')));
  await _settle(tester);
  await tester.tap(find.byKey(ValueKey('menu-item-$item')));
  await _settle(tester);
}

Future<void> _shot(WidgetTester tester, String dir, String name) async {
  if (Platform.environment['V2RAYN_R_PARITY_IMAGES'] != '1') return;
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
  testWidgets('frozen upstream parity through real Windows UI and Rust', (
    tester,
  ) async {
    final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
    final dir = Platform.environment['V2RAYN_R_PARITY_EVIDENCE'];
    expect(dataDir, isNotNull);
    expect(dir, isNotNull);
    await Directory(dir!).create(recursive: true);
    final records = <Map<String, Object?>>[];
    final failures = <String>[];
    void write({bool complete = false}) {
      File('$dir/observations.json').writeAsStringSync(
        const JsonEncoder.withIndent('  ').convert({
          'applicationCommit': '1251cbc',
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
    check(
      'isolated-first-launch',
      container.read(profilesControllerProvider).totalCount == 0,
      {
        'nodeCount': container.read(profilesControllerProvider).totalCount,
        'subscriptionCount': container
            .read(subsControllerProvider)
            .items
            .length,
      },
    );

    try {
      // The configuration contains no listeners and is never applied.
      const customText =
          '{"log":{"loglevel":"warning"},"inbounds":[],"outbounds":[{"tag":"direct","protocol":"freedom","settings":{}}]}';
      await Clipboard.setData(const ClipboardData(text: customText));
      await _menu(tester, '配置项', '从剪贴板导入分享链接');
      await _settle(tester);
      check(
        'import-complete-xray-config',
        container.read(profilesControllerProvider).totalCount == 1,
        {
          'nodeCount': container.read(profilesControllerProvider).totalCount,
          'message': container.read(uiShellControllerProvider).message,
        },
      );
      await _shot(tester, dir, '01-complete-config-import');

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
          'uuidLabelCount': find.text('UUID').evaluate().length,
        },
      );
      await _shot(tester, dir, '02-tuic-fields');
      await tester.tap(find.byKey(const ValueKey('editor-cancel')));
      await _settle(tester);

      await _menu(tester, '订阅分组', '订阅分组设置');
      await tester.tap(find.byKey(const ValueKey('sub-add')));
      await _settle(tester);
      await tester.enterText(
        find.byKey(const ValueKey('sub-field-remarks')),
        '合成普通分组',
      );
      await tester.tap(find.byKey(const ValueKey('sub-edit-save')));
      await _settle(tester);
      check(
        'create-group-without-subscription-url',
        container.read(subsControllerProvider).items.length == 1,
        {
          'subscriptionCount': container
              .read(subsControllerProvider)
              .items
              .length,
          'editorStillOpen': find
              .byKey(const ValueKey('sub-edit-window'))
              .evaluate()
              .isNotEmpty,
          'urlRequiredError': find
              .text('error.url_required')
              .evaluate()
              .isNotEmpty,
        },
      );
      await _shot(tester, dir, '03-empty-url-group');
      await tester.tap(find.byKey(const ValueKey('sub-close')));
      await _settle(tester);

      const nodeLink =
          'vless://00000001-1111-1111-1111-111111111111@127.0.0.1:11998?encryption=none#parity-VLESS';
      await Clipboard.setData(const ClipboardData(text: nodeLink));
      await _menu(tester, '配置项', '从剪贴板导入分享链接');
      final node = container
          .read(profilesControllerProvider)
          .visible
          .firstWhere((row) => row.remarks == 'parity-VLESS');
      final cell = find.byKey(ValueKey('cell-${node.id}-Remarks'));
      await tester.tap(cell);
      await _settle(tester);
      await _menu(tester, '配置项', '扫描屏幕上的二维码');
      final sharing = find.byKey(const ValueKey('profile-share-qr'));
      check('screen-qr-scan-is-import', sharing.evaluate().isEmpty, {
        'openedShareDialog': sharing.evaluate().isNotEmpty,
        'message': container.read(uiShellControllerProvider).message,
      });
      await _shot(tester, dir, '04-screen-scan-entry');
      if (sharing.evaluate().isNotEmpty) {
        await tester.tap(
          find.descendant(
            of: sharing,
            matching: find.widgetWithText(TextButton, '关闭'),
          ),
        );
        await _settle(tester);
      }
      await _menu(tester, '配置项', '扫描图片中的二维码');
      final textImport = find.byKey(const ValueKey('import-paste-dialog'));
      check('image-qr-scan-is-image-input', textImport.evaluate().isEmpty, {
        'openedTextImport': textImport.evaluate().isNotEmpty,
      });
      if (textImport.evaluate().isNotEmpty) {
        await tester.tap(find.byKey(const ValueKey('import-paste-cancel')));
        await _settle(tester);
      }

      // Arrange group membership through real persistence, then choose the
      // group in the UI. This setup does not validate the move-menu behavior.
      final sub = container.read(subsControllerProvider).items.single;
      expect(sub.id, isNotEmpty);
      final profiles = container.read(profilesControllerProvider.notifier);
      expect(profiles.moveProfilesToGroup([node.id], sub.id), isTrue);
      await _settle(tester);
      await tester.tap(find.byKey(ValueKey('group-filter-${sub.id}')));
      await _settle(tester);
      expect(container.read(profilesControllerProvider).groupSubId, sub.id);
      expect(profiles.profileById(node.id)?.subid, sub.id);
      records.add({
        'step': 'synthetic-group-fixture',
        'fixtureSetup': true,
        'actual': {
          'currentGroupId': sub.id,
          'nodeGroupId': profiles.profileById(node.id)?.subid,
        },
      });
      write();

      await tester.tapAt(
        tester.getCenter(cell),
        kind: PointerDeviceKind.mouse,
        buttons: kSecondaryButton,
      );
      await _settle(tester);
      await tester.tap(find.byKey(const ValueKey('ctx-一键生成策略组')));
      await _settle(tester);
      final countBefore = container.read(profilesControllerProvider).totalCount;
      await tester.tap(find.byKey(const ValueKey('ctx-全部配置项')));
      await _settle(tester);
      check(
        'context-generate-group-keeps-menu-target',
        container.read(profilesControllerProvider).totalCount > countBefore,
        {
          'nodeCountBefore': countBefore,
          'nodeCountAfter': container
              .read(profilesControllerProvider)
              .totalCount,
          'openedGroupEditor': find
              .byKey(const ValueKey('group-editor'))
              .evaluate()
              .isNotEmpty,
          'message': container.read(uiShellControllerProvider).message,
        },
      );
      await _shot(tester, dir, '05-group-command');
      write(complete: true);
      expect(failures, isEmpty, reason: 'Upstream differences: $failures');
    } finally {
      await tester.pumpWidget(const SizedBox.shrink());
      await _settle(tester);
    }
  });
}
