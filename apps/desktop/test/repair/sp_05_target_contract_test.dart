import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';

/// SP-05: frozen applied target and job terminal states (Dart contract half).
///
/// The Rust engine owns the freeze (submit-time target/operation/intent);
/// this file locks the Dart side of the same contract with a scripted
/// bridge (no native library, no sockets, no OS writes; synthetic ids and
/// probed-style ports >= 11808 only):
///
/// - `applyTarget` forwards the frozen target id verbatim: a later desired
///   default change never rewrites the submitted plan's target;
/// - the returned `<operation_id>:<job_id>` correlation resolves through
///   the read-only `operationStatus` path in both compound and raw form;
/// - `stop` withdraws the live target but keeps the frozen history (never
///   relabels it to the newer default);
/// - a stale `expectedRevision` is rejected without touching the backend.
class FrozenTargetBridge
    implements
        RuntimeBridge,
        ExplicitTargetRuntimeBridge,
        OperationQueryBridge {
  final StreamController<RuntimeEvent> _events =
      StreamController<RuntimeEvent>.broadcast();

  /// Currently desired default (mutable by the test, like `set_active`).
  String desiredDefault = 'node-a';

  /// Desired revision the backend expects.
  BigInt desiredRevisionValue = BigInt.one;

  /// Backend call log (`apply:<target>`, `stop`).
  final List<String> calls = [];

  /// Submit-time frozen history: target/operation/intent, kept across stop.
  String? appliedHistoryTarget;
  String? appliedHistoryOperation;
  int appliedHistoryIntent = 0;

  /// Live target, withdrawn on stop.
  String? liveTarget;

  /// Blocks the next apply until released (A in flight).
  Completer<void>? applyGate;

  int _ops = 0;
  int _intents = 0;
  final Map<String, String> _opTarget = {};
  final Map<String, String> _opState = {};

  Future<void> get done => _events.close();

  @override
  String? activeProfileId() => desiredDefault;

  @override
  BigInt desiredRevision() => desiredRevisionValue;

  @override
  Stream<RuntimeEvent> events() => _events.stream;

  @override
  Future<RuntimeView> snapshot() async => RuntimeView(
    state: liveTarget == null ? 'Stopped' : 'Running',
    hostAlive: true,
    ports: liveTarget == null ? const <int>[] : const <int>[11951],
    sessionId: liveTarget == null ? null : 'synthetic-$liveTarget',
  );

  @override
  Future<RuntimeActionResult> applyActive({required BigInt expectedRevision}) =>
      applyTarget(targetId: desiredDefault, expectedRevision: expectedRevision);

  @override
  Future<RuntimeActionResult> applyTarget({
    required String targetId,
    required BigInt expectedRevision,
  }) async {
    if (expectedRevision != desiredRevisionValue) {
      return const RuntimeActionResult(
        ok: false,
        error: RuntimeErrorView(
          code: 'E_REVISION_STALE',
          messageKey: 'error.revision_stale',
        ),
      );
    }
    calls.add('apply:$targetId');
    final gate = applyGate;
    if (gate != null) {
      applyGate = null;
      await gate.future;
    }
    // Freeze at submit: later default changes cannot rewrite this record.
    _ops++;
    _intents++;
    final op = 'sp05-op-$_ops';
    appliedHistoryTarget = targetId;
    appliedHistoryOperation = op;
    appliedHistoryIntent = _intents;
    liveTarget = targetId;
    _opTarget[op] = targetId;
    _opState[op] = 'running';
    return RuntimeActionResult(ok: true, operationId: '$op:sp05-job-$_ops');
  }

  @override
  Future<RuntimeActionResult> stop() async {
    calls.add('stop');
    liveTarget = null;
    // History is retained, never relabeled to the newer default.
    for (final op in _opTarget.keys) {
      _opState[op] = 'cancelled';
    }
    return const RuntimeActionResult(ok: true);
  }

  /// Drives one accepted operation to its terminal backend state (mirrors
  /// net-host writing Done once the core is ready).
  void markReady(String operationId) {
    if (_opState.containsKey(operationId)) {
      _opState[operationId] = 'done';
    }
  }

  @override
  Future<RuntimeOperationView?> operationStatus(String operationId) async {
    // The compound correlation is split, never sent verbatim (RUN-05).
    final raw = operationId.split(':').first;
    final state = _opState[raw];
    if (state == null) return null;
    return RuntimeOperationView(found: true, operationId: raw, state: state);
  }
}

void main() {
  test('frozen target survives a later default change', () async {
    final bridge = FrozenTargetBridge();
    addTearDown(() => bridge.done);
    final gate = Completer<void>();
    bridge.applyGate = gate;

    final pending = bridge.applyTarget(
      targetId: 'node-a',
      expectedRevision: BigInt.one,
    );
    // A is in flight inside the backend; the user edits the default to B.
    await Future<void>.delayed(const Duration(milliseconds: 50));
    expect(bridge.calls, ['apply:node-a']);
    bridge.desiredDefault = 'node-b';

    gate.complete();
    final result = await pending;
    expect(result.ok, isTrue);

    // The submitted plan still names A; the desired default is separate.
    expect(bridge.appliedHistoryTarget, 'node-a');
    expect(bridge.desiredDefault, 'node-b');
    final view = await bridge.snapshot();
    expect(view.state, 'Running');
    expect(view.sessionId, 'synthetic-node-a');
    expect(bridge.appliedHistoryIntent, 1);
  });

  test('stop withdraws live but keeps history A', () async {
    final bridge = FrozenTargetBridge();
    addTearDown(() => bridge.done);
    final result = await bridge.applyTarget(
      targetId: 'node-a',
      expectedRevision: BigInt.one,
    );
    expect(result.ok, isTrue);
    bridge.desiredDefault = 'node-b';

    await bridge.stop();

    expect(bridge.liveTarget, isNull);
    expect(
      bridge.appliedHistoryTarget,
      'node-a',
      reason: 'stop must not rewrite history to the newer default B',
    );
    expect(bridge.calls, ['apply:node-a', 'stop']);
    final view = await bridge.snapshot();
    expect(view.state, 'Stopped');
  });

  test('compound operation id resolves raw and compound', () async {
    final bridge = FrozenTargetBridge();
    addTearDown(() => bridge.done);
    final result = await bridge.applyTarget(
      targetId: 'node-a',
      expectedRevision: BigInt.one,
    );
    final correlation = result.operationId!;
    expect(correlation, contains(':'));
    final raw = correlation.split(':').first;
    bridge.markReady(raw);

    final compoundView = await bridge.operationStatus(correlation);
    final rawView = await bridge.operationStatus(raw);
    expect(compoundView, isNotNull);
    expect(rawView, isNotNull);
    expect(compoundView!.operationId, raw);
    expect(rawView!.operationId, raw);
    expect(rawView.state, 'done');
    expect(await bridge.operationStatus('sp05-op-9999'), isNull);
  });

  test('stale revision rejected without backend touch', () async {
    final bridge = FrozenTargetBridge();
    addTearDown(() => bridge.done);
    final result = await bridge.applyTarget(
      targetId: 'node-a',
      expectedRevision: BigInt.from(999),
    );
    expect(result.ok, isFalse);
    expect(result.error!.code, 'E_REVISION_STALE');
    expect(bridge.calls, isEmpty);
    expect(bridge.appliedHistoryTarget, isNull);
    expect(bridge.appliedHistoryIntent, 0);
  });
}
