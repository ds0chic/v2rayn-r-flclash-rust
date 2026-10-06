// R4-16/SP-14 repro: a group paste/scan import is committed by exactly one
// batch transaction (SP-14 `commitImportText`) and never re-saved per row on
// the Dart side (UF-PROF-08: "一次批提交 + N 次再保存"). This file asserts the
// fixed contract; it fails against the pre-fix implementation.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/subs/subs_actions.dart';

/// Records how a manual batch import is persisted. `importPersistCalls` models
/// the single-transaction SP-14 commit; `saveImportedCalls` models the legacy
/// FIX-04 per-row UI save (must stay zero on the new path).
class CountingImportBridge extends SyntheticBridgePort {
  int importPersistCalls = 0;
  int saveImportedCalls = 0;

  @override
  c.ImportResult commitImportText(
    List<c.ProfileDto> profiles, {
    String? subid,
  }) {
    final result = super.commitImportText(profiles, subid: subid);
    if (result.ok) importPersistCalls++;
    return result;
  }

  @override
  c.SaveProfileResult saveImportedProfile(
    c.ProfileDto draft,
    int expectedRevision,
  ) {
    saveImportedCalls++;
    return super.saveImportedProfile(draft, expectedRevision);
  }
}

class _Probe extends ConsumerWidget {
  const _Probe({required this.onPressed});

  final void Function(BuildContext context, WidgetRef ref) onPressed;

  @override
  Widget build(BuildContext context, WidgetRef ref) => Material(
    child: TextButton(
      onPressed: () => onPressed(context, ref),
      child: const Text('go'),
    ),
  );
}

const _text =
    'vless://11111111-2222-3333-4444-555555555555'
    '@node.example.invalid:11980#repro\n'
    'trojan://pw@node2.example.invalid:11980#repro2';

void main() {
  testWidgets('group paste import is a single backend commit', (tester) async {
    final bridge = CountingImportBridge();
    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(bridge),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(20),
      ],
    );
    addTearDown(container.dispose);
    container.read(profilesControllerProvider.notifier).setGroupSubId('sub-A');

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: _Probe(
            onPressed: (context, ref) {
              importShareText(context, ref, _text, sourceLabel: '粘贴');
            },
          ),
        ),
      ),
    );
    await tester.tap(find.text('go'));
    await tester.pump();
    expect(find.byKey(const ValueKey('import-preview-dialog')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('import-preview-commit')));
    await tester.pumpAndSettle();

    // Exactly one batch transaction commits the group import.
    expect(
      bridge.importPersistCalls,
      1,
      reason: 'the group import must be one batch commit',
    );
    // The Dart side must not add a second per-row write on top of it.
    expect(
      bridge.saveImportedCalls,
      0,
      reason: 'no batch + Dart per-row double write',
    );
  });
}
