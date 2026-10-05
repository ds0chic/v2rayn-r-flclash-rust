// R4-18.P10 contract (AnyTLS): password required, TLS+Reality applicable,
// sing-box-only core, TLS default, transport hidden, save -> reopen, cancel.
// Synthetic only; no native library, kernel or network.
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

ProfileDraft _newAny() => ProfileDraft()
  ..configType = ConfigType.anytls
  ..coreType = CoreType.singBox
  ..remarks = 'anytls'
  ..address = '192.0.2.10'
  ..port = 443
  ..password = 'anytls-pass'
  ..streamSecurity = 'reality'
  ..sni = 'sni.example'
  ..publicKey = 'PUBKEY'
  ..shortId = 'abcd';

void main() {
  testWidgets('P10 save -> reopen keeps AnyTLS password + Reality', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newAny(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    expect(saved.single.password, 'anytls-pass');
    expect(saved.single.security.streamSecurity, 'reality');
    expect(saved.single.security.publicKey, 'PUBKEY');
    final reopened = ProfileDraft.fromDto(saved.single);
    expect(reopened.password, 'anytls-pass');
    expect(reopened.streamSecurity, 'reality');
  });

  testWidgets('P10 AnyTLS forces TLS when empty', (tester) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newAny()..streamSecurity = null, saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    expect(saved.single.security.streamSecurity, 'tls');
  });

  testWidgets('P10 missing password is a visible error', (tester) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newAny()..password = '', saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.byKey(const ValueKey('editor-save')));
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(find.text('必填'), findsWidgets);
    expect(saved, isEmpty);
  });

  test('P10 AnyTLS keeps TLS/Reality, sing-box only, no transport', () {
    expect(ProfileCapabilities.supportsTls(ConfigType.anytls), isTrue);
    expect(ProfileCapabilities.supportsReality(ConfigType.anytls), isTrue);
    expect(
      ProfileCapabilities.allowedCores(ConfigType.anytls),
      const <CoreType>[CoreType.singBox],
    );
    expect(ProfileCapabilities.supportsTransport(ConfigType.anytls), isFalse);
  });
}
