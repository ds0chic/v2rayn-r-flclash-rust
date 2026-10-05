// R4-18.P07 repro (Hysteria2). Expected to FAIL pre-fix:
//  - upstream `AddServerWindow.InitializeData` keeps the TLS grid for Hysteria2
//    (stream-security list is ["", "tls"]), yet `ProfileCapabilities.supportsTls`
//    excludes Hysteria2.
//  - upstream collapses `gridTransport` for Hysteria2; the editor still renders
//    the transport section (`row-network`).
// Synthetic only; no native library, kernel, network or user data.
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
  ..password = 'hy2-pass';

void main() {
  test('P07 Hysteria2 keeps TLS (upstream shows the TLS grid)', () {
    expect(ProfileCapabilities.supportsTls(ConfigType.hysteria2), isTrue);
    expect(ProfileCapabilities.supportsReality(ConfigType.hysteria2), isFalse);
  });

  testWidgets('P07 Hysteria2 editor hides the collapsed transport grid', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newHy2(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('field-password')), findsOneWidget);
    expect(find.byKey(const ValueKey('field-salamanderPass')), findsOneWidget);
    expect(find.byKey(const ValueKey('field-ports')), findsOneWidget);
    expect(find.byKey(const ValueKey('row-network')), findsNothing);
  });
}
