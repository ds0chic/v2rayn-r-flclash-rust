import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/subs/subs_actions.dart';

/// Import bridge that records the `subid` each SP-14 phase was handed.
///
/// Preview is parse-only (always `null` group); the commit binds the group
/// snapshot taken when the command started.
class DelayedImportBridge extends SyntheticBridgePort {
  final List<String?> receivedPreviewSubids = <String?>[];
  final List<String?> receivedSubids = <String?>[];

  @override
  c.ImportResult previewImportText(String text, {String? subid}) {
    receivedPreviewSubids.add(subid);
    return super.previewImportText(text, subid: subid);
  }

  @override
  c.ImportResult commitImportText(
    List<c.ProfileDto> profiles, {
    String? subid,
  }) {
    receivedSubids.add(subid);
    return super.commitImportText(profiles, subid: subid);
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

void main() {
  testWidgets('paste/scan import snapshots the group before the async parse', (
    tester,
  ) async {
    final bridge = DelayedImportBridge();
    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(bridge),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(20),
      ],
    );
    addTearDown(container.dispose);

    final controller = container.read(profilesControllerProvider.notifier);
    controller.setGroupSubId('sub-A');

    late Future<void> pending;
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: _Probe(
            onPressed: (context, ref) {
              pending = importShareText(
                context,
                ref,
                'vless://11111111-2222-3333-4444-555555555555'
                '@node.example.invalid:11980#synthetic',
                sourceLabel: '粘贴',
              );
            },
          ),
        ),
      ),
    );

    await tester.tap(find.text('go'));
    await tester.pump();

    // The user switches group while the preview sheet is still open.
    controller.setGroupSubId('sub-B');
    expect(find.byKey(const ValueKey('import-preview-dialog')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('import-preview-commit')));
    await tester.pumpAndSettle();
    await pending;

    // Pure preview (no group) then the single commit, both bound to the
    // snapshot taken when the command started.
    expect(bridge.receivedPreviewSubids, <String?>[null]);
    expect(bridge.receivedSubids, <String?>['sub-A']);
    final saved = bridge.queryAllProfiles().firstWhere(
      (p) => p.remarks == 'synthetic',
    );
    expect(saved.subid, 'sub-A');
  });
}
