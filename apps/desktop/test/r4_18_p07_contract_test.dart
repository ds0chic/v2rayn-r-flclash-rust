// R4-18.P07 contract (Hysteria2): password required, obfs/ports/hop fields,
// save -> reopen consistency, cancel, TLS default, transport grid hidden.
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

ProfileDraft _newHy2() => ProfileDraft()
  ..configType = ConfigType.hysteria2
  ..coreType = CoreType.singBox
  ..remarks = 'hy2'
  ..address = '192.0.2.7'
  ..port = 443
  ..password = 'hy2-pass'
  ..salamanderPass = 'obfs-pass'
  ..ports = '443-8443'
  ..hopInterval = '30'
  ..upMbps = 50
  ..downMbps = 100;

void main() {
  testWidgets('P07 save -> reopen keeps Hysteria2 obfs/ports/hop', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newHy2(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.byKey(const ValueKey('editor-save')));
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    expect(saved.single.password, 'hy2-pass');
    expect(saved.single.protoExtra.salamanderPass, 'obfs-pass');
    expect(saved.single.protoExtra.ports, '443-8443');
    expect(saved.single.protoExtra.hopInterval, '30');
    expect(saved.single.protoExtra.upMbps, 50);
    expect(saved.single.protoExtra.downMbps, 100);
    // Upstream forces TLS for Hysteria2 when empty.
    expect(saved.single.security.streamSecurity, 'tls');

    final reopened = ProfileDraft.fromDto(saved.single);
    expect(reopened.salamanderPass, 'obfs-pass');
    expect(reopened.ports, '443-8443');
    expect(reopened.hopInterval, '30');
  });

  testWidgets('P07 missing password is a visible error, nothing saved', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newHy2()..password = '', saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.byKey(const ValueKey('editor-save')));
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(find.text('必填'), findsWidgets);
    expect(saved, isEmpty);
  });

  testWidgets('P07 cancel never saves', (tester) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newHy2(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-cancel')));
    await tester.pumpAndSettle();
    expect(saved, isEmpty);
    expect(find.byKey(const ValueKey('profile-editor')), findsNothing);
  });

  test('P07 Hysteria2 has no Reality and no transport', () {
    expect(ProfileCapabilities.supportsReality(ConfigType.hysteria2), isFalse);
    expect(
      ProfileCapabilities.supportsTransport(ConfigType.hysteria2),
      isFalse,
    );
  });
}
