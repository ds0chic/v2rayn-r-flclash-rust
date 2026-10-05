// R4-18.P11 repro (NaiveProxy). Expected to FAIL pre-fix:
//  - upstream `AddServerWindow` binds `cmbCongestionControl12` ->
//    `CongestionControl` with `Global.NaiveCongestionControls`; the Naive editor
//    exposes no congestion-control field.
//  - upstream collapses `gridTransport` for Naive; the editor still renders
//    `row-network`.
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

ProfileDraft _newNaive() => ProfileDraft()
  ..configType = ConfigType.naive
  ..coreType = CoreType.singBox
  ..remarks = 'naive'
  ..address = '192.0.2.11'
  ..port = 443
  ..username = 'naive-user'
  ..password = 'naive-pass';

void main() {
  test('P11 Naive exposes the upstream congestion-control field', () {
    final congestion = protocolFields(ConfigType.naive)
        .where((f) => f.key == 'congestionControl');
    expect(congestion, hasLength(1));
    expect(
      congestion.single.options!.map((o) => o.value).toList(),
      const <String?>['bbr', 'bbr2', 'cubic', 'reno'],
    );
  });

  testWidgets('P11 Naive editor shows congestion and hides transport', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newNaive(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('row-congestionControl')), findsOneWidget);
    expect(find.byKey(const ValueKey('row-network')), findsNothing);
  });
}
