// Copy into apps/desktop/test/_user_flow_audit_repro_test.dart to run.
// These assertions express the repair contract and intentionally fail on
// 77c74ed. Synthetic failure injection; no native library or network access.
import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';

import 'support/counting_runtime_bridge.dart';

ProviderContainer auditContainer(CountingRuntimeBridge runtime) =>
    ProviderContainer(
      overrides: [
        runtimeBridgeProvider.overrideWithValue(runtime),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      ],
    );

void main() {
  test('explicit apply retries after the previous core error is repaired', () async {
    final runtime = CountingRuntimeBridge(
      activeId: 'synthetic-node',
      applyError: const RuntimeErrorView(
        code: 'E_CORE_NOT_FOUND',
        messageKey: 'error.core_not_found',
      ),
    );
    final container = auditContainer(runtime);
    addTearDown(container.dispose);
    final controller = container.read(runtimeControllerProvider.notifier);
    await controller.applyActive();
    expect(runtime.applyCalls, 1);
    runtime.applyError = null;
    await controller.applyActive();
    expect(runtime.applyCalls, 2,
        reason: 'A historical snapshot error must not veto a new user command.');
    expect(container.read(runtimeControllerProvider).isRunning, isTrue);
  });

  test('F5 retries a repaired failure rather than re-reading and vetoing it', () async {
    final runtime = CountingRuntimeBridge(
      activeId: 'synthetic-node',
      applyError: const RuntimeErrorView(
        code: 'E_CORE_NOT_FOUND',
        messageKey: 'error.core_not_found',
      ),
    );
    final container = auditContainer(runtime);
    addTearDown(container.dispose);
    final controller = container.read(runtimeControllerProvider.notifier);
    await controller.applyActive();
    runtime.applyError = null;
    await controller.reload();
    expect(runtime.applyCalls, 2,
        reason: 'Reload must submit the new attempt after repairing the cause.');
  });

  test('a pending command reports busy before the host sends a transition', () async {
    final runtime = CountingRuntimeBridge(activeId: 'synthetic-node');
    final gate = Completer<void>();
    runtime.applyGate = gate;
    final container = auditContainer(runtime);
    addTearDown(container.dispose);
    final controller = container.read(runtimeControllerProvider.notifier);
    final pending = controller.applyActive();
    await Future<void>.delayed(Duration.zero);
    final busyWhileWaiting = container.read(runtimeControllerProvider).isBusy;
    gate.complete();
    await pending;
    expect(busyWhileWaiting, isTrue,
        reason: 'Click feedback and duplicate-command gating cannot wait for IPC.');
  });
}
