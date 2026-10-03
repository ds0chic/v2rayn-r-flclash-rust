import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';

/// Outcome of a TUN desired-state transition (FIX-13).
///
/// [ok] is true only after the settings save succeeded; [runtimeApplied] tells
/// whether the plan was re-applied. A failed save never reports success and
/// never touches the running session.
class TunToggleResult {
  const TunToggleResult({
    required this.ok,
    this.error,
    this.runtimeApplied = false,
  });

  final bool ok;
  final String? error;
  final bool runtimeApplied;
}

/// Persist the desired TUN flag first, then apply the real plan.
///
/// Extracted as a pure seam so widget tests can cover the persist-failure and
/// authorization/cancel branches without a native bridge. The persist callback
/// must return true only after the settings actually saved; [apply] is invoked
/// only when it did.
Future<TunToggleResult> toggleTunDesired({
  required bool enabled,
  required bool Function(bool enabled) persist,
  required Future<void> Function() apply,
}) async {
  if (!persist(enabled)) {
    return const TunToggleResult(
      ok: false,
      error: 'error.settings_save_failed',
    );
  }
  await apply();
  return const TunToggleResult(ok: true, runtimeApplied: true);
}

/// Actual TUN state read from the live runtime snapshot, never from the desired
/// switch. The bridge carries no per-lease TUN facts, so only what the snapshot
/// proves is shown: a helper refusal reads 失败已回滚; a stopped runtime reads
/// 未启用 even when the switch is on; a live runtime reads 已请求(未验证).
String tunActualLabel(bool desired, RuntimeView runtime) {
  if (!desired) return '未启用';
  if (runtime.error?.code == 'E_TUN_HELPER_UNAVAILABLE') return '失败已回滚';
  if (!runtime.isRunning) return '未启用';
  return '已请求(未验证)';
}
