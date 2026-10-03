// UX-SPACE-03-VLESS "before" probe: captures the editor as it renders at the
// frozen HEAD baseline (no label column, dense floating labels). Run against a
// HEAD checkout so the evidence has a before/after pair for the same window.
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

const _frame = ValueKey('ux-space03-before-frame');

Future<void> _settle(WidgetTester tester) =>
    tester.pump(const Duration(milliseconds: 450));

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets('capture pre-change editor', (tester) async {
    final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
    final evidenceDir =
        Platform.environment['V2RAYN_R_UX_SPACE03_EVIDENCE_DIR'];
    expect(dataDir, isNotNull);
    expect(evidenceDir, isNotNull);
    await Directory(evidenceDir!).create(recursive: true);

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
    container
        .read(uiShellControllerProvider.notifier)
        .setThemeMode(ThemeMode.light);
    await _settle(tester);
    final rows = container.read(profilesControllerProvider).visible;
    await tester.tapAt(
      tester.getCenter(find.byKey(ValueKey('cell-${rows[0].id}-Remarks'))),
      kind: PointerDeviceKind.mouse,
      buttons: kSecondaryButton,
    );
    await _settle(tester);
    await tester.tap(find.byKey(const ValueKey('ctx-编辑')));
    await _settle(tester);

    final remarks = tester.getRect(find.byKey(const ValueKey('field-remarks')));
    File('$evidenceDir/observations.json').writeAsStringSync(
      const JsonEncoder.withIndent(' ').convert({
        'recordingComplete': true,
        'baseline': 'frozen HEAD 25c907e',
        'remarksRect': {
          'x': remarks.left,
          'y': remarks.top,
          'w': remarks.width,
          'h': remarks.height,
        },
      }),
    );
    await tester.pump(const Duration(milliseconds: 200));
    final boundary = tester.renderObject<RenderRepaintBoundary>(
      find.byKey(_frame),
    );
    final image = await boundary.toImage(pixelRatio: 1);
    final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
    if (bytes != null) {
      await File('$evidenceDir/00-editor-before.png')
          .writeAsBytes(bytes.buffer.asUint8List());
    }
    image.dispose();

    await tester.tap(find.byKey(const ValueKey('editor-cancel')));
    await _settle(tester);
    await tester.pumpWidget(const SizedBox.shrink());
    await _settle(tester);
  });
}
