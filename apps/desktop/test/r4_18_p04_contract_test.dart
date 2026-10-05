// R4-18.P04 contract (SOCKS): user/pass optional and round-tripped, save/reopen,
// cancel. Synthetic only; no native library, kernel or network.
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

ProfileDraft _newSocks() => ProfileDraft()
  ..configType = ConfigType.socks
  ..coreType = CoreType.xray
  ..remarks = 'socks'
  ..address = '192.0.2.4'
  ..port = 1080;

void main() {
  testWidgets('R4-18.P04 save without credentials succeeds', (tester) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newSocks(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    expect(saved.single.username, '');
    expect(saved.single.password, '');
  });

  testWidgets('R4-18.P04 save -> reopen keeps user/pass', (tester) async {
    final saved = <c.ProfileDto>[];
    final draft = _newSocks()
      ..username = 'u'
      ..password = 'p';
    await tester.pumpWidget(_host(draft, saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    final reopened = ProfileDraft.fromDto(saved.single);
    expect(reopened.username, 'u');
    expect(reopened.password, 'p');
  });

  testWidgets('R4-18.P04 missing address is a visible error', (tester) async {
    final saved = <c.ProfileDto>[];
    final draft = _newSocks()..address = '';
    await tester.pumpWidget(_host(draft, saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(find.text('必填'), findsWidgets);
    expect(saved, isEmpty);
  });
}
