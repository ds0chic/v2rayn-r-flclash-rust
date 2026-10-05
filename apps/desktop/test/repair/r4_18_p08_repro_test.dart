// R4-18.P08 repro (TUIC). Expected to FAIL pre-fix:
//  - upstream `AddServerWindow.InitializeData` collapses `gridTransport` for
//    TUIC and sets `Network = ""`; the editor still renders `row-network`.
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

ProfileDraft _newTuic() => ProfileDraft()
  ..configType = ConfigType.tuic
  ..coreType = CoreType.singBox
  ..remarks = 'tuic'
  ..address = '192.0.2.8'
  ..port = 443
  ..username = '11111111-2222-3333-4444-555555555555'
  ..password = 'tuic-pass';

void main() {
  test(
    'P08 TUIC congestion list matches upstream and core is sing-box only',
    () {
      final congestion = protocolFields(ConfigType.tuic)
          .firstWhere((f) => f.key == 'congestionControl');
      expect(congestion.options!.map((o) => o.value).toList(), const <String?>[
        'cubic',
        'new_reno',
        'bbr',
      ]);
      expect(
        ProfileCapabilities.allowedCores(ConfigType.tuic),
        const <CoreType>[CoreType.singBox],
      );
    },
  );

  testWidgets('P08 TUIC editor hides the collapsed transport grid', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newTuic(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('row-congestionControl')), findsOneWidget);
    expect(find.byKey(const ValueKey('row-network')), findsNothing);
  });
}
