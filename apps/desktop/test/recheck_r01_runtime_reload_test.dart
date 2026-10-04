import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';

import 'support/counting_runtime_bridge.dart';

ProviderContainer makeRuntimeContainer(CountingRuntimeBridge runtime) =>
    ProviderContainer(
      overrides: [
        runtimeBridgeProvider.overrideWithValue(runtime),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      ],
    );

void main() {
  test(
    'reload refreshes the desired revision and applies the active plan',
    () async {
      final runtime = CountingRuntimeBridge(activeId: 'active-1');
      final container = makeRuntimeContainer(runtime);
      addTearDown(container.dispose);
      final controller = container.read(runtimeControllerProvider.notifier);

      await controller.reload();

      expect(runtime.applyCalls, 1);
      expect(runtime.snapshotCalls, greaterThanOrEqualTo(1));
      expect(container.read(runtimeControllerProvider).state, 'Running');
    },
  );

  test('reload skips apply when there is no persisted active node', () async {
    final runtime = CountingRuntimeBridge(activeId: null);
    final container = makeRuntimeContainer(runtime);
    addTearDown(container.dispose);
    final controller = container.read(runtimeControllerProvider.notifier);

    await controller.reload();

    expect(runtime.applyCalls, 0);
    expect(runtime.snapshotCalls, greaterThanOrEqualTo(1));
  });

  test('reload prompts when there is no active node', () async {
    // Upstream `Reload` enqueues `CheckServerSettings`; the empty-active path
    // must not be silent (R3-ROOT-01).
    final runtime = CountingRuntimeBridge(activeId: null);
    final container = makeRuntimeContainer(runtime);
    addTearDown(container.dispose);
    final controller = container.read(runtimeControllerProvider.notifier);

    await controller.reload();

    expect(runtime.applyCalls, 0);
    expect(container.read(uiShellControllerProvider).message, '配置项无效，请检查或重新选择');
  });

  test('reload re-entrancy runs one deferred job (no lost F5)', () async {
    // Upstream `_hasNextReloadJob`: a reload requested while one is running is
    // remembered and executed once when the in-flight one finishes
    // (R3-ROOT-01). It must not be silently dropped.
    final runtime = CountingRuntimeBridge(activeId: 'active-1');
    runtime.applyGate = Completer<void>();
    final container = makeRuntimeContainer(runtime);
    addTearDown(container.dispose);
    final controller = container.read(runtimeControllerProvider.notifier);

    final first = controller.reload();
    await Future<void>.delayed(const Duration(milliseconds: 10));
    expect(runtime.applyCalls, 1, reason: 'first reload is in flight');

    final second = controller.reload();
    await second;
    expect(runtime.applyCalls, 1, reason: 'second reload marks a pending job');

    runtime.applyGate!.complete();
    await first;

    expect(runtime.applyCalls, 2, reason: 'deferred reload runs exactly once');
    expect(container.read(runtimeControllerProvider).state, 'Running');
  });
}
