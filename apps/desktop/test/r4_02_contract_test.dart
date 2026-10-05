// R4-02 contract: explicit frozen start target, plain-select semantics and the
// default-node contract (F5/recovery).
//
// Synthetic only: no native library, no network, no port use and no user data.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profile_actions.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';

import 'support/counting_runtime_bridge.dart';

/// Synthetic bridge whose `setActiveProfile` can be forced to fail, so a
/// persist failure must not leave a fake in-memory active node.
class _FailSetActiveBridge extends SyntheticBridgePort {
  _FailSetActiveBridge({super.count});

  bool fail = false;

  @override
  c.SimpleResult setActiveProfile(String? id) {
    if (fail) {
      return const c.SimpleResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_STORAGE',
          messageKey: 'error.storage',
          retryable: true,
        ),
      );
    }
    return super.setActiveProfile(id);
  }
}

({
  ProviderContainer container,
  SyntheticBridgePort bridge,
  CountingRuntimeBridge runtime,
})
makeR4Container({int rows = 10, SyntheticBridgePort? bridge}) {
  final b = bridge ?? SyntheticBridgePort(count: rows);
  final runtime = CountingRuntimeBridge(activeId: 'syn-000000');
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(b),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      runtimeBridgeProvider.overrideWithValue(runtime),
      profileRowCountProvider.overrideWithValue(rows),
    ],
  );
  addTearDown(container.dispose);
  return (container: container, bridge: b, runtime: runtime);
}

void main() {
  test('plain single click only selects and never activates or applies', () {
    final made = makeR4Container();
    final profiles = made.container.read(profilesControllerProvider.notifier);
    profiles.setActive('syn-000001');
    profiles.selectRow('syn-000002');
    final state = made.container.read(profilesControllerProvider);
    expect(state.primaryId, 'syn-000002');
    expect(state.activeId, 'syn-000001', reason: 'selection is not activation');
    expect(made.runtime.applyCalls, 0);
  });

  test('no active + B selected freezes B and persists it as default', () {
    final made = makeR4Container();
    final profiles = made.container.read(profilesControllerProvider.notifier);
    profiles.clearSelection();
    profiles.selectRow('syn-000002');
    final prep = profiles.prepareStartTarget(frozenTargetId: null);
    expect(prep.target, 'syn-000002');
    expect(prep.persisted, isTrue);
    expect(prep.changed, isTrue);
    expect(
      made.container.read(profilesControllerProvider).activeId,
      'syn-000002',
    );
  });

  test('A active + B selected starts B, not A', () {
    final made = makeR4Container();
    final profiles = made.container.read(profilesControllerProvider.notifier);
    profiles.setActive('syn-000000');
    profiles.selectRow('syn-000003');
    final prep = profiles.prepareStartTarget(frozenTargetId: null);
    expect(prep.target, 'syn-000003');
    expect(
      made.container.read(profilesControllerProvider).activeId,
      'syn-000003',
    );
  });

  test('multi-selection uses the frozen primary row', () {
    final made = makeR4Container();
    final profiles = made.container.read(profilesControllerProvider.notifier);
    profiles.setActive('syn-000000');
    profiles.selectRow('syn-000001');
    profiles.selectRow('syn-000004', ctrl: true);
    final state = made.container.read(profilesControllerProvider);
    expect(state.primaryId, 'syn-000004');
    final prep = profiles.prepareStartTarget(frozenTargetId: null);
    expect(prep.target, 'syn-000004');
  });

  test('an explicit frozen target wins over the live primary row', () {
    final made = makeR4Container();
    final profiles = made.container.read(profilesControllerProvider.notifier);
    profiles.selectRow('syn-000002');
    final prep = profiles.prepareStartTarget(frozenTargetId: 'syn-000005');
    expect(prep.target, 'syn-000005');
  });

  test('no selection falls back to the persisted default', () {
    final made = makeR4Container();
    final profiles = made.container.read(profilesControllerProvider.notifier);
    profiles.setActive('syn-000000');
    profiles.clearSelection();
    final prep = profiles.prepareStartTarget(frozenTargetId: null);
    expect(prep.target, 'syn-000000');
    expect(prep.changed, isFalse);
  });

  test(
    'no active and no selection recovers the upstream default candidate',
    () {
      final made = makeR4Container();
      final profiles = made.container.read(profilesControllerProvider.notifier);
      profiles.clearSelection();
      final prep = profiles.prepareStartTarget(frozenTargetId: null);
      expect(
        prep.target,
        'syn-000000',
        reason: 'first Port>0 row of the current list',
      );
      expect(
        made.container.read(profilesControllerProvider).activeId,
        'syn-000000',
      );
    },
  );

  test('no profiles reports no target instead of inventing one', () {
    final made = makeR4Container(rows: 0);
    final profiles = made.container.read(profilesControllerProvider.notifier);
    final prep = profiles.prepareStartTarget(frozenTargetId: null);
    expect(prep.target, isNull);
    expect(prep.persisted, isFalse);
  });

  test('a persist failure leaves no fake in-memory active', () {
    final bridge = _FailSetActiveBridge(count: 10);
    final made = makeR4Container(bridge: bridge);
    final profiles = made.container.read(profilesControllerProvider.notifier);
    profiles.setActive('syn-000000');
    bridge.fail = true;
    final prep = profiles.prepareStartTarget(frozenTargetId: 'syn-000001');
    expect(prep.persisted, isFalse);
    expect(prep.errorCode, 'E_STORAGE');
    expect(
      made.container.read(profilesControllerProvider).activeId,
      'syn-000000',
      reason: 'a rejected persist must not advance the in-memory default',
    );
  });

  test('F5 reload still uses the default node, not the selected row', () async {
    final made = makeR4Container();
    final profiles = made.container.read(profilesControllerProvider.notifier);
    profiles.setActive('syn-000000');
    profiles.selectRow('syn-000007');
    await made.container.read(runtimeControllerProvider.notifier).reload();
    expect(made.runtime.applyCalls, 1);
    expect(
      made.runtime.lastTargetId,
      isNull,
      reason: 'reload submits the default (empty-target) command',
    );
    expect(
      made.container.read(profilesControllerProvider).activeId,
      'syn-000000',
      reason: 'F5 does not switch the default from the mouse highlight',
    );
  });

  testWidgets('the top start command applies exactly the frozen target', (
    tester,
  ) async {
    late WidgetRef captured;
    final runtime = CountingRuntimeBridge(activeId: 'syn-000000');
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          bridgePortProvider.overrideWithValue(SyntheticBridgePort(count: 10)),
          uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
          runtimeBridgeProvider.overrideWithValue(runtime),
          profileRowCountProvider.overrideWithValue(10),
        ],
        child: Consumer(
          builder: (context, ref, child) {
            captured = ref;
            return const SizedBox.shrink();
          },
        ),
      ),
    );
    final profiles = captured.read(profilesControllerProvider.notifier);
    profiles.setActive('syn-000000');
    profiles.selectRow('syn-000004');

    final outcome = await startProfileExplicit(captured);
    expect(outcome.persisted, isTrue);
    expect(outcome.applied, isTrue);
    expect(runtime.lastTargetId, 'syn-000004');
    expect(captured.read(profilesControllerProvider).activeId, 'syn-000004');

    // A repeated default activation is an idempotent no-op that runs nothing.
    final again = await activateProfileDetailed(captured, 'syn-000004');
    expect(again.noop, isTrue);
    expect(again.applied, isFalse);
    expect(runtime.applyCalls, 1);
  });
}
