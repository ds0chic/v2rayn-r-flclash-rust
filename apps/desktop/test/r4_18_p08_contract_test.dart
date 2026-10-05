// R4-18.P08 contract (TUIC): UUID(username)+password+congestion, TLS/h3
// defaults, sing-box-only core, save -> reopen, cancel, invalid UUID.
// Synthetic only; no native library, kernel or network.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';
import 'package:v2rayn_desktop/features/profiles/profile_editor_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/profile_fields.dart';

const _uuid = '11111111-2222-3333-4444-555555555555';

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

ProfileDraft _newTuic() => ProfileDraft()
  ..configType = ConfigType.tuic
  ..coreType = CoreType.singBox
  ..remarks = 'tuic'
  ..address = '192.0.2.8'
  ..port = 443
  ..username = _uuid
  ..password = 'tuic-pass';

void main() {
  testWidgets('P08 save -> reopen keeps UUID/password and TLS+h3 defaults', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newTuic(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    expect(saved.single.username, _uuid);
    expect(saved.single.password, 'tuic-pass');
    expect(saved.single.security.streamSecurity, 'tls');
    expect(saved.single.security.alpn, 'h3');
    final reopened = ProfileDraft.fromDto(saved.single);
    expect(reopened.username, _uuid);
    expect(reopened.password, 'tuic-pass');
  });

  testWidgets('P08 invalid UUID is a readable error', (tester) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newTuic()..username = 'not-a-uuid', saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(find.text('UUID 格式无效'), findsOneWidget);
    expect(saved, isEmpty);
  });

  testWidgets('P08 cancel never saves', (tester) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newTuic(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-cancel')));
    await tester.pumpAndSettle();
    expect(saved, isEmpty);
  });

  test('P08 TUIC is sing-box only and has no Reality/transport', () {
    expect(ProfileCapabilities.allowedCores(ConfigType.tuic), const <CoreType>[
      CoreType.singBox,
    ]);
    expect(ProfileCapabilities.supportsReality(ConfigType.tuic), isFalse);
    expect(ProfileCapabilities.supportsTransport(ConfigType.tuic), isFalse);
  });
}
