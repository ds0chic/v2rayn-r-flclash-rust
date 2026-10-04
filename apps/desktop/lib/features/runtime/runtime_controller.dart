import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
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
    try {
      final snapshot = await _bridge.snapshot();
      state = snapshot.copyWith(epoch: _lastEpoch, lastSeq: _lastSeq);
    } on Object catch (e) {
      state = state.copyWith(error: _bridgeError(e));
    }
  }

  /// Apply the real persisted plan for the active node.
  ///
  /// The desired revision is re-read from a fresh snapshot immediately before
  /// apply (upstream `SetDefaultServer` -> `Reload` semantics), so a save that
  /// happened after the last snapshot is not rejected as `E_REVISION_STALE`.
  /// A failed apply keeps its structured error visible; the follow-up snapshot
  /// never overwrites it with a fake success.
  Future<void> applyActive() async {
    state = state.copyWith(clearError: true);
    await refresh();
    if (state.error != null) return;
    final revision = state.desiredRevision ?? BigInt.zero;
    try {
      final result = await _bridge.applyActive(expectedRevision: revision);
      if (!result.ok) {
        // Keep the structured error; do not let the follow-up snapshot (which
        // reports Stopped) erase the reason apply failed.
        state = state.copyWith(error: result.error ?? _unknownError());
        return;
      }
      await refresh();
    } on Object catch (e) {
      state = state.copyWith(error: _bridgeError(e));
    }
  }

  /// Normal-startup restore: apply the persisted active node once, unless the
  /// runtime is already running. This is the production path (upstream
  /// `MainWindowViewModel.Init` -> `Reload`), not an env-armed test hook.
  Future<void> restoreActiveOnLaunch() async {
    if (_bridge.activeProfileId() == null) return;
    await refresh();
    if (state.isRunning || state.error != null) return;
    restoreAttempted = true;
    await applyActive();
  }

  /// Shared reload use case for the F5 shortcut and the ACT-MAIN-035 menu
  /// (upstream `MainWindowViewModel.Reload`: re-read the latest desired plan and
  /// re-apply it). Busy-protected so a reload never stomps an in-flight
  /// apply/stop; with no persisted active node it only refreshes the snapshot.
  Future<void> reload() async {
    if (state.isBusy) return;
    await refresh();
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
