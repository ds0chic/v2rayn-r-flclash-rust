// R4-18.P06 repro (Trojan). Expected to FAIL pre-fix: upstream
// `AddServerWindow.xaml.cs` binds `cmbFlow6` -> Flow for Trojan
// (`Global.Flows`), but `protocolFields(ConfigType.trojan)` does not expose a
// Flow field. Synthetic only; no native library, kernel or network.
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
  test('P06 Trojan exposes the upstream Flow field', () {
    final fields = protocolFields(ConfigType.trojan);
    final flow = fields.where((f) => f.key == 'flow').toList();
    expect(flow, hasLength(1));
    expect(flow.single.options!.map((o) => o.value).toList(), const <String?>[
      null,
      'xtls-rprx-vision',
      'xtls-rprx-vision-udp443',
    ]);
  });

  testWidgets('P06 Trojan editor renders and saves Flow', (tester) async {
    final saved = <c.ProfileDto>[];
    final draft = ProfileDraft()
      ..configType = ConfigType.trojan
      ..coreType = CoreType.xray
      ..remarks = 'trojan'
      ..address = '192.0.2.6'
      ..port = 443
      ..password = 'trojan-pass'
      ..flow = 'xtls-rprx-vision';
    await tester.pumpWidget(_host(draft, saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('field-flow')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    expect(saved.single.protoExtra.flow, 'xtls-rprx-vision');
  });
}
