// R4-18.P01 contract (VMess): new -> save -> reopen field consistency, cancel,
// invalid/ missing field errors, and the protocol-specific defaults that feed
// the codegen consumer. Synthetic DTOs only; no FRB/native/kernel/network.
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

ProfileDraft _newVmess() => ProfileDraft()
  ..configType = ConfigType.vmess
  ..coreType = CoreType.xray
  ..remarks = 'vmess'
  ..address = '192.0.2.1'
  ..port = 443
  ..password = '11111111-2222-3333-4444-555555555555';

void main() {
  testWidgets('R4-18.P01 save -> reopen keeps security=auto and fields', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newVmess(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();

    expect(saved, hasLength(1));
    final dto = saved.single;
    expect(dto.remarks, 'vmess');
    expect(dto.address, '192.0.2.1');
    expect(dto.port, 443);
    expect(dto.password, '11111111-2222-3333-4444-555555555555');
    expect(dto.protoExtra.vmessSecurity, 'auto');
    expect(dto.protoExtra.alterId, isNull);

    // Reopen from the persisted DTO: same visible security value.
    final reopened = ProfileDraft.fromDto(dto);
    expect(reopened.vmessSecurity, 'auto');
    final saved2 = <c.ProfileDto>[];
    await tester.pumpWidget(_host(reopened, saved2));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    expect(find.text('auto'), findsWidgets);
  });

  testWidgets('R4-18.P01 cancel writes nothing', (tester) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newVmess(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(const ValueKey('field-remarks')), 'x');
    await tester.tap(find.byKey(const ValueKey('editor-cancel')));
    await tester.pumpAndSettle();
    expect(find.byType(ProfileEditorDialog), findsNothing);
    expect(saved, isEmpty);
  });

  testWidgets('R4-18.P01 invalid UUID is blocked with a visible error', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    final draft = _newVmess()..password = 'not-a-uuid';
    await tester.pumpWidget(_host(draft, saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(find.text('UUID 格式无效'), findsOneWidget);
    expect(saved, isEmpty);
  });

  test('R4-18.P01 codegen boundary: VMess security/alterId feed', () {
    // The DTO carries the protocol-extra keys verbatim to the Rust codegen;
    // the mapping itself is covered by config_codegen xray_protocols /
    // singbox_protocols. Here we lock the field boundary.
    final draft = _newVmess()
      ..vmessSecurity = 'chacha20-poly1305'
      ..alterId = '4';
    final extra = draft.toDto().protoExtra;
    expect(extra.vmessSecurity, 'chacha20-poly1305');
    expect(extra.alterId, '4');
  });
}
