// R4-18.P11 contract (NaiveProxy): username/password/QUIC/congestion/
// concurrency/UOT, TLS default, sing-box-only core, transport hidden,
// save -> reopen, cancel, missing password.
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

ProfileDraft _newNaive() => ProfileDraft()
  ..configType = ConfigType.naive
  ..coreType = CoreType.singBox
  ..remarks = 'naive'
  ..address = '192.0.2.11'
  ..port = 443
  ..username = 'naive-user'
  ..password = 'naive-pass'
  ..congestionControl = 'bbr2'
  ..naiveQuic = true
  ..insecureConcurrency = 4
  ..uot = true;

void main() {
  testWidgets('P11 save -> reopen keeps Naive fields + TLS default', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newNaive(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    expect(saved.single.username, 'naive-user');
    expect(saved.single.password, 'naive-pass');
    expect(saved.single.protoExtra.congestionControl, 'bbr2');
    expect(saved.single.protoExtra.naiveQuic, isTrue);
    expect(saved.single.protoExtra.insecureConcurrency, 4);
    expect(saved.single.protoExtra.uot, isTrue);
    expect(saved.single.security.streamSecurity, 'tls');
    final reopened = ProfileDraft.fromDto(saved.single);
    expect(reopened.congestionControl, 'bbr2');
    expect(reopened.username, 'naive-user');
  });

  testWidgets('P11 missing password is a visible error', (tester) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newNaive()..password = '', saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.byKey(const ValueKey('editor-save')));
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(find.text('必填'), findsWidgets);
    expect(saved, isEmpty);
  });

  testWidgets('P11 cancel never saves', (tester) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newNaive(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-cancel')));
    await tester.pumpAndSettle();
    expect(saved, isEmpty);
  });

  test('P11 Naive keeps TLS, no Reality, sing-box only, no transport', () {
    expect(ProfileCapabilities.supportsTls(ConfigType.naive), isTrue);
    expect(ProfileCapabilities.supportsReality(ConfigType.naive), isFalse);
    expect(ProfileCapabilities.allowedCores(ConfigType.naive), const <CoreType>[
      CoreType.singBox,
    ]);
    expect(ProfileCapabilities.supportsTransport(ConfigType.naive), isFalse);
  });
}
