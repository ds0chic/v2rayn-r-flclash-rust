// R4-19 repro: a full-config template save that throws must surface a readable
// error and leave the dialog retryable instead of freezing it with
// `_submitting = true` (which the pre-fix window did by letting the exception
// escape the `onPressed` callback).
//
// Synthetic only: in-memory onSave callbacks, no native library, no network,
// no user data, no ports.
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
                initial: _rows(),
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

void main() {
  testWidgets('a thrown template save error is readable and retryable', (
    tester,
  ) async {
    var attempts = 0;
    await _open(
      tester,
      onSave: (item) {
        attempts++;
        // Fail the sing-box row by throwing, as a native bridge failure does.
        if (item.coreType == CoreType.singBox) {
          throw StateError('native boom');
        }
        return g.TemplateDtoResult(ok: true, item: item);
      },
    );

    await tester.enterText(
      find.byKey(const ValueKey('template-config-xray')),
      '{"log": {"loglevel": "warning"}}',
    );
    await tester.tap(find.text('sing-box'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('template-config-singbox')),
      '{"log": {}}',
    );
    await tester.tap(find.byKey(const ValueKey('template-save')));
    await tester.pumpAndSettle();

    // The dialog must stay open with a readable error, not trap the user.
    expect(find.byKey(const ValueKey('template-window')), findsOneWidget);
    expect(
      find.byKey(const ValueKey('template-error')),
      findsOneWidget,
      reason: 'a thrown save failure must be shown, not swallowed',
    );
    final save = tester.widget<FilledButton>(
      find.byKey(const ValueKey('template-save')),
    );
    expect(
      save.onPressed,
      isNotNull,
      reason: 'the save button must re-enable so the user can retry',
    );
    expect(attempts, greaterThanOrEqualTo(2));
  });
}
