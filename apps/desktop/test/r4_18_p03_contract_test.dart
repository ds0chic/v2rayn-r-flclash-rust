// R4-18.P03 contract (Shadowsocks): method list by core, UOT round-trip,
// save/reopen, cancel. Synthetic only; no native library, kernel or network.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';
import 'package:v2rayn_desktop/features/profiles/profile_editor_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/profile_fields.dart';

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

ProfileDraft _newSs() => ProfileDraft()
  ..configType = ConfigType.shadowsocks
  ..coreType = CoreType.xray
  ..remarks = 'ss'
  ..address = '192.0.2.3'
  ..port = 8388
  ..password = 'secret'
  ..ssMethod = 'aes-256-gcm';

void main() {
  test('R4-18.P03 method list follows the selected core', () {
    final xray = shadowsocksMethods(CoreType.xray);
    final singbox = shadowsocksMethods(CoreType.singBox);
    expect(xray, contains('plain'));
    expect(singbox, isNot(contains('plain')));
    expect(singbox, contains('rc4-md5'));
  });

  testWidgets('R4-18.P03 save -> reopen keeps method + uot', (tester) async {
    final saved = <c.ProfileDto>[];
    final draft = _newSs()..uot = true;
    await tester.pumpWidget(_host(draft, saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    expect(saved.single.protoExtra.ssMethod, 'aes-256-gcm');
    expect(saved.single.protoExtra.uot, isTrue);

    final reopened = ProfileDraft.fromDto(saved.single);
    expect(reopened.ssMethod, 'aes-256-gcm');
    expect(reopened.uot, isTrue);
  });

  testWidgets('R4-18.P03 empty method is a visible required error', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    final draft = _newSs()..ssMethod = null;
    await tester.pumpWidget(_host(draft, saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(find.text('必填'), findsWidgets);
    expect(saved, isEmpty);
  });

  testWidgets('R4-18.P03 cancel writes nothing', (tester) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newSs(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-cancel')));
    await tester.pumpAndSettle();
    expect(find.byType(ProfileEditorDialog), findsNothing);
    expect(saved, isEmpty);
  });
}
