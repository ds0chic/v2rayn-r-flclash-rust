// T10: template window renders dual tabs, validates JSON, saves both rows.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/groups.dart' as g;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/template_window.dart';

List<g.FullConfigTemplateDto> _rows() => const <g.FullConfigTemplateDto>[
  g.FullConfigTemplateDto(
    id: 'builtin-xray',
    remarks: 'V2ray',
    enabled: false,
    coreType: CoreType.xray,
  ),
  g.FullConfigTemplateDto(
    id: 'builtin-singbox',
    remarks: 'sing-box',
    enabled: false,
    coreType: CoreType.singBox,
  ),
];

Future<void> _open(
  WidgetTester tester, {
  required List<g.FullConfigTemplateDto> saved,
}) async {
  tester.view.physicalSize = const Size(1280, 1024);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(
    MaterialApp(
      home: Scaffold(
        body: Builder(
          builder: (context) => Center(
            child: ElevatedButton(
              onPressed: () async => showFullConfigTemplateWindow(
                context,
                initial: _rows(),
                onSave: (item) {
                  saved.add(item);
                  return g.TemplateDtoResult(ok: true, item: item);
                },
              ),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.text('open'));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('template window renders both core tabs', (tester) async {
    await _open(tester, saved: <g.FullConfigTemplateDto>[]);
    expect(find.byKey(const ValueKey('template-window')), findsOneWidget);
    expect(find.text('V2ray (Xray)'), findsOneWidget);
    expect(find.text('sing-box'), findsOneWidget);
    expect(find.byKey(const ValueKey('template-config-xray')), findsOneWidget);
    expect(find.byKey(const ValueKey('template-config-singbox')), findsNothing);
  });

  testWidgets('template window rejects invalid json', (tester) async {
    await _open(tester, saved: <g.FullConfigTemplateDto>[]);
    await tester.enterText(
      find.byKey(const ValueKey('template-config-xray')),
      '{not json',
    );
    await tester.tap(find.byKey(const ValueKey('template-save')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('template-window')), findsOneWidget);
    expect(find.text('JSON 解析失败'), findsOneWidget);
  });

  testWidgets('template window cancel saves nothing', (tester) async {
    final saved = <g.FullConfigTemplateDto>[];
    await _open(tester, saved: saved);
    await tester.enterText(
      find.byKey(const ValueKey('template-config-xray')),
      '{"log": {}}',
    );
    await tester.tap(find.byKey(const ValueKey('template-cancel')));
    await tester.pumpAndSettle();
    expect(saved, isEmpty);
    expect(find.byKey(const ValueKey('template-window')), findsNothing);
  });

  testWidgets('template window saves both rows with flags', (tester) async {
    final saved = <g.FullConfigTemplateDto>[];
    await _open(tester, saved: saved);
    await tester.enterText(
      find.byKey(const ValueKey('template-config-xray')),
      '{"log": {"loglevel": "warning"}}',
    );
    await tester.tap(find.byKey(const ValueKey('template-enabled-xray')));
    await tester.pumpAndSettle();
    await tester.tap(
      find.byKey(const ValueKey('template-add-proxy-only-xray')),
    );
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('template-detour-xray')),
      'direct',
    );
    // Switch to the sing-box tab and fill its config too.
    await tester.tap(find.text('sing-box'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('template-config-singbox')),
      '{"log": {}}',
    );
    await tester.tap(find.byKey(const ValueKey('template-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(2));
    final xray = saved.firstWhere((t) => t.coreType == CoreType.xray);
    expect(xray.enabled, isTrue);
    expect(xray.addProxyOnly, isTrue);
    expect(xray.proxyDetour, 'direct');
    expect(find.byKey(const ValueKey('template-window')), findsNothing);
  });
}
