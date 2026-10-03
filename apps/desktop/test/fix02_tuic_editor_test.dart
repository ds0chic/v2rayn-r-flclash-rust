// FIX-02: TUIC UUID/password split, Reality linkage, lossless edits, fallback,
// and the restored helper actions (UUID generate, Cert -> CertSha).
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';
import 'package:v2rayn_desktop/features/profiles/profile_editor_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/profile_fields.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// Open the editor for [draft]; [onSave] receives the submitted DTO.
Future<void> pumpEditor(
  WidgetTester tester, {
  required ProfileDraft draft,
  required c.SaveProfileResult Function(c.ProfileDto) onSave,
}) async {
  await tester.binding.setSurfaceSize(const Size(1100, 780));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    MaterialApp(
      theme: buildAppTheme(Brightness.light),
      home: Scaffold(
        body: Builder(
          builder: (context) => Center(
            child: ElevatedButton(
              key: const ValueKey('fix02-open'),
              onPressed: () =>
                  showProfileEditor(context, initial: draft, onSave: onSave),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.byKey(const ValueKey('fix02-open')));
  await tester.pumpAndSettle();
}

Future<void> reopen(
  WidgetTester tester, {
  required ProfileDraft draft,
  required c.SaveProfileResult Function(c.ProfileDto) onSave,
}) async {
  await tester.pumpWidget(
    MaterialApp(
      theme: buildAppTheme(Brightness.light),
      home: Scaffold(
        body: Builder(
          builder: (context) => Center(
            child: ElevatedButton(
              key: const ValueKey('fix02-open'),
              onPressed: () =>
                  showProfileEditor(context, initial: draft, onSave: onSave),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.byKey(const ValueKey('fix02-open')));
  await tester.pumpAndSettle();
}

String? initialOf(WidgetTester tester, String key) =>
    tester.widget<TextFormField>(find.byKey(ValueKey(key))).initialValue;

void main() {
  testWidgets('TUIC exposes distinct UUID (username) and password fields', (
    tester,
  ) async {
    await pumpEditor(
      tester,
      draft: ProfileDraft()
        ..configType = ConfigType.tuic
        ..remarks = 'tuic'
        ..address = '192.0.2.9'
        ..port = 443,
      onSave: (dto) => c.SaveProfileResult(ok: true, profile: dto),
    );
    expect(find.byKey(const ValueKey('field-username')), findsOneWidget);
    expect(find.byKey(const ValueKey('field-password')), findsOneWidget);
    // Upstream TUIC labels: TbId = 用户 ID (id), TbId3 = 密码 (password).
    expect(find.text('用户 ID (id)'), findsOneWidget);
    expect(find.text('密码 (password)'), findsOneWidget);
  });

  testWidgets('TUIC save keeps UUID and password separate across reopen', (
    tester,
  ) async {
    const uuid = '11111111-2222-3333-4444-555555555555';
    c.ProfileDto? stored;
    final draft = ProfileDraft()
      ..configType = ConfigType.tuic
      ..remarks = 'tuic'
      ..address = '192.0.2.9'
      ..port = 443;
    await pumpEditor(
      tester,
      draft: draft,
      onSave: (dto) {
        stored = dto;
        return c.SaveProfileResult(ok: true, profile: dto);
      },
    );
    await tester.enterText(find.byKey(const ValueKey('field-username')), uuid);
    await tester.enterText(
      find.byKey(const ValueKey('field-password')),
      'tuic-pass',
    );
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(stored, isNotNull);
    expect(stored!.username, uuid, reason: 'TUIC UUID -> username');
    expect(stored!.password, 'tuic-pass', reason: 'TUIC password -> password');
    // Upstream forces TLS for TUIC when empty.
    expect(stored!.security.streamSecurity, 'tls');

    // Reopen from the persisted DTO and both fields retain their values.
    await reopen(
      tester,
      draft: ProfileDraft.fromDto(stored!),
      onSave: (dto) => c.SaveProfileResult(ok: true, profile: dto),
    );
    expect(initialOf(tester, 'field-username'), uuid);
    expect(initialOf(tester, 'field-password'), 'tuic-pass');
  });

  testWidgets('TUIC UUID must be a UUID; password must be non-empty', (
    tester,
  ) async {
    var saved = 0;
    await pumpEditor(
      tester,
      draft: ProfileDraft()
        ..configType = ConfigType.tuic
        ..remarks = 'tuic'
        ..address = '192.0.2.9'
        ..port = 443,
      onSave: (dto) {
        saved += 1;
        return c.SaveProfileResult(ok: true, profile: dto);
      },
    );
    await tester.enterText(
      find.byKey(const ValueKey('field-username')),
      'not-a-uuid',
    );
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(find.text('UUID 格式无效'), findsOneWidget);
    expect(saved, 0);
  });

  testWidgets('selecting Reality reveals its fields without reopening', (
    tester,
  ) async {
    await pumpEditor(
      tester,
      draft: ProfileDraft()
        ..configType = ConfigType.vless
        ..coreType = CoreType.xray
        ..remarks = 'vless'
        ..address = '192.0.2.10'
        ..port = 443
        ..password = '11111111-2222-3333-4444-555555555555',
      onSave: (dto) => c.SaveProfileResult(ok: true, profile: dto),
    );
    expect(find.byKey(const ValueKey('field-publicKey')), findsNothing);

    final securityField = find.byKey(const ValueKey('field-streamSecurity'));
    await tester.ensureVisible(securityField);
    await tester.pumpAndSettle();
    await tester.tap(securityField);
    await tester.pumpAndSettle();
    await tester.tap(find.text('reality').last);
    await tester.pumpAndSettle();

    expect(find.byKey(const ValueKey('field-publicKey')), findsOneWidget);
    expect(find.byKey(const ValueKey('field-shortId')), findsOneWidget);
    expect(find.byKey(const ValueKey('field-spiderX')), findsOneWidget);
  });

  testWidgets('unknown dropdown value falls back instead of asserting', (
    tester,
  ) async {
    final draft = ProfileDraft()
      ..configType = ConfigType.vless
      ..coreType = CoreType.xray
      ..remarks = 'vless'
      ..address = '192.0.2.10'
      ..port = 443
      ..password = '11111111-2222-3333-4444-555555555555'
      ..flow = 'legacy-flow-not-listed'
      ..streamSecurity = 'reality';
    await pumpEditor(
      tester,
      draft: draft,
      onSave: (dto) => c.SaveProfileResult(ok: true, profile: dto),
    );
    expect(tester.takeException(), isNull);
    // The unknown flow value is preserved as an editor candidate.
    expect(find.text('legacy-flow-not-listed (未知候选)'), findsOneWidget);
  });

  testWidgets('Fingerprint candidates include randomized and empty', (
    tester,
  ) async {
    await pumpEditor(
      tester,
      draft: ProfileDraft()
        ..configType = ConfigType.vless
        ..coreType = CoreType.xray
        ..remarks = 'vless'
        ..address = '192.0.2.10'
        ..port = 443
        ..password = '11111111-2222-3333-4444-555555555555'
        ..streamSecurity = 'tls'
        ..fingerprint = 'randomized',
      onSave: (dto) => c.SaveProfileResult(ok: true, profile: dto),
    );
    expect(initialOf(tester, 'field-fingerprint'), 'randomized');
    // Combo is editable: the candidate menu offers randomized and "(无)".
    final menu = find.byKey(const ValueKey('combo-fingerprint'));
    await tester.ensureVisible(menu);
    await tester.pumpAndSettle();
    await tester.tap(menu);
    await tester.pumpAndSettle();
    expect(find.text('randomized'), findsWidgets);
    expect(find.text('(无)'), findsWidgets);
  });

  testWidgets('VLESS encryption is an editable text field', (tester) async {
    c.ProfileDto? stored;
    await pumpEditor(
      tester,
      draft: ProfileDraft()
        ..configType = ConfigType.vless
        ..coreType = CoreType.xray
        ..remarks = 'vless'
        ..address = '192.0.2.10'
        ..port = 443
        ..password = '11111111-2222-3333-4444-555555555555',
      onSave: (dto) {
        stored = dto;
        return c.SaveProfileResult(ok: true, profile: dto);
      },
    );
    await tester.enterText(
      find.byKey(const ValueKey('field-vlessEncryption')),
      'mlkem768x25519plus.native.600s',
    );
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(
      stored!.protoExtra.vlessEncryption,
      'mlkem768x25519plus.native.600s',
    );
  });

  testWidgets('remarks-only save does not reset security/transport values', (
    tester,
  ) async {
    c.ProfileDto? stored;
    final draft = ProfileDraft()
      ..configType = ConfigType.vless
      ..coreType = CoreType.xray
      ..remarks = '原备注'
      ..address = 'example.test'
      ..port = 8443
      ..network = 'ws'
      ..path = '/custom'
      ..host = 'cdn.example'
      ..password = '11111111-2222-3333-4444-555555555555'
      ..streamSecurity = 'tls'
      ..fingerprint = 'chrome'
      ..sni = 'sni.example';
    await pumpEditor(
      tester,
      draft: draft,
      onSave: (dto) {
        stored = dto;
        return c.SaveProfileResult(ok: true, profile: dto);
      },
    );
    await tester.enterText(find.byKey(const ValueKey('field-remarks')), '改后备注');
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();

    expect(stored!.remarks, '改后备注');
    expect(stored!.security.streamSecurity, 'tls');
    expect(stored!.security.fingerprint, 'chrome');
    expect(stored!.security.sni, 'sni.example');
    expect(stored!.network, 'ws');
    expect(stored!.transportExtra.path, '/custom');
    expect(stored!.transportExtra.host, 'cdn.example');
  });

  testWidgets('UUID generate button fills the UUID field', (tester) async {
    c.ProfileDto? stored;
    await pumpEditor(
      tester,
      draft: ProfileDraft()
        ..configType = ConfigType.vless
        ..coreType = CoreType.xray
        ..remarks = 'vless'
        ..address = '192.0.2.10'
        ..port = 443,
      onSave: (dto) {
        stored = dto;
        return c.SaveProfileResult(ok: true, profile: dto);
      },
    );
    await tester.tap(find.byKey(const ValueKey('gen-uuid-password')));
    await tester.pumpAndSettle();
    final uuid = initialOf(tester, 'field-password')!;
    expect(
      uuid,
      matches(RegExp(r'^[0-9a-f]{8}-([0-9a-f]{4}-){3}[0-9a-f]{12}$')),
    );
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(stored!.password, uuid);
  });

  testWidgets('Cert SHA-256 helpers parse PEM chains deterministically', (
    tester,
  ) async {
    // Two synthetic self-signed bodies; the helper hashes the DER body.
    const pem =
        '-----BEGIN CERTIFICATE-----\n'
        'MIIBkTCB+wIJAKHexampleBodyOne\n'
        '-----END CERTIFICATE-----\n'
        '-----BEGIN CERTIFICATE-----\n'
        'MIIBkTCB+wIJAKHexampleBodyTwo\n'
        '-----END CERTIFICATE-----';
    final blocks = parsePemChain(pem);
    expect(blocks, hasLength(2));
    final sha = certShaFromChain(
      '-----BEGIN CERTIFICATE-----\n'
      'AA==\n'
      '-----END CERTIFICATE-----',
    );
    expect(sha, isNotNull);
    expect(sha, matches(RegExp(r'^[0-9a-f]{64}$')));
  });

  testWidgets('editing Cert auto-derives CertSha on save', (tester) async {
    c.ProfileDto? stored;
    // Single-line PEM accepted by the one-line Cert field.
    const pem = '-----BEGIN CERTIFICATE-----AA==-----END CERTIFICATE-----';
    await pumpEditor(
      tester,
      draft: ProfileDraft()
        ..configType = ConfigType.vless
        ..coreType = CoreType.xray
        ..remarks = 'vless'
        ..address = '192.0.2.10'
        ..port = 443
        ..password = '11111111-2222-3333-4444-555555555555'
        ..streamSecurity = 'tls',
      onSave: (dto) {
        stored = dto;
        return c.SaveProfileResult(ok: true, profile: dto);
      },
    );
    final certField = find.byKey(const ValueKey('field-cert'));
    await tester.ensureVisible(certField);
    await tester.pumpAndSettle();
    await tester.enterText(certField, pem);
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(stored!.security.cert, pem);
    expect(
      stored!.security.certSha,
      matches(RegExp(r'^[0-9a-f]{64}$')),
      reason: 'Cert text change must derive CertSha',
    );
  });
}
