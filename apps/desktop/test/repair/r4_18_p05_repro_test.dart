// R4-18.P05 repro (HTTP). Expected to FAIL pre-fix:
//  - upstream `AddServerViewModel.SaveServerAsync` rejects a non-empty HTTP
//    headers string that is not valid JSON; the editor currently saves it and
//    the codegen silently drops it.
//  - `AddServerWindow.SetStreamSecurity` only offers ""/"tls" for HTTP, yet the
//    editor currently always offers Reality.
// Synthetic only; no native library, kernel or network.
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
  testWidgets('P05 HTTP security list excludes Reality', (tester) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newHttp(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    final security = find.byKey(const ValueKey('field-streamSecurity'));
    await tester.ensureVisible(security);
    await tester.pumpAndSettle();
    await tester.tap(security);
    await tester.pumpAndSettle();
    expect(find.text('reality'), findsNothing);
  });

  testWidgets('P05 invalid HTTP headers JSON is rejected', (tester) async {
    final saved = <c.ProfileDto>[];
    final draft = _newHttp()..httpHeaders = '{not-json';
    await tester.pumpWidget(_host(draft, saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(find.text('JSON 格式无效'), findsOneWidget);
    expect(saved, isEmpty);
  });
}
