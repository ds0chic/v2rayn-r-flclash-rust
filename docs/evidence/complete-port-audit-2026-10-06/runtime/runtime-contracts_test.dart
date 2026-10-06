import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/runtime/tun_toggle.dart';

class AuditRuntime implements RuntimeBridge, ExplicitTargetRuntimeBridge {
  RuntimeView view = const RuntimeView();
  Completer<void>? firstApplyGate;
  RuntimeErrorView? applyError;
  bool appliesDespiteUnknownResult = false;
  final List<String> calls = [];

  @override
  String? activeProfileId() => 'synthetic-node';
  @override
  BigInt desiredRevision() => BigInt.one;
  @override
  Future<RuntimeView> snapshot() async => view;
  @override
  Stream<RuntimeEvent> events() => const Stream.empty();
  @override
  Future<RuntimeActionResult> applyActive({required BigInt expectedRevision}) =>
      applyTarget(
        targetId: 'synthetic-node',
        expectedRevision: expectedRevision,
      );
  @override
  Future<RuntimeActionResult> applyTarget({
    required String targetId,
    required BigInt expectedRevision,
  }) async {
    calls.add('apply:$targetId');
    if (calls.length == 1 && firstApplyGate != null)
      await firstApplyGate!.future;
    if (applyError == null || appliesDespiteUnknownResult) {
      view = RuntimeView(
        state: 'Running',
        hostAlive: true,
        ports: [11977],
        sessionId: 'synthetic-$targetId',
      );
    }
    return RuntimeActionResult(
      ok: applyError == null,
      operationId: 'synthetic-op',
      error: applyError,
    );
  }

  @override
  Future<RuntimeActionResult> stop() async {
    calls.add('stop');
    view = const RuntimeView();
    return const RuntimeActionResult(ok: true);
  }
}

const liveLease = RuntimeTunView(
  adapterName: 'synthetic-audit-tun',
  interfaceIndex: 7,
  routeCount: 0,
  dryRun: false,
);

void main() {
  test(
    'actual active lease remains enabled when desired disable has not applied',
    () {
      const view = RuntimeView(
        state: 'Running',
        sessionId: 'synthetic-old',
        tun: liveLease,
      );
      expect(
        tunActualLabel(false, view),
        startsWith('已启用'),
        reason: 'desired=false does not prove a live lease disappeared',
      );
    },
  );
  test(
    'failed candidate does not describe retained old TUN lease as rolled back',
    () {
      const view = RuntimeView(
        state: 'Running',
        sessionId: 'synthetic-old',
        tun: liveLease,
        error: RuntimeErrorView(
          code: 'E_TUN_HELPER_UNAVAILABLE',
          messageKey: 'error.tun_helper_denied',
        ),
      );
      expect(
        tunActualLabel(true, view),
        startsWith('已启用'),
        reason:
            'retained actual lease must win over candidate failure feedback',
      );
    },
  );
  test('a dry-run lease must not be presented as an enabled real adapter', () {
    const view = RuntimeView(
      state: 'Running',
      tun: RuntimeTunView(
        adapterName: 'synthetic-dry',
        interfaceIndex: 7,
        routeCount: 0,
        dryRun: true,
      ),
    );
    expect(tunActualLabel(true, view), isNot(startsWith('已启用')));
  });
  test('latest start intent after a queued stop leaves the runtime running', () async {
    final bridge = AuditRuntime()..firstApplyGate = Completer<void>();
    final container = ProviderContainer(
      overrides: [runtimeBridgeProvider.overrideWithValue(bridge)],
    );
    addTearDown(container.dispose);
    final controller = container.read(runtimeControllerProvider.notifier);
    final initial = controller.applyActive(targetId: 'a');
    await Future<void>.delayed(Duration.zero);
    final stop = controller.stop();
    final latest = controller.applyActive(targetId: 'b');
    bridge.firstApplyGate!.complete();
    await Future.wait([initial, stop, latest]);
    print(
      'OBSERVED queue=${bridge.calls}; final=${container.read(runtimeControllerProvider).state}',
    );
    expect(
      container.read(runtimeControllerProvider).isRunning,
      isTrue,
      reason: 'the newer user intent was start b, not stop',
    );
  });
  test('an unknown apply result is reconciled from actual backend state', () async {
    final bridge = AuditRuntime()
      ..applyError = const RuntimeErrorView(
        code: 'E_TIMEOUT',
        messageKey: 'error.timeout',
      )
      ..appliesDespiteUnknownResult = true;
    final container = ProviderContainer(
      overrides: [runtimeBridgeProvider.overrideWithValue(bridge)],
    );
    addTearDown(container.dispose);
    final controller = container.read(runtimeControllerProvider.notifier);
    await controller.applyActive();
    final view = container.read(runtimeControllerProvider);
    print(
      'OBSERVED unknown apply backend=${bridge.view.state}, UI=${view.state}, reconcile=${view.reconcileNeeded}',
    );
    expect(
      view.isRunning,
      isTrue,
      reason: 'a timeout does not prove that a backend apply failed',
    );
    expect(view.reconcileNeeded, isTrue);
  });
  test(
    'failed apply remains a failure through the new bool result contract',
    () async {
      final bridge = AuditRuntime()
        ..applyError = const RuntimeErrorView(
          code: 'E_CORE_NOT_FOUND',
          messageKey: 'error.core_not_found',
        );
      final container = ProviderContainer(
        overrides: [runtimeBridgeProvider.overrideWithValue(bridge)],
      );
      addTearDown(container.dispose);
      final controller = container.read(runtimeControllerProvider.notifier);
      final result = await toggleTunDesired(
        enabled: false,
        persist: (_) => true,
        apply: controller.applyActive,
      );
      expect(result.ok, isFalse);
      expect(result.runtimeApplied, isFalse);
    },
  );
}
