import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';

import 'support/counting_runtime_bridge.dart';

ProviderContainer makeRuntimeContainer(CountingRuntimeBridge runtime) =>
    ProviderContainer(
      overrides: [runtimeBridgeProvider.overrideWithValue(runtime)],
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

  test('reload is a no-op while the runtime is busy', () async {
    final runtime = CountingRuntimeBridge(
      activeId: 'active-1',
      initial: const RuntimeView(state: 'Starting'),
    );
    final container = makeRuntimeContainer(runtime);
    addTearDown(container.dispose);
    final controller = container.read(runtimeControllerProvider.notifier);

    await controller.refresh();
    expect(container.read(runtimeControllerProvider).isBusy, isTrue);

    await controller.reload();
    expect(runtime.applyCalls, 0);
  });
}
