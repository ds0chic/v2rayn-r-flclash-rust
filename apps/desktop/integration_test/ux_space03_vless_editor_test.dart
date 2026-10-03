// UX-SPACE-03-VLESS: real-window spacing/error/cancel evidence.
//
// Drives the real Flutter Windows window against the real FRB/Rust/SQLite,
// imports two synthetic VLESS links, opens the node editor, measures the
// label/control/error/next-field/button rectangles, checks the invalid-port
// rejection keeps the draft, and reopens to read the stored values.
//
// Only presentation is exercised: no kernel, no listening port, no system
// proxy, no TUN. Data dir and evidence dir come from the environment.
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

const _frame = ValueKey('ux-space03-frame');

Future<void> _settle(WidgetTester tester) =>
    tester.pump(const Duration(milliseconds: 450));

Future<void> _shot(WidgetTester tester, String dir, String name) async {
  await tester.pump(const Duration(milliseconds: 200));
  if (Platform.environment['V2RAYN_R_UX_SPACE03_SKIP_IMAGES'] == '1') return;
  final boundary = tester.renderObject<RenderRepaintBoundary>(
    find.byKey(_frame),
  );
  final image = await boundary.toImage(pixelRatio: 1);
  final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
  if (bytes == null) throw StateError('screenshot encoding failed');
  await File('$dir/$name.png').writeAsBytes(bytes.buffer.asUint8List());
  image.dispose();
}

Map<String, double> _rect(WidgetTester tester, Finder finder) {
  final r = tester.getRect(finder);
  return {'x': r.left, 'y': r.top, 'w': r.width, 'h': r.height};
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets('UX-SPACE-03 VLESS editor in the real window', (tester) async {
    final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
    final evidenceDir =
        Platform.environment['V2RAYN_R_UX_SPACE03_EVIDENCE_DIR'];
    expect(dataDir, isNotNull);
    expect(evidenceDir, isNotNull);
    await Directory(evidenceDir!).create(recursive: true);

    final records = <Map<String, Object?>>[];
    final failures = <String>[];
    void record(String name, Map<String, Object?> facts, {bool? passed}) {
      final entry = <String, Object?>{'step': name, 'facts': facts};
      if (passed != null) entry['passed'] = passed;
      records.add(entry);
      if (passed == false) failures.add(name);
      File('$evidenceDir/observations.json').writeAsStringSync(
        const JsonEncoder.withIndent(' ')
            .convert({'failures': failures, 'records': records}),
      );
    }

    void complete() {
      File('$evidenceDir/observations.json').writeAsStringSync(
        const JsonEncoder.withIndent(' ').convert({
          'recordingComplete': true,
          'devicePixelRatio': tester.view.devicePixelRatio,
          'physicalSize': <double>[
            tester.view.physicalSize.width,
            tester.view.physicalSize.height,
          ],
          'failures': failures,
          'records': records,
        }),
      );
      expect(
        failures,
        isEmpty,
        reason: 'UX-SPACE-03 gaps: ${failures.join(', ')}',
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
        child: const RepaintBoundary(key: _frame, child: V2rayNRApp()),
      ),
    );
    for (
      var i = 0;
      i < 100 && find.byKey(const ValueKey('menu-配置项')).evaluate().isEmpty;
      i++
    ) {
      await tester.pump(const Duration(milliseconds: 200));
    }
    final container = ProviderScope.containerOf(
      tester.element(find.byType(V2rayNRApp)),
    );
    // A fresh UI-state store leaves themeMode at `system` and the settings
    // load may still be applying, so the theme is set right before each editor
    // opens (see [setTheme]) to keep the evidence deterministic.
    Future<void> setTheme(ThemeMode mode) async {
      container.read(uiShellControllerProvider.notifier).setThemeMode(mode);
      await _settle(tester);
    }

    final links = List.generate(
      2,
      (i) =>
          'vless://${(i + 1).toString().padLeft(8, '0')}-1111-1111-1111-111111111111@127.0.0.1:11980?encryption=none#${Uri.encodeComponent('节点${i + 1}')}',
    ).join('\n');
    await Clipboard.setData(ClipboardData(text: links));
    await tester.tap(find.byKey(const ValueKey('menu-配置项')));
    await _settle(tester);
    await tester.tap(find.byKey(const ValueKey('menu-item-从剪贴板导入分享链接')));
    for (
      var i = 0;
      i < 100 && container.read(profilesControllerProvider).totalCount < 2;
      i++
    ) {
      await tester.pump(const Duration(milliseconds: 200));
    }
    expect(container.read(profilesControllerProvider).totalCount, 2);
    final rows = container.read(profilesControllerProvider).visible;
    Finder cell(int i) => find.byKey(ValueKey('cell-${rows[i].id}-Remarks'));

    Future<void> openEditor(int index) async {
      await tester.tapAt(
        tester.getCenter(cell(index)),
        kind: PointerDeviceKind.mouse,
        buttons: kSecondaryButton,
      );
      await _settle(tester);
      await tester.tap(find.byKey(const ValueKey('ctx-编辑')));
      await _settle(tester);
      expect(find.byKey(const ValueKey('profile-editor')), findsOneWidget);
    }

    Future<void> closeEditor() async {
      await tester.tap(find.byKey(const ValueKey('editor-cancel')));
      await _settle(tester);
    }

    String? initial(String key) =>
        tester.widget<TextFormField>(find.byKey(ValueKey(key))).initialValue;

    Map<String, Object?> measure(ThemeData theme) {
      final label = _rect(tester, find.byKey(const ValueKey('label-remarks')));
      final remarks = _rect(
        tester,
        find.byKey(const ValueKey('field-remarks')),
      );
      final address = _rect(
        tester,
        find.byKey(const ValueKey('field-address')),
      );
      final encryption = _rect(
        tester,
        find.byKey(const ValueKey('field-vlessEncryption')),
      );
      final transport = _rect(tester, find.byKey(const ValueKey('section-传输')));
      final save = _rect(tester, find.byKey(const ValueKey('editor-save')));
      final cancel = _rect(tester, find.byKey(const ValueKey('editor-cancel')));
      final frame = _rect(tester, find.byKey(_frame));
      final labelText = tester.widget<Text>(
        find.byKey(const ValueKey('label-remarks')),
      );
      final editable = tester.widget<EditableText>(
        find.descendant(
          of: find.byKey(const ValueKey('field-remarks')),
          matching: find.byType(EditableText),
        ),
      );
      return {
        'themeBrightness': theme.brightness.name,
        'baseFontSize': theme.textTheme.bodyMedium?.fontSize,
        'labelFontSize': labelText.style?.fontSize,
        'contentFontSize': editable.style.fontSize,
        'labelGap': remarks['x']! - (label['x']! + label['w']!),
        'controlHeight': remarks['h'],
        'fieldGap': address['y']! - (remarks['y']! + remarks['h']!),
        'groupGap': transport['y']! - (encryption['y']! + encryption['h']!),
        'saveInWindow':
            (save['y']! + save['h']!) <= (frame['y']! + frame['h']! + 0.5),
        'cancelInWindow':
            (cancel['y']! + cancel['h']!) <= (frame['y']! + frame['h']! + 0.5),
        'label': label,
        'remarks': remarks,
        'address': address,
        'transportTitle': transport,
        'save': save,
        'cancel': cancel,
        'frame': frame,
      };
    }

    bool metricsOk(Map<String, Object?> m) {
      final controlHeight = m['controlHeight']! as double;
      final labelGap = m['labelGap']! as double;
      final fieldGap = m['fieldGap']! as double;
      final groupGap = m['groupGap']! as double;
      final baseFont = m['baseFontSize']! as double;
      final large = baseFont > 14;
      return (large
              ? controlHeight >= 34
              : controlHeight >= 34 && controlHeight <= 37) &&
          labelGap >= 12 &&
          labelGap <= 16 &&
          fieldGap >= 8 &&
          fieldGap <= 12 &&
          groupGap >= 14 &&
          groupGap <= 18 &&
          m['saveInWindow'] == true &&
          m['cancelInWindow'] == true;
    }

    try {
      // --- default font, light -------------------------------------------
      await setTheme(ThemeMode.light);
      await openEditor(0);
      final theme0 = Theme.of(
        tester.element(find.byKey(const ValueKey('profile-editor'))),
      );
      final m0 = measure(theme0);
      record('editor-default-light', m0, passed: metricsOk(m0));
      await _shot(tester, evidenceDir, '01-editor-default-light');

      // RepaintBoundary captured above already proves the painter; assert the
      // runtime font style equals the theme base size (no const 12 lock).
      record(
        'font-propagation-default',
        {
          'baseFontSize': m0['baseFontSize'],
          'labelFontSize': m0['labelFontSize'],
          'contentFontSize': m0['contentFontSize'],
        },
        passed:
            m0['labelFontSize'] == m0['baseFontSize'] &&
            m0['contentFontSize'] == m0['baseFontSize'],
      );

      // --- invalid port: rejected, draft + error kept --------------------
      final countBefore = container.read(profilesControllerProvider).totalCount;
      await tester.enterText(
        find.byKey(const ValueKey('field-remarks')),
        '真实窗口改后备注',
      );
      await tester.enterText(find.byKey(const ValueKey('field-port')), '99999');
      await tester.tap(find.byKey(const ValueKey('editor-save')));
      await _settle(tester);
      final errorRect = _rect(tester, find.text('端口需在 1-65535'));
      final remarksRect = _rect(
        tester,
        find.byKey(const ValueKey('field-remarks')),
      );
      final addressRect = _rect(
        tester,
        find.byKey(const ValueKey('field-address')),
      );
      record(
        'invalid-port-rejected',
        {
          'dialogOpen': find
              .byKey(const ValueKey('profile-editor'))
              .evaluate()
              .isNotEmpty,
          'errorVisible': find.text('端口需在 1-65535').evaluate().isNotEmpty,
          'errorRect': errorRect,
          'remarksInitial': initial('field-remarks'),
          'portInitial': initial('field-port'),
          'countUnchanged':
              container.read(profilesControllerProvider).totalCount ==
              countBefore,
          'remarksErrorRowGap':
              addressRect['y']! - (remarksRect['y']! + remarksRect['h']!),
        },
        passed:
            find
                .byKey(const ValueKey('profile-editor'))
                .evaluate()
                .isNotEmpty &&
            find.text('端口需在 1-65535').evaluate().isNotEmpty &&
            initial('field-remarks') == '真实窗口改后备注' &&
            initial('field-port') == '99999' &&
            container.read(profilesControllerProvider).totalCount ==
                countBefore,
      );
      await _shot(tester, evidenceDir, '02-editor-error-light');
      await closeEditor();

      // --- reopen reads stored values, not the discarded draft -----------
      await openEditor(0);
      final reopenRemarks = initial('field-remarks');
      final reopenPort = initial('field-port');
      record('cancel-reopen-original', {
        'expectedRemarks': rows[0].remarks,
        'reopenRemarks': reopenRemarks,
        'reopenPort': reopenPort,
      }, passed: reopenRemarks == rows[0].remarks && reopenPort == '11980');
      await _shot(tester, evidenceDir, '03-editor-reopen-light');
      await closeEditor();

      // --- dark theme ----------------------------------------------------
      await setTheme(ThemeMode.dark);
      await openEditor(0);
      final themeDark = Theme.of(
        tester.element(find.byKey(const ValueKey('profile-editor'))),
      );
      final mDark = measure(themeDark);
      record('editor-default-dark', mDark, passed: metricsOk(mDark));
      await _shot(tester, evidenceDir, '04-editor-default-dark');
      await closeEditor();

      // --- large user font size -----------------------------------------
      await setTheme(ThemeMode.light);
      container
          .read(uiShellControllerProvider.notifier)
          .applyThemeSelection(fontSize: 20);
      await _settle(tester);
      await openEditor(0);
      final themeLarge = Theme.of(
        tester.element(find.byKey(const ValueKey('profile-editor'))),
      );
      final mLarge = measure(themeLarge);
      record(
        'editor-large-font',
        mLarge,
        passed:
            metricsOk(mLarge) &&
            mLarge['baseFontSize'] == 20 &&
            mLarge['labelFontSize'] == 20 &&
            (mLarge['controlHeight']! as double) >= 34,
      );
      await _shot(tester, evidenceDir, '05-editor-large-font');
      await closeEditor();

      complete();
    } finally {
      await _shot(tester, evidenceDir, 'final-state');
      await tester.pumpWidget(const SizedBox.shrink());
      await _settle(tester);
    }
  });
}
