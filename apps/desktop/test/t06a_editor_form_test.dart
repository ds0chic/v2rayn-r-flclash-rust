// T06a: editor renders per-protocol fields and validates near the field.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';
import 'package:v2rayn_desktop/features/profiles/profile_editor_dialog.dart';

void main() {
  testWidgets('11-protocol editor renders protocol fields and validates', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    final draft = ProfileDraft()
      ..configType = ConfigType.vmess
      ..coreType = CoreType.xray
      ..remarks = '节点'
      ..address = '192.0.2.1'
      ..port = 443
      ..network = 'raw'
      ..streamSecurity = 'reality'
      ..publicKey = 'pub';

    await tester.pumpWidget(
      MaterialApp(
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
      ),
    );
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();

    // VMess protocol fields + Reality security fields.
    expect(find.byKey(const ValueKey('field-password')), findsOneWidget);
    expect(find.byKey(const ValueKey('field-alterId')), findsOneWidget);
    expect(find.byKey(const ValueKey('field-vmessSecurity')), findsOneWidget);
    expect(find.byKey(const ValueKey('field-publicKey')), findsOneWidget);
    expect(find.byKey(const ValueKey('field-shortId')), findsOneWidget);

    // Switch to VLESS.
    await tester.tap(find.byKey(const ValueKey('field-configType')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('vless').last);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('field-flow')), findsOneWidget);
    expect(find.byKey(const ValueKey('field-vlessEncryption')), findsOneWidget);

    // Switch to WireGuard; the core is constrained to sing-box.
    await tester.tap(find.byKey(const ValueKey('field-configType')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('wireGuard').last);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('field-wgPublicKey')), findsOneWidget);
    expect(
      find.byKey(const ValueKey('field-wgInterfaceAddress')),
      findsOneWidget,
    );

    // Validation: required remarks + port range, without saving.
    await tester.enterText(find.byKey(const ValueKey('field-remarks')), '');
    await tester.enterText(find.byKey(const ValueKey('field-port')), '99999');
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();
    expect(find.text('必填'), findsWidgets);
    expect(find.text('端口需在 1-65535'), findsOneWidget);
    expect(saved, isEmpty);
  });
}
