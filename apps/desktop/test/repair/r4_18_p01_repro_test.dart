// R4-18.P01 repro (VMess). Expected to FAIL against the current pre-fix code:
// upstream `AddServerViewModel` defaults an empty VMess security to
// `Global.DefaultSecurity = "auto"`, lists `Global.VmessSecurities` in its
// canonical order, and `AddServerWindow.SetStreamSecurity` never offers
// Reality for VMess. Synthetic data only; no native library, kernel, network
// or user data.
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
  test('P01 VMess security list/order and Reality capability', () {
    final fields = protocolFields(ConfigType.vmess);
    final security = fields.firstWhere((f) => f.key == 'vmessSecurity');
    expect(security.options!.map((o) => o.value).toList(), const <String?>[
      'aes-128-gcm',
      'chacha20-poly1305',
      'auto',
      'none',
      'zero',
    ]);
    expect(ProfileCapabilities.supportsReality(ConfigType.vmess), isFalse);
  });

  testWidgets('P01 new VMess save defaults security=auto and alterId unset', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    final draft = ProfileDraft()
      ..configType = ConfigType.vmess
      ..coreType = CoreType.xray
      ..remarks = 'vmess'
      ..address = '192.0.2.1'
      ..port = 443
      ..password = '11111111-2222-3333-4444-555555555555';
    await tester.pumpWidget(_host(draft, saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    expect(saved.single.protoExtra.vmessSecurity, 'auto');
    expect(saved.single.protoExtra.alterId, isNull);
  });
}
