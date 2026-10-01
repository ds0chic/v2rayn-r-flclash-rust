// T06a: cancel never persists the draft.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';
import 'package:v2rayn_desktop/features/profiles/profile_editor_dialog.dart';

void main() {
  testWidgets('editor cancel does not invoke save', (tester) async {
    var saveCalls = 0;
    final draft = ProfileDraft()
      ..configType = ConfigType.vless
      ..coreType = CoreType.xray
      ..remarks = 'original'
      ..address = '192.0.2.9'
      ..port = 443
      ..network = 'ws';

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
                    saveCalls += 1;
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
    expect(find.byKey(const ValueKey('profile-editor')), findsOneWidget);

    await tester.enterText(
      find.byKey(const ValueKey('field-remarks')),
      'changed 中文 🙃',
    );
    await tester.tap(find.byKey(const ValueKey('editor-cancel')));
    await tester.pumpAndSettle();

    expect(saveCalls, 0);
    expect(find.byKey(const ValueKey('profile-editor')), findsNothing);
  });
}
