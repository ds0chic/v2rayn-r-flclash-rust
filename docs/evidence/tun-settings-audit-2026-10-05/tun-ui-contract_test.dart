import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/runtime/tun_toggle.dart';

class RejectedRuntime implements RuntimeBridge {
  const RejectedRuntime();

  @override
  String? activeProfileId() => 'synthetic-node';

  @override
  Future<RuntimeActionResult> applyActive({required BigInt expectedRevision}) async =>
      const RuntimeActionResult(
        ok: false,
        error: RuntimeErrorView(
          code: 'E_CORE_NOT_FOUND',
          messageKey: 'error.core_not_found',
        ),
      );

  @override
  Stream<RuntimeEvent> events() => const Stream.empty();

  @override
  Future<RuntimeView> snapshot() async => const RuntimeView(
    state: 'Running',
    hostAlive: true,
    ports: [11970],
    sessionId: 'synthetic-old-tun-session',
  );

  @override
  Future<RuntimeActionResult> stop() async =>
      const RuntimeActionResult(ok: true);
}

void main() {
  test('audit repro: real controller failure completes toggle as applied', () async {
    final container = ProviderContainer(overrides: [
      runtimeBridgeProvider.overrideWithValue(const RejectedRuntime()),
    ]);
    addTearDown(container.dispose);
    final runtime = container.read(runtimeControllerProvider.notifier);
    final result = await toggleTunDesired(
      enabled: false,
      persist: (_) => true,
      apply: runtime.applyActive,
    );
    final actual = container.read(runtimeControllerProvider);
    expect(actual.error?.code, 'E_CORE_NOT_FOUND');
    expect(actual.isRunning, isTrue);
    expect(actual.sessionId, 'synthetic-old-tun-session');
    expect(result.ok, isTrue);
    expect(result.runtimeApplied, isTrue);
    expect(tunActualLabel(false, actual), '未启用');
    // These assertions record the current defect; they are not release gates.
    print('REPRODUCED: error=${actual.error?.code}; old_session=${actual.sessionId}; '
        'toggle.ok=${result.ok}; runtimeApplied=${result.runtimeApplied}; '
        'actual_label=${tunActualLabel(false, actual)}');
  });
}
