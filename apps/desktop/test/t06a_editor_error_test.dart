// T06a: a rejected save keeps the dialog, the input and the field error.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';
import 'package:v2rayn_desktop/features/profiles/profile_editor_dialog.dart';

void main() {
  testWidgets('rejected save preserves input and shows field error', (
    tester,
  ) async {
    var calls = 0;
    final draft = ProfileDraft()
      ..configType = ConfigType.trojan
      ..coreType = CoreType.xray
      ..remarks = 'trojan'
      ..address = '192.0.2.30'
      ..port = 8388
      ..network = 'raw'
      ..password = 'trojan-pass';

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
                    calls += 1;
                    return const c.SaveProfileResult(
                      ok: false,
                      error: c.ErrorDto(
                        code: 'E_FIELD_REQUIRED',
                        messageKey: 'error.remarks_required',
                        fieldPath: 'remarks',
                        retryable: false,
                      ),
                    );
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
      find.byKey(const ValueKey('field-remarks')),
      'bad name',
    );
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();

    expect(calls, 1);
    // Dialog still open, input preserved, both error surfaces shown.
    expect(find.byKey(const ValueKey('profile-editor')), findsOneWidget);
    expect(find.byKey(const ValueKey('editor-error')), findsOneWidget);
    expect(find.text('error.remarks_required'), findsWidgets);
    final field = tester.widget<TextFormField>(
      find.byKey(const ValueKey('field-remarks')),
    );
    expect(field.initialValue, 'bad name');
  });
}
