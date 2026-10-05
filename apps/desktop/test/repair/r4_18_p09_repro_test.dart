// R4-18.P09 repro (WireGuard). Expected to FAIL pre-fix:
//  - upstream `AddServerWindow.InitializeData` collapses `gridTls` and
//    `gridTransport` for WireGuard; the editor still renders `field-streamSecurity`
//    and `row-network`.
// Synthetic only; no native library, kernel, network or user data.
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

ProfileDraft _newWg() => ProfileDraft()
  ..configType = ConfigType.wireGuard
  ..coreType = CoreType.singBox
  ..remarks = 'wg'
  ..address = '192.0.2.9'
  ..port = 51820
  ..password = 'AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8='
  ..wgPublicKey = 'BAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8='
  ..wgInterfaceAddress = '10.0.0.2/32';

void main() {
  testWidgets('P09 WireGuard editor hides TLS and transport grids', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newWg(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('field-wgPublicKey')), findsOneWidget);
    expect(find.byKey(const ValueKey('field-wgReserved')), findsOneWidget);
    expect(find.byKey(const ValueKey('field-streamSecurity')), findsNothing);
    expect(find.byKey(const ValueKey('row-network')), findsNothing);
  });
}
