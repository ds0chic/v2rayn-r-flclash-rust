// R4-18.P02 repro (VLESS). Expected to FAIL pre-fix: upstream
// `AddServerViewModel` defaults an empty VLESS encryption to `Global.None`
// ("none"). Synthetic data only; no native library, kernel, network or user
// data.
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

void main() {
  testWidgets('P02 new VLESS save defaults encryption=none', (tester) async {
    final saved = <c.ProfileDto>[];
    final draft = ProfileDraft()
      ..configType = ConfigType.vless
      ..coreType = CoreType.xray
      ..remarks = 'vless'
      ..address = '192.0.2.2'
      ..port = 443
      ..password = '11111111-2222-3333-4444-555555555555';
    await tester.pumpWidget(_host(draft, saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    expect(saved.single.protoExtra.vlessEncryption, 'none');
  });

  test('P02 VLESS supports Reality (upstream adds it for VLESS)', () {
    expect(ProfileCapabilities.supportsReality(ConfigType.vless), isTrue);
  });
}
