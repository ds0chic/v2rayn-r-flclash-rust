import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';

/// Single source of truth for the runtime status bar and toolbar.
///
/// The controller never invents a running state: [RuntimeView] is replaced
/// only by a `get_snapshot` result or by a structured error from apply/stop.
/// Events act as refresh triggers, never as an independent state store.
final runtimeControllerProvider =
    NotifierProvider<RuntimeController, RuntimeView>(RuntimeController.new);

/// One queued user intent in the single authoritative command sequence
/// (SP-04; mirrors `ipc_contract::stable::RuntimeIntentAction` Start/Stop).
///
/// The controller owns exactly one FIFO: submission order is execution order.
/// A stop is a cleanup barrier that is never superseded or merged away, not
/// even when it has not started yet. Only consecutive unstarted applies may
/// merge (last target wins); merging never crosses a stop barrier and never
/// changes the applied semantics. Transport ACK is not a business commit:
/// waiters complete with the command outcome once the backend truth has been
/// re-read, never with a queued guess.
class _RuntimeCommand {
  _RuntimeCommand.apply(this.targetId, this.enqueuedAt, this.intentSeq)
    : kind = _CommandKind.apply;

  _RuntimeCommand.stop(this.enqueuedAt, this.intentSeq)
    : kind = _CommandKind.stop,
      targetId = null;

  final _CommandKind kind;
  String? targetId;
  final DateTime enqueuedAt;

  /// Monotonic submission order. A reply tagged with an older sequence must
  /// never overwrite a newer view (SP-04 old-reply eviction).
  final int intentSeq;
  final List<Completer<bool>> waiters = <Completer<bool>>[];

  bool get isStop => kind == _CommandKind.stop;

  void complete(bool ok) {
    final pending = List<Completer<bool>>.of(waiters);
    waiters.clear();
    for (final waiter in pending) {
      if (!waiter.isCompleted) waiter.complete(ok);
    }
  }
}

enum _CommandKind { apply, stop }

class RuntimeController extends Notifier<RuntimeView> {
  StreamSubscription<RuntimeEvent>? _events;
  Timer? _debounce;
  bool _started = false;
  bool _reloadInFlight = false;
  bool _reloadPending = false;
  BigInt? _lastEpoch;
  BigInt? _lastSeq;

  /// In-flight refresh future; concurrent [refresh] calls share it instead of
  /// stacking snapshots on the shared IPC lock (R4-04 refresh merge).
  Future<void>? _refreshFuture;
  bool _refreshPending = false;

  /// Monotonic generation advanced when a command starts or an external epoch
  /// change is observed while idle. A snapshot/result tagged with an older
  /// generation is dropped rather than overwriting current state (R4-04).
  int _stateGeneration = 0;

  _RuntimeCommand? _activeCommand;

  /// The single authoritative command sequence (SP-04). Submission order is
  /// execution order; at most the tail may coalesce (unstarted apply merged
  /// into an unstarted apply). Stops are barriers and always keep their slot.
  final List<_RuntimeCommand> _commandQueue = <_RuntimeCommand>[];
  int _intentSeq = 0;
  bool _pumping = false;
  int _staleResponsesDropped = 0;

  /// Bound on how long a queued command may wait before execution; the same
  /// deadline spans queue time and execution (R4-04 D07).
  Duration commandDeadline = const Duration(minutes: 2);

  /// Injectable clock so tests can assert the queue deadline deterministically.
  DateTime Function() clock = DateTime.now;

  /// Diagnostics: whether normal-launch restore found a persisted active node
  /// and invoked apply. Not part of the read model.
  bool restoreAttempted = false;

  @override
  RuntimeView build() {
    ref.onDispose(() {
      _events?.cancel();
      _debounce?.cancel();
    });
    return const RuntimeView();
  }

  RuntimeBridge get _bridge => ref.read(runtimeBridgeProvider);

  ExplicitTargetRuntimeBridge? get _explicitBridge =>
      _bridge is ExplicitTargetRuntimeBridge
      ? _bridge as ExplicitTargetRuntimeBridge
      : null;

  OperationQueryBridge? get _queryBridge =>
      _bridge is OperationQueryBridge ? _bridge as OperationQueryBridge : null;

  /// Subscribe to runtime events and load the initial snapshot once.
  Future<void> start() async {
    if (_started) return;
    _started = true;
    _events ??= _bridge.events().listen(_onEvent, onError: (_) {});
    await refresh();
  }

  void _onEvent(RuntimeEvent event) {
    _trackStreamPosition(event);
    const refreshKinds = <String>{
      'runtime_detail',
      'runtime_state_changed',
      'error_raised',
      'lease_reclaimed',
    };
    if (!refreshKinds.contains(event.kind)) return;
    _debounce?.cancel();
    _debounce = Timer(const Duration(milliseconds: 120), refresh);
  }

  /// Record the last `(epoch, seq)` and flag a gap/reorder. A reconnect or a
  /// net-host restart bumps the epoch; within an epoch a sequence must be
  /// contiguous. Full replay/reconnect is a T09+ item; here we only refuse to
  /// pretend the stream was complete.
  void _trackStreamPosition(RuntimeEvent event) {
    final epoch = event.epoch;
    final seq = event.seq;
    if (epoch == null || seq == null) return;
    final previousEpoch = _lastEpoch;
    final previousSeq = _lastSeq;
    String? warning;
    if (previousEpoch != null && epoch == previousEpoch) {
      if (previousSeq != null && seq <= previousSeq) {
        warning = 'runtime event out of order: seq=$seq <= last=$previousSeq';
      } else if (previousSeq != null && seq != previousSeq + BigInt.one) {
        warning = 'runtime event gap: seq=$seq after $previousSeq';
      }
    } else if (previousEpoch != null && epoch != previousEpoch) {
      // An epoch change while no command is in flight means the session
      // changed underneath us; stale snapshots/results from the old epoch must
      // not overwrite the new state (R4-04).
      if (_activeCommand == null) _stateGeneration++;
      debugPrint('[runtime] event epoch changed: $previousEpoch -> $epoch');
    }
    _lastEpoch = epoch;
    _lastSeq = seq;
    if (warning != null) {
      debugPrint('[runtime] $warning');
      state = state.copyWith(
        epoch: epoch,
        lastSeq: seq,
        sequenceWarning: warning,
      );
    } else {
      state = state.copyWith(epoch: epoch, lastSeq: seq);
    }
  }

  /// True while a command is active or queued. Exposed through the read model,
  /// never confused with the backend's own Starting/Running fact.
  bool get _commandBusy => _activeCommand != null || _commandQueue.isNotEmpty;

  int get _pendingCount =>
      (_activeCommand != null ? 1 : 0) + _commandQueue.length;

  void _publishCommandState() {
    state = state.copyWith(
      commandPending: _commandBusy,
      pendingCommands: _pendingCount,
      staleResponsesDropped: _staleResponsesDropped,
    );
  }

  /// Load the latest snapshot. Concurrent calls are merged onto one in-flight
  /// request (a storm of event refreshes cannot stack on the IPC lock), and a
  /// response that lost a race against a newer generation is dropped.
  Future<void> refresh() {
    final inFlight = _refreshFuture;
    if (inFlight != null) {
      _refreshPending = true;
      return inFlight;
    }
    final future = _runRefreshLoop();
    _refreshFuture = future;
    return future;
  }

  Future<void> _runRefreshLoop() async {
    try {
      var passes = 0;
      do {
        if (passes++ >= 4) break;
        _refreshPending = false;
        final generation = _stateGeneration;
        final epochAtStart = _lastEpoch;
        try {
          final snapshot = await _bridge.snapshot();
          final stale =
              generation != _stateGeneration ||
              (_activeCommand == null && epochAtStart != _lastEpoch);
          if (stale) {
            _staleResponsesDropped++;
            _refreshPending = true;
            continue;
          }
          state = snapshot.copyWith(
            epoch: _lastEpoch,
            lastSeq: _lastSeq,
            commandPending: _commandBusy,
            pendingCommands: _pendingCount,
            staleResponsesDropped: _staleResponsesDropped,
          );
        } on Object catch (e) {
          if (generation != _stateGeneration) {
            _staleResponsesDropped++;
            continue;
          }
          state = state.copyWith(
            error: _bridgeError(e),
            commandPending: _commandBusy,
            pendingCommands: _pendingCount,
            staleResponsesDropped: _staleResponsesDropped,
          );
        }
      } while (_refreshPending);
    } finally {
      _refreshFuture = null;
    }
  }

  /// Apply the real persisted plan for the active node.
  ///
  /// When [targetId] is given it is the explicit target frozen at click time
  /// (R4-02); otherwise the persisted default node is used (F5/restore).
  ///
  /// A historical snapshot error is feedback, not a veto: the user repaired the
  /// cause and a new command must be submitted (R4-01 / D02). The desired
  /// revision is re-read from a fresh snapshot immediately before apply
  /// (upstream `SetDefaultServer` -> `Reload` semantics), falling back to the
  /// local persisted revision when the snapshot itself failed. A failed apply
  /// keeps its structured error visible; the follow-up snapshot never overwrites
  /// it with a fake success.
  ///
  /// Returns true only when the submitted plan really applied; a void
  /// completion is not an applied result (AUD-ROOT-02 / TUN-A03).
  Future<bool> applyActive({String? targetId}) async {
    final command = _RuntimeCommand.apply(targetId, clock(), ++_intentSeq);
    final waiter = Completer<bool>();
    command.waiters.add(waiter);
    _enqueue(command);
    // Local pending is visible synchronously, before any await: the UI shows
    // "requesting" immediately without claiming the backend is running.
    state = state.copyWith(
      clearError: true,
      commandPending: true,
      pendingCommands: _pendingCount,
      reconcileNeeded: false,
    );
    unawaited(_pumpCommands());
    return waiter.future;
  }

  /// Append [command] to the authoritative sequence. Only an unstarted apply
  /// directly behind another unstarted apply coalesces (last target wins, and
  /// the merged waiters observe the surviving intent's outcome so no caller
  /// hangs). A stop is never merged or dropped: it keeps its barrier slot
  /// even when newer applies arrive behind it.
  void _enqueue(_RuntimeCommand command) {
    if (!command.isStop &&
        _commandQueue.isNotEmpty &&
        !_commandQueue.last.isStop) {
      final tail = _commandQueue.last;
      tail.targetId = command.targetId;
      tail.waiters.addAll(command.waiters);
      _publishCommandState();
      return;
    }
    _commandQueue.add(command);
    _publishCommandState();
  }

  /// Drop every queued-but-unstarted command (SP-04 cancel semantics).
  ///
  /// Each dropped waiter completes with false: its intent never executed, so
  /// no success may be reported. The active command, if any, runs to its safe
  /// point first: an in-flight apply cannot be cancelled mid-flight (the
  /// net-host client reports `NotCancellable`), it is only followed by
  /// whatever the sequence still holds. Returns the number of dropped
  /// commands.
  Future<int> cancelPending() async {
    final dropped = _commandQueue.length;
    final pending = List<_RuntimeCommand>.of(_commandQueue);
    _commandQueue.clear();
    for (final command in pending) {
      command.complete(false);
    }
    _publishCommandState();
    return dropped;
  }

  /// Serialize and bound command execution in strict submission order
  /// (SP-04). One command runs while the rest wait in [_commandQueue]; no
  /// priority lane lets a later apply overtake an earlier stop.
  Future<void> _pumpCommands() async {
    if (_pumping) return;
    _pumping = true;
    try {
      while (_commandQueue.isNotEmpty) {
        final next = _commandQueue.removeAt(0);
        _activeCommand = next;
        _publishCommandState();

        if (clock().difference(next.enqueuedAt) > commandDeadline) {
          // The queue deadline (which includes waiting) expired: reject the
          // stale intent instead of executing it late.
          state = state.copyWith(
            error: const RuntimeErrorView(
              code: 'E_TIMEOUT',
              messageKey: 'error.runtime_timeout',
              detail: 'command expired while queued',
            ),
          );
          next.complete(false);
          _activeCommand = null;
          _publishCommandState();
          continue;
        }

        final ok = await _executeCommand(next);
        next.complete(ok);
        _activeCommand = null;
        _publishCommandState();
      }
    } finally {
      _pumping = false;
      _publishCommandState();
    }
  }

  /// Reconcile an unknown outcome (SP-04): a timeout, disconnect or lost
  /// reply proves nothing about the backend. Query the recorded operation
  /// first (best effort, never throws), then re-read the authoritative
  /// snapshot; the original error stays visible and [reconcileNeeded] tells
  /// the UI the state came from reconciliation. Never re-executes the
  /// command: a retried submit would be a second, non-idempotent write.
  Future<void> _reconcileUnknown(
    RuntimeErrorView error, {
    String? operationId,
  }) async {
    final id = operationId;
    if (id != null) {
      try {
        await _queryBridge?.operationStatus(id);
      } on Object catch (_) {
        // The query itself is best effort; the snapshot below is authoritative.
      }
    }
    try {
      await refresh();
    } on Object catch (_) {
      // Even a failed re-read must not hide the unknown outcome.
    }
    state = state.copyWith(error: error, reconcileNeeded: true);
  }

  /// Returns true only when the submitted plan really applied; a void
  /// completion, a superseded intent or an unknown outcome is not an applied
  /// result (AUD-ROOT-02 / TUN-A03 / SP-04).
  Future<bool> _executeCommand(_RuntimeCommand command) async {
    if (command.isStop) {
      _stateGeneration++;
      final generation = _stateGeneration;
      try {
        final result = await _bridge.stop();
        if (generation != _stateGeneration) {
          _noteStaleResponse();
          return false;
        }
        if (result.ok) {
          await refresh();
          return true;
        }
        if (_isUnknownOutcome(result.error)) {
          // Outcome unknown (timeout/disconnect): reconcile from truth instead
          // of reporting a success or a definitive failure.
          await _reconcileUnknown(
            result.error ?? _unknownError(),
            operationId: result.operationId,
          );
          return false;
        }
        state = state.copyWith(error: result.error ?? _unknownError());
        return false;
      } on Object catch (e) {
        if (generation != _stateGeneration) {
          _noteStaleResponse();
          return false;
        }
        // A thrown transport error is an unknown outcome too (the request may
        // have been applied before the reply was lost): reconcile, no replay.
        await _reconcileUnknown(_bridgeError(e), operationId: null);
        return false;
      }
    }

    _stateGeneration++;
    final generation = _stateGeneration;
    try {
      await refresh();
      final explicit = _explicitBridge;
      final revision =
          state.desiredRevision ?? explicit?.desiredRevision() ?? BigInt.zero;
      final targetId = command.targetId;
      final result = targetId == null
          ? await _bridge.applyActive(expectedRevision: revision)
          : await (explicit?.applyTarget(
                  targetId: targetId,
                  expectedRevision: revision,
                ) ??
                _bridge.applyActive(expectedRevision: revision));
      if (generation != _stateGeneration) {
        _noteStaleResponse();
        return false;
      }
      if (!result.ok) {
        if (_isUnknownOutcome(result.error)) {
          // The backend may be Running despite the timeout: query first,
          // show the reconciled truth, keep the original error visible.
          await _reconcileUnknown(
            result.error ?? _unknownError(),
            operationId: result.operationId,
          );
          return false;
        }
        // Keep the structured error; do not let the follow-up snapshot (which
        // reports Stopped) erase the reason apply failed.
        state = state.copyWith(error: result.error ?? _unknownError());
        return false;
      }
      await refresh();
      return true;
    } on Object catch (e) {
      if (generation != _stateGeneration) {
        _noteStaleResponse();
        return false;
      }
      await _reconcileUnknown(_bridgeError(e), operationId: null);
      return false;
    }
  }

  void _noteStaleResponse() {
    _staleResponsesDropped++;
    state = state.copyWith(staleResponsesDropped: _staleResponsesDropped);
  }

  bool _isUnknownOutcome(RuntimeErrorView? error) {
    if (error == null) return false;
    return error.code == 'E_TIMEOUT' ||
        error.code == 'E_UNAVAILABLE' ||
        error.code == 'E_BRIDGE';
  }

  /// Normal-startup restore: apply the persisted active node once, unless the
  /// runtime is already running. This is the production path (upstream
  /// `MainWindowViewModel.Init` -> `Reload`), not an env-armed test hook.
  Future<void> restoreActiveOnLaunch() async {
    if (_bridge.activeProfileId() == null) return;
    await refresh();
    // A historical run error must not block the launch restore; only an
    // already-running session makes it a no-op.
    if (state.isRunning) return;
    restoreAttempted = true;
    await applyActive();
  }

  /// Shared reload use case for the F5 shortcut and the ACT-MAIN-035 menu
  /// (upstream `MainWindowViewModel.Reload`: re-read the latest desired plan and
  /// re-apply it). A call while a reload is already running marks a pending job
  /// that runs once after the in-flight one finishes (upstream
  /// `_hasNextReloadJob`), so a fast double F5 never loses the last request.
  /// With no persisted active node it refreshes and prompts the user (upstream
  /// `NoticeManager.Enqueue(CheckServerSettings)`).
  Future<void> reload() async {
    if (_reloadInFlight) {
      _reloadPending = true;
      return;
    }
    _reloadInFlight = true;
    try {
      await refresh();
      // A historical snapshot/run error must not veto the reload: the reload
      // re-reads the latest plan and submits the new attempt (R4-01 / D02).
      if (_bridge.activeProfileId() == null) {
        ref
            .read(uiShellControllerProvider.notifier)
            .setMessage('配置项无效，请检查或重新选择');
        return;
      }
      await applyActive();
    } finally {
      _reloadInFlight = false;
      if (_reloadPending) {
        _reloadPending = false;
        await reload();
      }
    }
  }

  /// SR-03 restore-lifecycle hook: the engine exchanged storage and stopped
  /// the managed session. Drop any stale running view (idempotent stop), then
  /// re-apply the restored active node at the original post-restore timing.
  /// No active node leaves the runtime stopped; an apply failure stays visible
  /// and is never replaced by a fabricated Running state.
  Future<void> resyncAfterRestore() async {
    await stop();
    if (state.error != null) return;
    if (_bridge.activeProfileId() == null) return;
    await applyActive();
  }

  /// Enqueue a stop barrier (SP-04). Stops are never merged: two rapid
  /// stops keep two barrier slots (stop is idempotent, so executing both is
  /// safe), and a stop is never overtaken by a later apply.
  Future<void> stop() {
    final command = _RuntimeCommand.stop(clock(), ++_intentSeq);
    final waiter = Completer<bool>();
    command.waiters.add(waiter);
    _enqueueStop(command);
    state = state.copyWith(
      commandPending: true,
      pendingCommands: _pendingCount,
      reconcileNeeded: false,
    );
    unawaited(_pumpCommands());
    return waiter.future.then((_) {});
  }

  void _enqueueStop(_RuntimeCommand command) {
    _commandQueue.add(command);
    _publishCommandState();
  }

  RuntimeErrorView _bridgeError(Object e) => RuntimeErrorView(
    code: 'E_BRIDGE',
    messageKey: 'error.bridge_unavailable',
    detail: e.toString(),
  );

  RuntimeErrorView _unknownError() =>
      const RuntimeErrorView(code: 'E_INTERNAL', messageKey: 'error.unknown');
}
