// T06a: save forwards the full draft through the bridge callback.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';
import 'package:v2rayn_desktop/features/profiles/profile_editor_dialog.dart';

void main() {
  testWidgets('editor save forwards the edited draft', (tester) async {
    final saved = <c.ProfileDto>[];
    final draft = ProfileDraft()
      ..configType = ConfigType.trojan
      ..coreType = CoreType.xray
      ..remarks = 'trojan'
      ..address = '192.0.2.20'
      ..port = 8443
      ..network = 'ws'
      ..path = '/ws'
      ..streamSecurity = 'tls';

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Builder(
            builder: (context) => Center(
              child: ElevatedButton(
                onPressed: () async => showProfileEditor(
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

    await tester.enterText(
      find.byKey(const ValueKey('field-password')),
      'trojan-pass',
    );
    await tester.enterText(
      find.byKey(const ValueKey('field-remarks')),
      'Trojan 节点 🚀',
    );
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();

    expect(saved, hasLength(1));
    expect(saved.single.remarks, 'Trojan 节点 🚀');
    expect(saved.single.port, 8443);
    expect(saved.single.network, 'ws');
    expect(saved.single.transportExtra.path, '/ws');
    // Success closes the dialog.
    expect(find.byKey(const ValueKey('profile-editor')), findsNothing);
  });
}
