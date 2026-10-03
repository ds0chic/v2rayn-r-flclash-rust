// UX-SPACE-03-VLESS: spacing, font propagation, draft/error/cancel semantics.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';
import 'package:v2rayn_desktop/features/profiles/profile_editor_dialog.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

ProfileDraft _vless() => ProfileDraft()
  ..configType = ConfigType.vless
  ..coreType = CoreType.xray
  ..remarks = '原备注'
  ..address = '2001:db8::1'
  ..port = 443
  ..network = 'raw'
  ..password = '00000000-0000-0000-0000-000000000000';

Future<void> _pumpEditor(
  WidgetTester tester, {
  required ProfileDraft draft,
  required c.SaveProfileResult Function(c.ProfileDto) onSave,
  Brightness brightness = Brightness.light,
  double? fontSize,
}) async {
  await tester.pumpWidget(
    MaterialApp(
      theme: buildAppTheme(brightness, fontSize: fontSize),
      home: Scaffold(
        body: Builder(
          builder: (context) => Center(
            child: ElevatedButton(
              key: const ValueKey('ux-open'),
              onPressed: () =>
                  showProfileEditor(context, initial: draft, onSave: onSave),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    ),
  );
}

Future<void> _open(WidgetTester tester) async {
  await tester.tap(find.byKey(const ValueKey('ux-open')));
  await tester.pumpAndSettle();
}

double _h(WidgetTester tester, String key) =>
    tester.getRect(find.byKey(ValueKey(key))).height;

Rect _rect(WidgetTester tester, Finder finder) => tester.getRect(finder);

String? _initial(WidgetTester tester, String key) =>
    tester.widget<TextFormField>(find.byKey(ValueKey(key))).initialValue;

void main() {
  testWidgets(
    'invalid port keeps draft, cancel discards, reopen reads original',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1100, 780));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      var saveCalls = 0;
      final original = _vless();
      await _pumpEditor(
        tester,
        draft: original,
        onSave: (dto) {
          saveCalls += 1;
          return c.SaveProfileResult(ok: true, profile: dto);
        },
      );
      await _open(tester);
      expect(find.byKey(const ValueKey('profile-editor')), findsOneWidget);

      await tester.enterText(
        find.byKey(const ValueKey('field-remarks')),
        '改后备注',
      );
      await tester.enterText(find.byKey(const ValueKey('field-port')), '99999');
      await tester.tap(find.byKey(const ValueKey('editor-save')));
      await tester.pumpAndSettle();

      // Rejected before the bridge; the draft and the field error stay visible.
      expect(saveCalls, 0);
      expect(find.text('端口需在 1-65535'), findsOneWidget);
      expect(find.byKey(const ValueKey('profile-editor')), findsOneWidget);
      expect(_initial(tester, 'field-remarks'), '改后备注');
      expect(_initial(tester, 'field-port'), '99999');

      await tester.tap(find.byKey(const ValueKey('editor-cancel')));
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('profile-editor')), findsNothing);

      // Reopen reads the original stored values, not the discarded draft.
      await _open(tester);
      expect(_initial(tester, 'field-remarks'), '原备注');
      expect(_initial(tester, 'field-port'), '443');
    },
  );

  testWidgets('default metrics: control 34-36, field gap 8-12, label gap '
      '12-16, group gap ~16', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1100, 780));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await _pumpEditor(
      tester,
      draft: _vless(),
      onSave: (dto) => c.SaveProfileResult(ok: true, profile: dto),
    );
    await _open(tester);

    final remarks = _rect(tester, find.byKey(const ValueKey('field-remarks')));
    final address = _rect(tester, find.byKey(const ValueKey('field-address')));
    final label = _rect(tester, find.byKey(const ValueKey('label-remarks')));

    expect(remarks.height, inInclusiveRange(34, 37));
    // Label column to control horizontal net gap.
    final labelGap = remarks.left - (label.left + label.width);
    expect(labelGap, inInclusiveRange(12, 16));
    // Net vertical gap between two adjacent controls.
    expect(
      address.top - remarks.bottom,
      inInclusiveRange(8, 12),
      reason: 'field gap must stay 8-12',
    );

    // Section group gap: last field of 协议 -> 传输 title.
    final encryption = _rect(
      tester,
      find.byKey(const ValueKey('field-vlessEncryption')),
    );
    final transport = _rect(tester, find.byKey(const ValueKey('section-传输')));
    expect(transport.top - encryption.bottom, inInclusiveRange(14, 18));
  });

  testWidgets('field error grows its own row without covering the next field', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1100, 780));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await _pumpEditor(
      tester,
      draft: _vless(),
      onSave: (dto) => c.SaveProfileResult(ok: true, profile: dto),
    );
    await _open(tester);

    final baseHeight = _h(tester, 'field-remarks');
    await tester.enterText(find.byKey(const ValueKey('field-remarks')), '');
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();

    expect(find.text('必填'), findsWidgets);
    final withError = _rect(
      tester,
      find.byKey(const ValueKey('field-remarks')),
    );
    final address = _rect(tester, find.byKey(const ValueKey('field-address')));
    // The error reserves space inside the field box; the next field keeps its
    // 8-12 net gap instead of being overlapped.
    expect(withError.height, greaterThan(baseHeight));
    expect(address.top - withError.bottom, inInclusiveRange(8, 12));
  });

  testWidgets('user font size propagates to label, content and error', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1100, 780));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await _pumpEditor(
      tester,
      draft: _vless(),
      onSave: (dto) => c.SaveProfileResult(ok: true, profile: dto),
      fontSize: 20,
    );
    await _open(tester);

    final label = tester.widget<Text>(
      find.byKey(const ValueKey('label-remarks')),
    );
    expect(label.style?.fontSize, 20);
    final content = tester.widget<EditableText>(
      find.descendant(
        of: find.byKey(const ValueKey('field-remarks')),
        matching: find.byType(EditableText),
      ),
    );
    expect(content.style.fontSize, 20);

    await tester.enterText(find.byKey(const ValueKey('field-port')), '99999');
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    final error = tester.widget<Text>(find.text('端口需在 1-65535'));
    expect(error.style?.fontSize, 19);

    expect(tester.takeException(), isNull);
  });

  testWidgets('shared form does not clip VMess or Trojan fields', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1100, 780));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final cases = <ConfigType, String>{
      ConfigType.vmess: 'field-alterId',
      ConfigType.trojan: 'field-password',
    };
    for (final entry in cases.entries) {
      final draft = ProfileDraft()
        ..configType = entry.key
        ..coreType = CoreType.xray
        ..remarks = '抽查节点'
        ..address = '192.0.2.77'
        ..port = 443
        ..network = 'raw'
        ..password = 'x';
      await _pumpEditor(
        tester,
        draft: draft,
        onSave: (dto) => c.SaveProfileResult(ok: true, profile: dto),
      );
      await _open(tester);
      expect(
        find.byKey(ValueKey(entry.value)),
        findsOneWidget,
        reason: '${entry.key.name} field missing',
      );
      expect(_h(tester, entry.value), inInclusiveRange(34, 37));
      expect(find.byKey(const ValueKey('editor-save')), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.tap(find.byKey(const ValueKey('editor-cancel')));
      await tester.pumpAndSettle();
    }
  });
}
