import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';

class _FakeRuntimeBridge implements RuntimeBridge {
  final StreamController<RuntimeEvent> eventController =
      StreamController<RuntimeEvent>.broadcast(sync: true);

  @override
  Future<RuntimeView> snapshot() async => const RuntimeView();

  @override
  String? activeProfileId() => null;

  @override
  Future<RuntimeActionResult> applyActive({
    required BigInt expectedRevision,
  }) async => const RuntimeActionResult(ok: true);

  @override
  Future<RuntimeActionResult> stop() async =>
      const RuntimeActionResult(ok: true);

  @override
  Stream<RuntimeEvent> events() => eventController.stream;
}

/// D3b-04: a sequence warning belongs to its epoch; a new epoch clears it.
void main() {
  test('epoch change clears a stale sequence warning', () async {
    final bridge = _FakeRuntimeBridge();
    addTearDown(bridge.eventController.close);
    final container = ProviderContainer(
      overrides: [runtimeBridgeProvider.overrideWithValue(bridge)],
    );
    addTearDown(container.dispose);
    final controller = container.read(runtimeControllerProvider.notifier);
    await controller.start();

    RuntimeEvent event(BigInt epoch, BigInt seq) => RuntimeEvent(
      kind: 'runtime_state_changed',
      payloadJson: '{}',
      epoch: epoch,
      seq: seq,
    );
    bridge.eventController.add(event(BigInt.one, BigInt.from(2)));
    bridge.eventController.add(event(BigInt.one, BigInt.one));
    expect(
      container.read(runtimeControllerProvider).sequenceWarning,
      isNotNull,
    );

    bridge.eventController.add(event(BigInt.two, BigInt.one));
    expect(container.read(runtimeControllerProvider).sequenceWarning, isNull);
    // Settle the controller's 120ms event-debounce refresh before teardown
    // disposes the container.
    await Future<void>.delayed(const Duration(milliseconds: 300));
  });
}
