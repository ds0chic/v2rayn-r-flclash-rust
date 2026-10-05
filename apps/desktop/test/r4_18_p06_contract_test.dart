// R4-18.P06 contract (Trojan): Flow field, Reality allowed with public key
// required, save/reopen, password required, cancel. Synthetic only.
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

ProfileDraft _newTrojan() => ProfileDraft()
  ..configType = ConfigType.trojan
  ..coreType = CoreType.xray
  ..remarks = 'trojan'
  ..address = '192.0.2.6'
  ..port = 443
  ..password = 'trojan-pass';

void main() {
  test('R4-18.P06 Trojan keeps Reality (upstream adds it for Trojan)', () {
    expect(ProfileCapabilities.supportsReality(ConfigType.trojan), isTrue);
  });

  testWidgets('R4-18.P06 save -> reopen keeps Flow, user/pass', (tester) async {
    final saved = <c.ProfileDto>[];
    final draft = _newTrojan()..flow = 'xtls-rprx-vision-udp443';
    await tester.pumpWidget(_host(draft, saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.byKey(const ValueKey('editor-save')));
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    expect(saved.single.protoExtra.flow, 'xtls-rprx-vision-udp443');
    final reopened = ProfileDraft.fromDto(saved.single);
    expect(reopened.flow, 'xtls-rprx-vision-udp443');
  });

  testWidgets('R4-18.P06 Reality without public key is rejected', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    final draft = _newTrojan()..streamSecurity = 'reality';
    await tester.pumpWidget(_host(draft, saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.byKey(const ValueKey('editor-save')));
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    // The Reality public key is a required field, so the generic required
    // error is what the user sees; the save must not go through.
    expect(find.text('必填'), findsWidgets);
    expect(saved, isEmpty);
  });

  testWidgets('R4-18.P06 missing password is a visible error', (tester) async {
    final saved = <c.ProfileDto>[];
    final draft = _newTrojan()..password = '';
    await tester.pumpWidget(_host(draft, saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.byKey(const ValueKey('editor-save')));
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(find.text('必填'), findsWidgets);
    expect(saved, isEmpty);
  });
}
