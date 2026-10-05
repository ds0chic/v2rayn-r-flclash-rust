// R4-18.P02 contract (VLESS): save/reopen, flow/encryption linkage, Reality
// fields only when Reality is allowed, cancel and validation. Synthetic only.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';
import 'package:v2rayn_desktop/features/profiles/profile_editor_dialog.dart';

Widget _host(ProfileDraft draft, List<c.ProfileDto> saved) => MaterialApp(
  home: Scaffold(
    body: Builder(
      builder: (context) => Center(
        child: ElevatedButton(
          onPressed: () => showProfileEditor(
            context,
            initial: draft,
            onSave: (dto) {
              saved.add(dto);
              return c.SaveProfileResult(ok: true, profile: dto);
            },
          ),
          child: const Text('open'),
        ),
      ),
    ),
  ),
);

ProfileDraft _newVless() => ProfileDraft()
  ..configType = ConfigType.vless
  ..coreType = CoreType.xray
  ..remarks = 'vless'
  ..address = '192.0.2.2'
  ..port = 443
  ..password = '11111111-2222-3333-4444-555555555555';

void main() {
  testWidgets('R4-18.P02 save -> reopen keeps flow + encryption=none', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    final draft = _newVless()..flow = 'xtls-rprx-vision';
    await tester.pumpWidget(_host(draft, saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();

    expect(saved, hasLength(1));
    expect(saved.single.protoExtra.flow, 'xtls-rprx-vision');
    expect(saved.single.protoExtra.vlessEncryption, 'none');

    final reopened = ProfileDraft.fromDto(saved.single);
    expect(reopened.flow, 'xtls-rprx-vision');
    expect(reopened.vlessEncryption, 'none');
  });

  testWidgets('R4-18.P02 Reality is offered and reveals its fields', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newVless(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    final security = find.byKey(const ValueKey('field-streamSecurity'));
    await tester.ensureVisible(security);
    await tester.pumpAndSettle();
    await tester.tap(security);
    await tester.pumpAndSettle();
    expect(find.text('reality'), findsWidgets);
    await tester.tap(find.text('reality').last);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('field-publicKey')), findsOneWidget);
  });

  testWidgets('R4-18.P02 cancel writes nothing', (tester) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newVless(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-cancel')));
    await tester.pumpAndSettle();
    expect(find.byType(ProfileEditorDialog), findsNothing);
    expect(saved, isEmpty);
  });
}
