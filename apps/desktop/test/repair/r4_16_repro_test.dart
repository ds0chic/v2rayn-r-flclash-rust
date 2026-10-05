// R4-16 repro: a group paste/scan import is committed by the Rust batch path
// AND then re-saved per row on the Dart side (UF-PROF-08: "一次批提交 + N 次
// 再保存"). This file intentionally asserts the fixed contract; it must fail
// against the pre-fix implementation.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/subs/subs_actions.dart';

/// Records how a manual batch import is persisted. `importPersistCalls` models
/// the real Rust `import_from_text` group transaction; `saveImportedCalls`
/// models the FIX-04 per-row UI save.
class CountingImportBridge extends SyntheticBridgePort {
  final List<String?> importSubids = <String?>[];
  final List<bool> importDedup = <bool>[];
  int importPersistCalls = 0;
  int saveImportedCalls = 0;

  @override
  Future<c.ImportResult> importFromText(
    String text, {
    String? subid,
    bool deduplicate = true,
  }) async {
    importSubids.add(subid);
    importDedup.add(deduplicate);
    final result = await super.importFromText(
      text,
      subid: subid,
      deduplicate: deduplicate,
    );
    if (subid != null && subid.isNotEmpty && result.ok) importPersistCalls++;
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
    await tester.pumpAndSettle();

    // Exactly one batch transaction commits the group import.
    expect(
      bridge.importPersistCalls,
      1,
      reason: 'the group import must be one Rust batch commit',
    );
    // The Dart side must not add a second per-row write on top of it.
    expect(
      bridge.saveImportedCalls,
      0,
      reason: 'no Rust batch + Dart per-row double write',
    );
    // Manual batch import keeps duplicates: upstream `AddBatchServersCommon`
    // only applies `Distinct()` when `isSub`.
    expect(bridge.importDedup, everyElement(isFalse));
  });
}
