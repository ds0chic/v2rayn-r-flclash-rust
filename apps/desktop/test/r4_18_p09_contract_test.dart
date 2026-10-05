// R4-18.P09 contract (WireGuard): private/public/preshared/reserved/address/
// MTU/DNS, MTU default, no TLS/transport grid, save -> reopen, cancel.
// Synthetic only; no native library, kernel or network.
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

ProfileDraft _newWg() => ProfileDraft()
  ..configType = ConfigType.wireGuard
  ..coreType = CoreType.singBox
  ..remarks = 'wg'
  ..address = '192.0.2.9'
  ..port = 51820
  ..password = 'AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8='
  ..wgPublicKey = 'BAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8='
  ..wgPresharedKey = 'CAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8='
  ..wgInterfaceAddress = '10.0.0.2/32'
  ..wgReserved = '1, 2, 3'
  ..wgDns = '1.1.1.1';

void main() {
  testWidgets('P09 save -> reopen keeps WireGuard keys and MTU default', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newWg(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    expect(saved.single.password, _newWg().password);
    expect(saved.single.protoExtra.wgPublicKey, _newWg().wgPublicKey);
    expect(saved.single.protoExtra.wgPresharedKey, _newWg().wgPresharedKey);
    expect(saved.single.protoExtra.wgInterfaceAddress, '10.0.0.2/32');
    expect(saved.single.protoExtra.wgReserved, '1, 2, 3');
    // Upstream defaults MTU to `Global.TunMtus.First()`.
    expect(saved.single.protoExtra.wgMtu, 1280);
    final reopened = ProfileDraft.fromDto(saved.single);
    expect(reopened.wgInterfaceAddress, '10.0.0.2/32');
    expect(reopened.wgMtu, 1280);
  });

  testWidgets('P09 missing private key is a visible error', (tester) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newWg()..password = '', saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.byKey(const ValueKey('editor-save')));
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(find.text('必填'), findsWidgets);
    expect(saved, isEmpty);
  });

  testWidgets('P09 cancel never saves', (tester) async {
    final saved = <c.ProfileDto>[];
    await tester.pumpWidget(_host(_newWg(), saved));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('editor-cancel')));
    await tester.pumpAndSettle();
    expect(saved, isEmpty);
  });

  test('P09 WireGuard has no TLS and no transport', () {
    expect(ProfileCapabilities.supportsTls(ConfigType.wireGuard), isFalse);
    expect(ProfileCapabilities.supportsReality(ConfigType.wireGuard), isFalse);
    expect(
      ProfileCapabilities.supportsTransport(ConfigType.wireGuard),
      isFalse,
    );
  });
}
