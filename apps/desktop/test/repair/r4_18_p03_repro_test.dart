// R4-18.P03 repro (Shadowsocks). Expected to FAIL pre-fix:
//  - `AddServerWindow.SetStreamSecurity` only offers ""/"tls` for Shadowsocks
//    (no Reality), but the editor currently always offers Reality.
//  - A method outside the selected core's `Global.SsSecuritiesIn*` list must be
//    rejected so the codegen never silently downgrades it to "none".
// Synthetic data only; no native library, kernel, network or user data.
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

ProfileDraft _newSs() => ProfileDraft()
  ..configType = ConfigType.shadowsocks
  ..coreType = CoreType.xray
  ..remarks = 'ss'
  ..address = '192.0.2.3'
  ..port = 8388
  ..password = 'secret'
  ..ssMethod = 'aes-256-gcm';

void main() {
  testWidgets('P03 Shadowsocks security list excludes Reality', (tester) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newSs(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    final security = find.byKey(const ValueKey('field-streamSecurity'));
    await tester.ensureVisible(security);
    await tester.pumpAndSettle();
    await tester.tap(security);
    await tester.pumpAndSettle();
    expect(find.text('reality'), findsNothing);
  });

  testWidgets('P03 SS method unsupported by the selected core is rejected', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    final draft = _newSs()
      ..coreType = CoreType.singBox
      ..ssMethod = 'plain'; // Xray-only; not in SsSecuritiesInSingbox.
    await tester.pumpWidget(_host(draft, saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(find.text('当前内核不支持该加密方式'), findsOneWidget);
    expect(saved, isEmpty);
  });
}
