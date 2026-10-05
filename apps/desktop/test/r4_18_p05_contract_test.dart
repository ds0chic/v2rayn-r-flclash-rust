// R4-18.P05 contract (HTTP): valid headers JSON round-trips, empty headers are
// allowed, save/reopen, cancel. Synthetic only; no native library or network.
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

ProfileDraft _newHttp() => ProfileDraft()
  ..configType = ConfigType.http
  ..coreType = CoreType.xray
  ..remarks = 'http'
  ..address = '192.0.2.5'
  ..port = 8080;

void main() {
  testWidgets('R4-18.P05 valid headers JSON save -> reopen', (tester) async {
    final saved = <c.ProfileDto>[];
    final draft = _newHttp()..httpHeaders = '{"Host":"example.com"}';
    await tester.pumpWidget(_host(draft, saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    expect(saved.single.protoExtra.httpHeaders, '{"Host":"example.com"}');

    final reopened = ProfileDraft.fromDto(saved.single);
    expect(reopened.httpHeaders, '{"Host":"example.com"}');
  });

  testWidgets('R4-18.P05 empty headers is allowed', (tester) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newHttp(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    expect(saved.single.protoExtra.httpHeaders, isNull);
  });

  testWidgets('R4-18.P05 cancel writes nothing', (tester) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newHttp(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-cancel')));
    await tester.pumpAndSettle();
    expect(find.byType(ProfileEditorDialog), findsNothing);
    expect(saved, isEmpty);
  });
}
