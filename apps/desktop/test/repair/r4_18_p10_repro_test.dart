// R4-18.P10 repro (AnyTLS). Expected to FAIL pre-fix:
//  - upstream `AddServerWindow.InitializeData` keeps `gridTls` (and adds Reality)
//    for Anytls and collapses `gridTransport`; `ProfileCapabilities.supportsTls`
//    excludes Anytls and the editor still renders `row-network`.
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

ProfileDraft _newAny() => ProfileDraft()
  ..configType = ConfigType.anytls
  ..coreType = CoreType.singBox
  ..remarks = 'anytls'
  ..address = '192.0.2.10'
  ..port = 443
  ..password = 'anytls-pass';

void main() {
  test('P10 AnyTLS keeps TLS and Reality (upstream adds Reality)', () {
    expect(ProfileCapabilities.supportsTls(ConfigType.anytls), isTrue);
    expect(ProfileCapabilities.supportsReality(ConfigType.anytls), isTrue);
    expect(
      ProfileCapabilities.allowedCores(ConfigType.anytls),
      const <CoreType>[CoreType.singBox],
    );
  });

  testWidgets('P10 AnyTLS editor keeps TLS and hides transport', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newAny(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('field-password')), findsOneWidget);
    expect(find.byKey(const ValueKey('field-streamSecurity')), findsOneWidget);
    expect(find.byKey(const ValueKey('row-network')), findsNothing);
  });
}
