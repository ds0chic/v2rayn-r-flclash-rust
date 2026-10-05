// R4-01 contract: startup retry, honest pending state and trustworthy run facts.
//
// Synthetic only: no native library, no network, no port use and no user data.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profile_actions.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';

import 'support/counting_runtime_bridge.dart';

ProviderContainer runtimeContainer(CountingRuntimeBridge runtime) {
  final container = ProviderContainer(
    overrides: [
      runtimeBridgeProvider.overrideWithValue(runtime),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  test(
    'an explicit target apply is submitted with that frozen target',
    () async {
      final runtime = CountingRuntimeBridge(activeId: 'node-a');
      final container = runtimeContainer(runtime);
      final controller = container.read(runtimeControllerProvider.notifier);
      await controller.applyActive(targetId: 'node-b');
      expect(runtime.applyCalls, 1);
      expect(runtime.lastTargetId, 'node-b');
      expect(container.read(runtimeControllerProvider).isRunning, isTrue);
    },
  );

  test(
    'a failed apply never fabricates Running and keeps the old session',
    () async {
      final runtime = CountingRuntimeBridge(
        initial: const RuntimeView(
          state: 'Running',
          hostAlive: true,
          ports: <int>[11810],
          sessionId: 's-old',
        ),
        applyError: const RuntimeErrorView(
          code: 'E_CORE_NOT_FOUND',
          messageKey: 'error.core_not_found',
        ),
      );
      final container = runtimeContainer(runtime);
      final controller = container.read(runtimeControllerProvider.notifier);
      await controller.applyActive(targetId: 'node-b');
      final view = container.read(runtimeControllerProvider);
      expect(view.error?.code, 'E_CORE_NOT_FOUND');
      expect(
        view.isRunning,
        isTrue,
        reason: 'the old session is still running',
      );
      expect(view.sessionId, 's-old');
      expect(view.ports, <int>[11810]);
      expect(view.commandPending, isFalse);
    },
  );

  test(
    'a repaired failure can be retried and only reports running on success',
    () async {
      final runtime = CountingRuntimeBridge(
        activeId: 'node-a',
        applyError: const RuntimeErrorView(
          code: 'E_CORE_NOT_FOUND',
          messageKey: 'error.core_not_found',
        ),
      );
      final container = runtimeContainer(runtime);
      final controller = container.read(runtimeControllerProvider.notifier);
      await controller.applyActive();
      expect(container.read(runtimeControllerProvider).isRunning, isFalse);
      runtime.applyError = null;
      await controller.applyActive();
      expect(runtime.applyCalls, 2);
      expect(container.read(runtimeControllerProvider).isRunning, isTrue);
    },
  );

  testWidgets(
    'a same-active default activation is an idempotent no-op that never '
    'reports a run',
    (tester) async {
      late WidgetRef captured;
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            bridgePortProvider.overrideWithValue(
              SyntheticBridgePort(count: 10),
            ),
            uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
            runtimeBridgeProvider.overrideWithValue(
              CountingRuntimeBridge(activeId: 'syn-000000'),
            ),
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
      final outcome = await activateProfileDetailed(captured, 'syn-000000');
      expect(outcome.persisted, isTrue);
      expect(outcome.noop, isTrue);
      expect(
        outcome.applied,
        isFalse,
        reason: 'an already-active node must not be reported as applied',
      );
      expect(outcome.fullyOk, isFalse);
    },
  );
}
