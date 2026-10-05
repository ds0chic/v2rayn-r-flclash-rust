// R4-19 contract: full-config template window (dual-core tabs, validation,
// cancel-no-write, save-both, readable/retryable errors, unknown-key
// preservation). Synthetic only; no native bridge, network, or ports.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/groups.dart' as g;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/template_window.dart';

g.FullConfigTemplateDto _row(
  CoreType core,
  String id, {
  bool enabled = false,
  String? config,
}) => g.FullConfigTemplateDto(
  id: id,
  remarks: core == CoreType.singBox ? 'sing-box' : 'V2ray',
  enabled: enabled,
  coreType: core,
  config: config,
);

Future<void> _open(
  WidgetTester tester, {
  required List<g.FullConfigTemplateDto> initial,
  required g.TemplateDtoResult Function(g.FullConfigTemplateDto) onSave,
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
                initial: initial,
                onSave: onSave,
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

g.TemplateDtoResult _accept(g.FullConfigTemplateDto item) =>
    g.TemplateDtoResult(ok: true, item: item);

void main() {
  testWidgets('renders both core tabs', (tester) async {
    await _open(
      tester,
      initial: <g.FullConfigTemplateDto>[
        _row(CoreType.xray, 'builtin-xray'),
        _row(CoreType.singBox, 'builtin-singbox'),
      ],
      onSave: _accept,
    );
    expect(find.byKey(const ValueKey('template-window')), findsOneWidget);
    expect(find.text('V2ray (Xray)'), findsOneWidget);
    expect(find.text('sing-box'), findsOneWidget);
  });

  testWidgets('rejects invalid JSON with a readable message', (tester) async {
    await _open(
      tester,
      initial: <g.FullConfigTemplateDto>[
        _row(CoreType.xray, 'builtin-xray'),
        _row(CoreType.singBox, 'builtin-singbox'),
      ],
      onSave: _accept,
    );
    await tester.enterText(
      find.byKey(const ValueKey('template-config-xray')),
      '{not json',
    );
    await tester.tap(find.byKey(const ValueKey('template-save')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('template-window')), findsOneWidget);
    expect(find.text('JSON 解析失败'), findsOneWidget);
  });

  testWidgets('cancel writes nothing', (tester) async {
    final saved = <g.FullConfigTemplateDto>[];
    await _open(
      tester,
      initial: <g.FullConfigTemplateDto>[
        _row(CoreType.xray, 'builtin-xray'),
        _row(CoreType.singBox, 'builtin-singbox'),
      ],
      onSave: (item) {
        saved.add(item);
        return _accept(item);
      },
    );
    await tester.enterText(
      find.byKey(const ValueKey('template-config-xray')),
      '{"log": {}}',
    );
    await tester.tap(find.byKey(const ValueKey('template-cancel')));
    await tester.pumpAndSettle();
    expect(saved, isEmpty);
    expect(find.byKey(const ValueKey('template-window')), findsNothing);
  });

  testWidgets('saves both rows with flags and trims detour', (tester) async {
    final saved = <g.FullConfigTemplateDto>[];
    await _open(
      tester,
      initial: <g.FullConfigTemplateDto>[
        _row(CoreType.xray, 'builtin-xray'),
        _row(CoreType.singBox, 'builtin-singbox'),
      ],
      onSave: (item) {
        saved.add(item);
        return _accept(item);
      },
    );
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
      '  direct  ',
    );
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
    expect(xray.config, contains('loglevel'));
  });

  testWidgets('a thrown save error is readable and retryable', (tester) async {
    await _open(
      tester,
      initial: <g.FullConfigTemplateDto>[
        _row(CoreType.xray, 'builtin-xray'),
        _row(CoreType.singBox, 'builtin-singbox'),
      ],
      onSave: (item) {
        if (item.coreType == CoreType.singBox) {
          throw StateError('native boom');
        }
        return _accept(item);
      },
    );
    await tester.tap(find.byKey(const ValueKey('template-save')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('template-error')), findsOneWidget);
    final save = tester.widget<FilledButton>(
      find.byKey(const ValueKey('template-save')),
    );
    expect(save.onPressed, isNotNull);
  });

  testWidgets('preserves unknown template keys on round-trip', (tester) async {
    final saved = <g.FullConfigTemplateDto>[];
    const raw =
        '{"log": {"loglevel": "warning"}, "r4_19_unknown_section": {"keep": true}, "experimental": {"clash_api": {"external_controller": "127.0.0.1:19090"}}}';
    await _open(
      tester,
      initial: <g.FullConfigTemplateDto>[
        _row(CoreType.xray, 'builtin-xray', enabled: true, config: raw),
        _row(CoreType.singBox, 'builtin-singbox'),
      ],
      onSave: (item) {
        saved.add(item);
        return _accept(item);
      },
    );
    await tester.tap(find.byKey(const ValueKey('template-save')));
    await tester.pumpAndSettle();
    final xray = saved.firstWhere((t) => t.coreType == CoreType.xray);
    expect(xray.config, contains('r4_19_unknown_section'));
    expect(xray.config, contains('external_controller'));
  });
}
