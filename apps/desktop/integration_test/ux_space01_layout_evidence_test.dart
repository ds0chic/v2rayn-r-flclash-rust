// UX-SPACE-01 real-window evidence: renders the restored top entry row in the
// real Windows Flutter window at the required logical sizes / layouts / themes,
// saves PNGs and records the measured toolbar rectangle and wrap-run count.
//
// Data is synthetic (SyntheticBridgePort) so no real profile store or network
// is touched; the window itself is the live app window.
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:v2rayn_desktop/app/app.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/shared/widgets/adaptive_toolbar.dart';

import '../test/support/fake_monitor_bridge.dart';
import '../test/support/fake_platform_bridge.dart';

import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';

const _frame = ValueKey('space01-frame');

Future<void> _settle(WidgetTester tester) async {
  for (var i = 0; i < 6; i++) {
    await tester.pump(const Duration(milliseconds: 120));
  }
}

Future<void> _shot(WidgetTester tester, String dir, String name) async {
  await tester.pump(const Duration(milliseconds: 250));
  if (Platform.environment['V2RAYN_R_SPACE01_SKIP_IMAGES'] == '1') return;
  final boundary = tester.renderObject<RenderRepaintBoundary>(
    find.byKey(_frame),
  );
  final image = await boundary.toImage(pixelRatio: 1);
  final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
  if (bytes == null) throw StateError('screenshot encode failed');
  File('$dir/$name.png').writeAsBytesSync(bytes.buffer.asUint8List());
  image.dispose();
}

Map<String, double> _rect(WidgetTester tester, Finder finder) {
  final r = tester.getRect(finder);
  return {'x': r.left, 'y': r.top, 'width': r.width, 'height': r.height};
}

const _runFinders = <String>[
  'group-filter-all',
  'group-filter-sub-000',
  'group-filter-sub-001',
  'toolbar-sub-edit',
  'toolbar-sub-add',
  'filter-field',
  'toolbar-自动列宽',
  'toolbar-快速真延迟',
  'toolbar-混合',
  'toolbar-备注',
  'column-settings-button',
];

int _runs(WidgetTester tester) {
  final tops = <int>{};
  for (final id in _runFinders) {
    final f = find.byKey(ValueKey(id));
    if (f.evaluate().isEmpty) continue;
    tops.add(tester.getTopLeft(f).dy.round());
  }
  return tops.length;
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets('restored top entry row in the real window', (tester) async {
    final dir = Platform.environment['V2RAYN_R_SPACE01_EVIDENCE_DIR'];
    expect(dir, isNotNull, reason: 'set V2RAYN_R_SPACE01_EVIDENCE_DIR');
    Directory(dir!).createSync(recursive: true);
    final records = <Map<String, Object?>>[];

    // The shell runtime subscribes to Rust events even when the profiles table
    // is driven by the synthetic bridge, so the FRB layer must be up. The
    // isolated V2RAYN_R_DATA_DIR keeps the real profile store untouched.
    RustBridgeInit.configure(RustLib.init);
    await RustBridgeInit.init();

    void writeRecords() {
      File('$dir/observations.json').writeAsStringSync(
        const JsonEncoder.withIndent('  ').convert({
          'schema': 'ux-space01-real-window',
          'note':
              'logical surface sizes via setSurfaceSize; pixelRatio=1; '
              'synthetic bridge data; OS window is the live app window',
          'records': records,
        }),
      );
    }

    final bridge = SyntheticBridgePort();
    bridge.saveSubItem(
      const c.SubItemDto(
        id: 'sub-000',
        remarks: '演示订阅A',
        url: 'https://example.com/a',
        moreUrl: '',
        enabled: true,
        userAgent: '',
        sort: 1,
        autoUpdateInterval: 0,
        updateTime: 0,
      ),
    );
    bridge.saveSubItem(
      const c.SubItemDto(
        id: 'sub-001',
        remarks: '这是一个很长的中文订阅分组名称用于验证换行不裁剪',
        url: 'https://example.com/b',
        moreUrl: '',
        enabled: true,
        userAgent: '',
        sort: 2,
        autoUpdateInterval: 0,
        updateTime: 0,
      ),
    );

    runApp(
      ProviderScope(
        overrides: [
          bridgePortProvider.overrideWithValue(bridge),
          uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
          profileRowCountProvider.overrideWithValue(60),
          platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
          monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
        ],
        child: const RepaintBoundary(key: _frame, child: V2rayNRApp()),
      ),
    );
    for (
      var i = 0;
      i < 80 && find.byKey(const ValueKey('menu-配置项')).evaluate().isEmpty;
      i++
    ) {
      await tester.pump(const Duration(milliseconds: 150));
    }
    final container = ProviderScope.containerOf(
      tester.element(find.byType(V2rayNRApp)),
    );
    container.read(profilesControllerProvider.notifier).reload();
    final shell = container.read(uiShellControllerProvider.notifier);

    Future<void> capture(
      Size size,
      AppLayoutMode layout,
      ThemeMode theme,
      String name,
    ) async {
      await tester.binding.setSurfaceSize(size);
      shell.setLayout(layout);
      shell.setThemeMode(theme);
      await _settle(tester);
      await _shot(tester, dir, name);
      final toolbar = find.byType(AdaptiveToolbar).first;
      records.add({
        'name': name,
        'surface': {'width': size.width, 'height': size.height},
        'layout': layout.id,
        'theme': theme == ThemeMode.dark ? 'dark' : 'light',
        'toolbar': _rect(tester, toolbar),
        'filter': _rect(tester, find.byKey(const ValueKey('filter-field'))),
        'header': _rect(tester, find.byKey(const ValueKey('header-类型'))),
        'wrapRuns': _runs(tester),
      });
      writeRecords();
    }

    await capture(
      const Size(800, 600),
      AppLayoutMode.vertical,
      ThemeMode.light,
      '01-800x600-vertical-light',
    );
    await capture(
      const Size(1200, 800),
      AppLayoutMode.vertical,
      ThemeMode.light,
      '02-1200x800-vertical-light',
    );
    await capture(
      const Size(1920, 1080),
      AppLayoutMode.vertical,
      ThemeMode.light,
      '03-1920x1080-vertical-light',
    );
    await capture(
      const Size(1200, 800),
      AppLayoutMode.horizontal,
      ThemeMode.light,
      '04-1200x800-horizontal-light',
    );
    await capture(
      const Size(1200, 800),
      AppLayoutMode.tab,
      ThemeMode.light,
      '05-1200x800-tab-light',
    );
    await capture(
      const Size(1200, 800),
      AppLayoutMode.vertical,
      ThemeMode.dark,
      '06-1200x800-vertical-dark',
    );
    await capture(
      const Size(800, 600),
      AppLayoutMode.vertical,
      ThemeMode.dark,
      '07-800x600-vertical-dark',
    );

    await tester.binding.setSurfaceSize(null);
    records.add({'recordingComplete': true, 'count': records.length});
    writeRecords();
  });
}
