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

class RuntimeController extends Notifier<RuntimeView> {
  StreamSubscription<RuntimeEvent>? _events;
  Timer? _debounce;
  bool _started = false;
  bool _reloadInFlight = false;
  bool _reloadPending = false;
  int _pendingCommands = 0;
  BigInt? _lastEpoch;
  BigInt? _lastSeq;

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

  Future<void> refresh() async {
    final pending = _pendingCommands > 0;
    try {
      final snapshot = await _bridge.snapshot();
      state = snapshot.copyWith(
        epoch: _lastEpoch,
        lastSeq: _lastSeq,
        commandPending: pending,
      );
    } on Object catch (e) {
      state = state.copyWith(error: _bridgeError(e), commandPending: pending);
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
  Future<void> applyActive({String? targetId}) async {
    _pendingCommands++;
    state = state.copyWith(clearError: true, commandPending: true);
    try {
      await refresh();
      final explicit = _explicitBridge;
      final revision =
          state.desiredRevision ?? explicit?.desiredRevision() ?? BigInt.zero;
      final result = targetId == null
          ? await _bridge.applyActive(expectedRevision: revision)
          : await (explicit?.applyTarget(
                  targetId: targetId,
                  expectedRevision: revision,
                ) ??
                _bridge.applyActive(expectedRevision: revision));
      if (!result.ok) {
        // Keep the structured error; do not let the follow-up snapshot (which
        // reports Stopped) erase the reason apply failed.
        state = state.copyWith(error: result.error ?? _unknownError());
        return;
      }
      await refresh();
    } on Object catch (e) {
      state = state.copyWith(error: _bridgeError(e));
    } finally {
      _pendingCommands = _pendingCommands > 0 ? _pendingCommands - 1 : 0;
      state = state.copyWith(commandPending: _pendingCommands > 0);
    }
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

  Future<void> stop() async {
    try {
      final result = await _bridge.stop();
      if (!result.ok) {
        state = state.copyWith(error: result.error ?? _unknownError());
        return;
      }
      await refresh();
    } on Object catch (e) {
      state = state.copyWith(error: _bridgeError(e));
    }
  }

  RuntimeErrorView _bridgeError(Object e) => RuntimeErrorView(
    code: 'E_BRIDGE',
    messageKey: 'error.bridge_unavailable',
    detail: e.toString(),
  );

  RuntimeErrorView _unknownError() =>
      const RuntimeErrorView(code: 'E_INTERNAL', messageKey: 'error.unknown');
}
