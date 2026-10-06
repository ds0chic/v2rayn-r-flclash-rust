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
/// only when it did and must report whether the plan really applied — a void
/// completion is not an applied result (AUD-ROOT-02 / TUN-A03).
Future<TunToggleResult> toggleTunDesired({
  required bool enabled,
  required bool Function(bool enabled) persist,
  required Future<bool> Function() apply,
}) async {
  if (!persist(enabled)) {
    return const TunToggleResult(
      ok: false,
      error: 'error.settings_save_failed',
    );
  }
  final applied = await apply();
  if (!applied) {
    return const TunToggleResult(
      ok: false,
      error: 'error.tun_apply_failed',
      runtimeApplied: false,
    );
  }
  return const TunToggleResult(ok: true, runtimeApplied: true);
}

/// Actual TUN state read from the live runtime snapshot, never from the desired
/// switch. The snapshot carries the real lease facts (adapter/interface), so a
/// live lease reads 已启用; a dry-run lease reads 模拟 (no session was built);
/// desired=false with a live lease reads 仍启用(关闭待确认), never closed; a
/// helper refusal reads 失败已回滚; a stopped runtime reads 未启用 even when
/// the switch is on; a running plan without a lease reads 已请求(未验证).
/// Pending-cleanup leases still read from desired when the snapshot carries no
/// pending facts (needs the N-H1 pending surface over IPC/FRB).
String tunActualLabel(bool desired, RuntimeView runtime) {
  final tun = runtime.tun;
  if (tun != null && tun.dryRun) {
    return '模拟 (${tun.adapterName} if=${tun.interfaceIndex})';
  }
  if (runtime.error?.code == 'E_TUN_HELPER_UNAVAILABLE') return '失败已回滚';
  if (!runtime.isRunning) return '未启用';
  if (tun != null) {
    if (!desired) return '仍启用(关闭待确认)';
    return '已启用 (${tun.adapterName} if=${tun.interfaceIndex})';
  }
  if (!desired) return '未启用';
  return '已请求(未验证)';
}
