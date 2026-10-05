// R4-18.P04 repro (SOCKS). Expected to FAIL pre-fix: upstream
// `AddServerWindow.SetStreamSecurity` only offers ""/"tls" for SOCKS, yet the
// editor currently always offers Reality. Synthetic only.
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

void main() {
  testWidgets('P04 SOCKS security list excludes Reality', (tester) async {
    final saved = <c.ProfileDto>[];
    final draft = ProfileDraft()
      ..configType = ConfigType.socks
      ..coreType = CoreType.xray
      ..remarks = 'socks'
      ..address = '192.0.2.4'
      ..port = 1080;
    await tester.pumpWidget(_host(draft, saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    final security = find.byKey(const ValueKey('field-streamSecurity'));
    await tester.ensureVisible(security);
    await tester.pumpAndSettle();
    await tester.tap(security);
    await tester.pumpAndSettle();
    expect(find.text('reality'), findsNothing);
  });
}
