// RE-PROF-07: Mux / Finalmask / applicable UOT controls.
//
// Field visibility is asserted against the same spec lists the editor renders
// from; save/cancel are exercised through the real ProfileEditorDialog.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';
import 'package:v2rayn_desktop/features/profiles/profile_editor_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/profile_fields.dart';

Set<String> _protocolKeys(ConfigType t) =>
    protocolFields(t).map((f) => f.key).toSet();

Set<String> _securityKeys(String? security, {bool finalmask = false}) =>
    securityFields(security, finalmask: finalmask).map((f) => f.key).toSet();

ProfileDraft _vmessDraft() => ProfileDraft()
  ..configType = ConfigType.vmess
  ..indexId = ''
  ..remarks = 'synthetic'
  ..address = '192.0.2.10'
  ..port = 443
  ..password = '11111111-2222-3333-4444-555555555555'
  ..muxEnabled = true
  ..finalmask = 'tcp finalmask';

ProfileDraft _tuicDraft() => ProfileDraft()
  ..configType = ConfigType.tuic
  ..indexId = ''
  ..remarks = 'synthetic'
  ..address = '192.0.2.10'
  ..port = 443
  ..username = 'user'
  ..password = 'pass';

Future<void> _openEditor(
  WidgetTester tester, {
  required ProfileDraft draft,
  required c.SaveProfileResult Function(c.ProfileDto) onSave,
}) async {
  await tester.pumpWidget(
    MaterialApp(
      home: Builder(
        builder: (context) => Scaffold(
          body: Center(
            child: ElevatedButton(
              onPressed: () =>
                  showProfileEditor(context, initial: draft, onSave: onSave),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.text('open'));
  await tester.pumpAndSettle();
}

void main() {
  test('mux applies to VMess/VLESS/Trojan/Shadowsocks only', () {
    for (final t in const <ConfigType>[
      ConfigType.vmess,
      ConfigType.vless,
      ConfigType.trojan,
      ConfigType.shadowsocks,
    ]) {
      expect(_protocolKeys(t), contains('muxEnabled'), reason: '$t mux');
    }
    for (final t in const <ConfigType>[
      ConfigType.tuic,
      ConfigType.hysteria2,
      ConfigType.anytls,
      ConfigType.naive,
      ConfigType.wireGuard,
      ConfigType.socks,
      ConfigType.http,
    ]) {
      expect(_protocolKeys(t), isNot(contains('muxEnabled')), reason: '$t mux');
    }
  });

  test('uot applies to Shadowsocks and Naive only', () {
    expect(_protocolKeys(ConfigType.shadowsocks), contains('uot'));
    expect(_protocolKeys(ConfigType.naive), contains('uot'));
    for (final t in const <ConfigType>[
      ConfigType.tuic,
      ConfigType.vmess,
      ConfigType.vless,
      ConfigType.trojan,
      ConfigType.hysteria2,
      ConfigType.anytls,
    ]) {
      expect(_protocolKeys(t), isNot(contains('uot')), reason: '$t uot');
    }
  });

  test('finalmask is hidden for TUIC/Anytls/Naive', () {
    for (final t in const <ConfigType>[ConfigType.vmess, ConfigType.vless]) {
      expect(ProfileCapabilities.supportsFinalmask(t), isTrue, reason: '$t');
      expect(_securityKeys('tls', finalmask: true), contains('finalmask'));
    }
    for (final t in const <ConfigType>[
      ConfigType.tuic,
      ConfigType.anytls,
      ConfigType.naive,
    ]) {
      expect(ProfileCapabilities.supportsFinalmask(t), isFalse, reason: '$t');
    }
    expect(_securityKeys('tls'), isNot(contains('finalmask')));
  });

  testWidgets('VMess editor shows mux/finalmask and hides uot', (tester) async {
    await _openEditor(
      tester,
      draft: _vmessDraft(),
      onSave: (dto) => c.SaveProfileResult(ok: true, profile: dto),
    );
    expect(find.byKey(const ValueKey('row-muxEnabled')), findsOneWidget);
    expect(find.byKey(const ValueKey('row-finalmask')), findsOneWidget);
    expect(find.byKey(const ValueKey('row-uot')), findsNothing);
  });

  testWidgets('TUIC editor hides uot/mux/finalmask', (tester) async {
    await _openEditor(
      tester,
      draft: _tuicDraft(),
      onSave: (dto) => c.SaveProfileResult(ok: true, profile: dto),
    );
    expect(find.byKey(const ValueKey('row-uot')), findsNothing);
    expect(find.byKey(const ValueKey('row-muxEnabled')), findsNothing);
    expect(find.byKey(const ValueKey('row-finalmask')), findsNothing);
    expect(find.byKey(const ValueKey('row-congestionControl')), findsOneWidget);
  });

  testWidgets(
    'save returns muxEnabled/finalmask unchanged (reopen roundtrip)',
    (tester) async {
      c.ProfileDto? captured;
      await _openEditor(
        tester,
        draft: _vmessDraft(),
        onSave: (dto) {
          captured = dto;
          return c.SaveProfileResult(ok: true, profile: dto);
        },
      );
      await tester.tap(find.byKey(const ValueKey('editor-save')));
      await tester.pumpAndSettle();

      expect(captured, isNotNull);
      expect(captured!.muxEnabled, isTrue);
      expect(captured!.finalmask, 'tcp finalmask');
      // Reopening the returned dto keeps the same draft shape.
      final reopened = ProfileDraft.fromDto(captured!);
      expect(reopened.muxEnabled, isTrue);
      expect(reopened.finalmask, 'tcp finalmask');
    },
  );

  testWidgets('cancel never calls onSave', (tester) async {
    var called = false;
    await _openEditor(
      tester,
      draft: _vmessDraft(),
      onSave: (dto) {
        called = true;
        return c.SaveProfileResult(ok: true, profile: dto);
      },
    );
    await tester.tap(find.byKey(const ValueKey('editor-cancel')));
    await tester.pumpAndSettle();

    expect(called, isFalse);
    expect(find.byKey(const ValueKey('profile-editor')), findsNothing);
  });
}
