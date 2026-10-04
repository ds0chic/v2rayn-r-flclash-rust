import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

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
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_fields.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

const _frameKey = ValueKey('journey-frame');

Future<void> _wait(
  WidgetTester tester,
  bool Function() condition, {
  String reason = 'UI did not reach the requested state',
}) async {
  final end = DateTime.now().add(const Duration(seconds: 30));
  while (DateTime.now().isBefore(end)) {
    await tester.pump(const Duration(milliseconds: 200));
    if (condition()) return;
  }
  throw TestFailure(reason);
}

Future<void> _tap(WidgetTester tester, String key) async {
  final target = find.byKey(ValueKey(key));
  await tester.ensureVisible(target);
  await tester.tap(target);
  await tester.pump(const Duration(milliseconds: 500));
}

Future<void> _menu(WidgetTester tester, String group, String item) async {
  await _tap(tester, 'menu-$group');
  await _tap(tester, 'menu-item-$item');
}

Future<void> _screenshot(WidgetTester tester, String dir, String name) async {
  await tester.pump(const Duration(milliseconds: 200));
  final boundary = tester.renderObject<RenderRepaintBoundary>(
    find.byKey(_frameKey),
  );
  final image = await boundary.toImage(pixelRatio: 1);
  final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
  if (bytes == null) throw StateError('Screenshot encoding failed');
  await File('$dir/$name.png').writeAsBytes(bytes.buffer.asUint8List());
  image.dispose();
}

Map<String, Object?> _feedback(WidgetTester tester) {
  final view = tester.getRect(find.byKey(_frameKey));
  final finder = find.byKey(const ValueKey('status-message'));
  if (finder.evaluate().isEmpty) return {'exists': false};
  final rect = tester.getRect(finder);
  return {
    'exists': true,
    'text': tester.widget<Text>(finder).data,
    'left': rect.left,
    'right': rect.right,
    'viewWidth': view.width,
    'intersectsViewport': rect.overlaps(view),
  };
}

Finder _portField() => find.descendant(
  of: find.byWidgetPredicate(
    (widget) =>
        widget is SettingsNumberField && widget.label == '本地端口 (LocalPort)',
  ),
  matching: find.byType(TextField),
);

Future<int> _availablePort([int start = 11840]) async {
  for (var port = start; port <= 11870; port++) {
    try {
      final probe = await ServerSocket.bind(InternetAddress.loopbackIPv4, port);
      await probe.close();
      return port;
    } on SocketException {
      continue;
    }
  }
  throw StateError('No available test port in 11840..11860');
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('normal user journeys with the real Windows UI and Rust bridge', (
    tester,
  ) async {
    final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
    final evidenceDir = Platform.environment['V2RAYN_R_JOURNEY_EVIDENCE_DIR'];
    expect(dataDir, isNotNull);
    expect(evidenceDir, isNotNull);
    await Directory(evidenceDir!).create(recursive: true);
    final observations = <Map<String, Object?>>[];
    final failures = <String>[];
    void checkpoint() {
      File('$evidenceDir/observations.json').writeAsStringSync(
        const JsonEncoder.withIndent('  ').convert({
          'failures': failures,
          'observations': observations,
          'recordingComplete': false,
        }),
      );
    }

    void check(String name, bool passed, Map<String, Object?> actual) {
      observations.add({'step': name, 'passed': passed, 'actual': actual});
      if (!passed) failures.add(name);
      checkpoint();
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

    try {
      if (Platform.environment['V2RAYN_R_JOURNEY_MODE'] == 'reopen') {
        check(
          'reopen-data',
          container.read(profilesControllerProvider).totalCount > 0,
          {
            'rows': container.read(profilesControllerProvider).totalCount,
            'activeId': container.read(profilesControllerProvider).activeId,
            'subscriptions': container
                .read(subsControllerProvider)
                .items
                .length,
            'inbound': container
                .read(settingsControllerProvider)
                .document['Inbound'],
            'doubleClick2Activate': container
                .read(profilesControllerProvider)
                .doubleClick2Activate,
          },
        );
        await _screenshot(tester, evidenceDir, '12-reopen');
        return;
      }

      observations.add({
        'step': 'first-launch',
        'actual': {
          'rows': container.read(profilesControllerProvider).totalCount,
          'applyEnabled':
              tester
                  .widget<FilledButton>(
                    find.byKey(const ValueKey('runtime-start')),
                  )
                  .onPressed !=
              null,
          'editEnabledWithoutSelection':
              tester
                  .widget<TextButton>(find.byKey(const ValueKey('toolbar-编辑')))
                  .onPressed !=
              null,
          'runtime': container.read(runtimeControllerProvider).state,
        },
      });
      checkpoint();
      await _screenshot(tester, evidenceDir, '01-first-launch');

      await _menu(tester, '设置', '参数设置');
      await tester.ensureVisible(_portField());
      final port = await _availablePort();
      await tester.enterText(_portField(), '$port');
      await tester.tap(find.widgetWithText(FilledButton, '保存'));
      await tester.pump(const Duration(milliseconds: 500));
      await _screenshot(tester, evidenceDir, '02-settings-saved');
      expect(
        container.read(settingsControllerProvider).document['Inbound'],
        isA<List>(),
      );
      final inbound =
          container.read(settingsControllerProvider).document['Inbound']
              as List;
      expect((inbound.first as Map)['LocalPort'], port);
      await tester.tap(find.widgetWithText(TextButton, '取消'));
      await tester.pump(const Duration(milliseconds: 400));

      HttpServer? server;
      for (var listenPort = 11821; listenPort <= 11831; listenPort++) {
        try {
          server = await HttpServer.bind(
            InternetAddress.loopbackIPv4,
            listenPort,
          );
          break;
        } on SocketException {
          continue;
        }
      }
      if (server == null) {
        throw StateError('No free subscription loopback port');
      }
      final subscriptionServer = server;
      final hits = <String>[];
      subscriptionServer.listen((request) {
        hits.add(request.uri.path);
        final names = request.uri.path == '/b' ? ['B1'] : ['A1', 'A2'];
        final text = names
            .map(
              (name) =>
                  'vless://${name == 'A1'
                      ? '11111111'
                      : name == 'A2'
                      ? '22222222'
                      : '33333333'}-1111-1111-1111-111111111111@127.0.0.1:${subscriptionServer.port}?encryption=none#$name',
            )
            .join('\n');
        request.response.write(base64Encode(utf8.encode(text)));
        request.response.close();
      });
      addTearDown(() => subscriptionServer.close(force: true));

      await Clipboard.setData(
        ClipboardData(text: 'http://127.0.0.1:${subscriptionServer.port}/a'),
      );
      await _menu(tester, '配置项', '从剪贴板导入分享链接');
      await _wait(
        tester,
        () => find
            .byKey(const ValueKey('sub-url-import-dialog'))
            .evaluate()
            .isNotEmpty,
      );
      await _screenshot(tester, evidenceDir, '03-subscription-guidance');
      await _tap(tester, 'sub-url-import-add');
      await _wait(
        tester,
        () => container.read(profilesControllerProvider).totalCount == 2,
      );
      check('import-subscription', true, {
        'rows': container.read(profilesControllerProvider).totalCount,
        'feedback': _feedback(tester),
      });
      await _screenshot(tester, evidenceDir, '04-imported');

      await _menu(tester, '订阅分组', '订阅分组设置');
      await _tap(tester, 'sub-add');
      await tester.enterText(
        find.byKey(const ValueKey('sub-field-remarks')),
        '测试-B',
      );
      await tester.enterText(
        find.byKey(const ValueKey('sub-field-url')),
        'http://127.0.0.1:${subscriptionServer.port}/b',
      );
      await _tap(tester, 'sub-edit-save');
      final subB = container
          .read(subsControllerProvider)
          .items
          .singleWhere((s) => s.remarks == '测试-B');
      await _tap(tester, 'sub-close');
      check(
        'saved-subscription-visible-in-main-window',
        find.byKey(ValueKey('group-filter-${subB.id}')).evaluate().isNotEmpty,
        {},
      );
      await _menu(tester, '订阅分组', '更新全部订阅 (不通过代理)');
      await _wait(
        tester,
        () => container.read(profilesControllerProvider).totalCount == 3,
      );
      await _tap(tester, 'group-filter-${subB.id}');
      hits.clear();
      await _menu(tester, '订阅分组', '更新当前订阅 (不通过代理)');
      await tester.pump(const Duration(milliseconds: 700));
      check('update-current-visible-group', hits.contains('/b'), {
        'selectedMainGroup': container
            .read(profilesControllerProvider)
            .groupSubId,
        'selectedSettingsSubscription': container
            .read(subsControllerProvider)
            .selectedId,
        'requestedPaths': List<String>.of(hits),
        'message': container.read(uiShellControllerProvider).message,
        'feedback': _feedback(tester),
      });
      await _screenshot(tester, evidenceDir, '05-current-group-update');

      await _tap(tester, 'group-filter-all');
      var profileState = container.read(profilesControllerProvider);
      final nodeA = profileState.profiles.singleWhere((p) => p.remarks == 'A1');
      await _tap(tester, 'cell-${nodeA.indexId}-Remarks');
      await _tap(tester, 'toolbar-启用/停用');
      observations.add({
        'step': 'choose-default-node',
        'actual': {
          'activeId': container.read(profilesControllerProvider).activeId,
          'runtime': container.read(runtimeControllerProvider).state,
          'feedback': _feedback(tester),
        },
      });
      await _screenshot(tester, evidenceDir, '06-activated-but-not-running');
      await _tap(tester, 'toolbar-启用/停用');
      check(
        'repeat-set-default-keeps-node',
        container.read(profilesControllerProvider).activeId == nodeA.indexId,
        {
          'activeId': container.read(profilesControllerProvider).activeId,
          'message': container.read(uiShellControllerProvider).message,
        },
      );
      await _tap(tester, 'toolbar-启用/停用');

      await _tap(tester, 'group-filter-${nodeA.subid}');
      await _tap(tester, 'cell-${nodeA.indexId}-Remarks');
      await _tap(tester, 'toolbar-TCPing');
      await _wait(
        tester,
        () => !container.read(profilesControllerProvider).speedTestRunning,
      );
      profileState = container.read(profilesControllerProvider);
      check(
        'testing-keeps-current-group',
        profileState.visible.every(
          (r) =>
              profileState.profiles
                  .singleWhere((p) => p.indexId == r.id)
                  .subid ==
              nodeA.subid,
        ),
        {
          'visibleRows': profileState.visible.map((r) => r.remarks).toList(),
          'delays': {
            for (final row in profileState.visible) row.remarks: row.delay,
          },
          'stage': profileState.speedTestStage,
        },
      );
      await _screenshot(tester, evidenceDir, '06b-test-results');
      await _tap(tester, 'group-filter-${nodeA.subid}');
      await _tap(tester, 'toolbar-备注');
      await _wait(
        tester,
        () =>
            find.byKey(const ValueKey('remarks-dialog')).evaluate().isNotEmpty,
      );
      await tester.enterText(
        find.byKey(const ValueKey('remarks-field')),
        'A1-renamed',
      );
      await _tap(tester, 'remarks-save');
      profileState = container.read(profilesControllerProvider);
      check(
        'editing-keeps-current-group',
        profileState.visible.every(
          (r) =>
              profileState.profiles
                  .singleWhere((p) => p.indexId == r.id)
                  .subid ==
              nodeA.subid,
        ),
        {
          'visibleRows': profileState.visible.map((r) => r.remarks).toList(),
          'groupId': profileState.groupSubId,
        },
      );
      await _screenshot(tester, evidenceDir, '07-group-after-edit');
      await _tap(tester, 'group-filter-${subB.id}');
      check(
        'switch-group-has-no-hidden-action-targets',
        container
            .read(profilesControllerProvider)
            .selected
            .every(
              (id) => container
                  .read(profilesControllerProvider)
                  .visible
                  .any((r) => r.id == id),
            ),
        {
          'visibleRows': container
              .read(profilesControllerProvider)
              .visible
              .map((r) => r.remarks)
              .toList(),
          'selectedIds': container
              .read(profilesControllerProvider)
              .selected
              .toList(),
        },
      );
      await _tap(tester, 'toolbar-编辑');
      observations.add({
        'step': 'edit-after-switching-group',
        'actual': {
          'editorOpened': find
              .byKey(const ValueKey('profile-editor'))
              .evaluate()
              .isNotEmpty,
          'selectedIds': container
              .read(profilesControllerProvider)
              .selected
              .toList(),
        },
      });
      if (find.byKey(const ValueKey('editor-cancel')).evaluate().isNotEmpty) {
        await _screenshot(tester, evidenceDir, '08-hidden-row-editor');
        await _tap(tester, 'editor-cancel');
      }

      await _tap(tester, 'group-filter-all');
      await _tap(tester, 'cell-${nodeA.indexId}-Remarks');
      final beforeCopy = container.read(profilesControllerProvider).totalCount;
      await Clipboard.setData(
        const ClipboardData(text: 'journey-clipboard-sentinel'),
      );
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyC);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump(const Duration(milliseconds: 500));
      final clipboard = (await Clipboard.getData(Clipboard.kTextPlain))?.text;
      check(
        'copy-share-does-not-clone',
        container.read(profilesControllerProvider).totalCount == beforeCopy &&
            clipboard != 'journey-clipboard-sentinel',
        {
          'beforeRows': beforeCopy,
          'afterRows': container.read(profilesControllerProvider).totalCount,
          'clipboardChanged': clipboard != 'journey-clipboard-sentinel',
        },
      );

      await _tap(tester, 'double-click-switch');
      final doubleClickBeforeSettings = container
          .read(profilesControllerProvider)
          .doubleClick2Activate;
      await _menu(tester, '设置', '参数设置');
      check(
        'opening-settings-preserves-double-click-choice',
        container.read(profilesControllerProvider).doubleClick2Activate ==
            doubleClickBeforeSettings,
        {
          'beforeSettings': doubleClickBeforeSettings,
          'afterOpeningSettings': container
              .read(profilesControllerProvider)
              .doubleClick2Activate,
        },
      );
      await tester.ensureVisible(_portField());
      final secondPort = await _availablePort(11850);
      final thirdPort = await _availablePort(secondPort + 1);
      await tester.enterText(_portField(), '$secondPort');
      await tester.tap(find.widgetWithText(FilledButton, '保存'));
      await tester.pump(const Duration(milliseconds: 500));
      await tester.enterText(_portField(), '$thirdPort');
      await _screenshot(tester, evidenceDir, '09-edit-after-save');
      await _tap(tester, 'settings-save');
      await _wait(
        tester,
        () => !container.read(runtimeControllerProvider).isBusy,
      );
      await tester.pump(const Duration(milliseconds: 700));
      final savedInbound =
          container.read(settingsControllerProvider).document['Inbound']
              as List;
      check(
        'apply-submits-latest-draft',
        (savedInbound.first as Map)['LocalPort'] == thirdPort,
        {
          'savedPort': (savedInbound.first as Map)['LocalPort'],
          'latestEditedPort': thirdPort,
          'runtimeError': container
              .read(runtimeControllerProvider)
              .error
              ?.toString(),
          'dialogStillOpen': find
              .byType(OptionSettingWindow)
              .evaluate()
              .isNotEmpty,
        },
      );
      await _screenshot(tester, evidenceDir, '10-apply-result');
      await _menu(tester, '设置', '参数设置');
      await _screenshot(tester, evidenceDir, '11-reopened-settings');
      await tester.tap(find.widgetWithText(TextButton, '取消'));
      await tester.pump(const Duration(milliseconds: 300));
      expect(
        failures,
        isEmpty,
        reason: 'User-flow gaps: ${failures.join(', ')}',
      );
    } finally {
      observations.add({
        'step': 'final-state',
        'actual': {
          'rows': container.read(profilesControllerProvider).totalCount,
          'message': container.read(uiShellControllerProvider).message,
          'subscriptionStatus': container
              .read(subsControllerProvider)
              .status
              ?.message,
          'runtimeError': container
              .read(runtimeControllerProvider)
              .error
              ?.toString(),
        },
      });
      await _screenshot(tester, evidenceDir, 'final-state');
      await File(
        '$evidenceDir/${Platform.environment['V2RAYN_R_JOURNEY_MODE'] == 'reopen' ? 'reopen' : 'observations'}.json',
      ).writeAsString(
        const JsonEncoder.withIndent('  ').convert({
          'failures': failures,
          'observations': observations,
          'recordingComplete': true,
        }),
      );
      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pump(const Duration(milliseconds: 300));
    }
  });
}
