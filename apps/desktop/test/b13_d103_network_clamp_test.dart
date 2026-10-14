// B13 D1-03: opening the editor must not rewrite a persisted network that
// sits outside the per-protocol candidate subset (upstream only resets a
// network missing from the global Networks list on save).
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';
import 'package:v2rayn_desktop/features/profiles/profile_editor_dialog.dart';

void main() {
  testWidgets('editor keeps a persisted out-of-subset network on save', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    final draft = ProfileDraft()
      ..configType = ConfigType.shadowsocks
      ..coreType = CoreType.xray
      ..remarks = 'ss'
      ..address = '192.0.2.20'
      ..port = 8388
      ..network = 'kcp'
      ..password = 'pass'
      ..ssMethod = 'aes-256-gcm';

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

    // No edits at all: a remarks-only save must stay lossless.
    await tester.tap(find.byKey(const ValueKey('editor-save')));
    await tester.pumpAndSettle();

    expect(saved, hasLength(1));
    expect(
      saved.single.network,
      'kcp',
      reason: 'opening the editor must not clamp a persisted network',
    );
    expect(find.byKey(const ValueKey('profile-editor')), findsNothing);
  });
}
