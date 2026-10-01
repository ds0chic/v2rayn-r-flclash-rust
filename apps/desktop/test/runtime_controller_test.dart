import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';

/// Minimal bridge that lets the test drive the event stream directly.
class _FakeBridge implements RuntimeBridge {
  final StreamController<RuntimeEvent> _controller =
      StreamController<RuntimeEvent>.broadcast();

  void emit(RuntimeEvent event) => _controller.add(event);

  @override
  Future<RuntimeView> snapshot() async => const RuntimeView();

  @override
  Future<RuntimeActionResult> applySmoke({
    required BigInt expectedRevision,
  }) async => const RuntimeActionResult(ok: true);

  @override
  Future<RuntimeActionResult> stop() async =>
      const RuntimeActionResult(ok: true);

  @override
  Stream<RuntimeEvent> events() => _controller.stream;
}

void main() {
  test('revision label renders desired/applied without fabricating values', () {
    final view = RuntimeView(
      desiredRevision: BigInt.from(4),
      appliedRevision: BigInt.from(2),
    );
    expect(view.revisionLabel, 'rev: 4/2');
    expect(const RuntimeView().revisionLabel, 'rev: -/-');
  });

  test('controller records epoch/seq and flags out-of-order events', () async {
    final bridge = _FakeBridge();
    addTearDown(bridge._controller.close);
    final container = ProviderContainer(
      overrides: [runtimeBridgeProvider.overrideWithValue(bridge)],
    );
    addTearDown(container.dispose);

    final controller = container.read(runtimeControllerProvider.notifier);
    await controller.start();

    bridge.emit(
      RuntimeEvent(
        kind: 'runtime_detail',
        payloadJson: '{}',
        epoch: BigInt.one,
        seq: BigInt.from(5),
      ),
    );
    await Future<void>.delayed(Duration.zero);
    expect(container.read(runtimeControllerProvider).lastSeq, BigInt.from(5));

    // Same epoch, seq goes backwards: a gap/reorder, never silently replayed.
    bridge.emit(
      RuntimeEvent(
        kind: 'runtime_detail',
        payloadJson: '{}',
        epoch: BigInt.one,
        seq: BigInt.from(4),
      ),
    );
    await Future<void>.delayed(Duration.zero);
    expect(
      container.read(runtimeControllerProvider).sequenceWarning,
      isNotNull,
    );
  });
}
