/// Armed-only evidence/automation environment hooks (ISSUE-08).
///
/// Default release builds must ignore every `V2RAYN_R_*` variable that could
/// change state; only builds compiled with
/// `--dart-define=V2RAYN_R_SMOKE_ARMED=true` honor them. The `V2RAYN_R_OPEN_*`
/// navigation hooks stay UI-only and are intentionally unguarded (see main.dart).
///
/// Parsing here is pure over an injected env map so unit tests can cover the
/// guard without touching `Platform.environment`.
library;

/// Compile-time arming flag, mirroring `smokeArmed` in `main.dart`.
const bool evidenceArmed = bool.fromEnvironment(
  'V2RAYN_R_SMOKE_ARMED',
  defaultValue: false,
);

/// Reserved user-proxy port (hard constraint 1): hooks never poll or bind it.
const int reservedProxyPort = 10808;

/// Lowest port an evidence/test hook may touch (hard constraint 1).
const int minEvidencePort = 11808;

/// Highest valid TCP/UDP port.
const int maxPort = 65535;

/// Parse `V2RAYN_R_MONITOR_XRAY_PORT` from [env]. Returns the port only when
/// [armed] and the value is a valid evidence port; otherwise null. The
/// reserved user-proxy port and anything below [minEvidencePort] is refused.
int? evidenceMonitorXrayPort(
  Map<String, String> env, {
  bool armed = evidenceArmed,
}) {
  if (!armed) return null;
  final raw = env['V2RAYN_R_MONITOR_XRAY_PORT'];
  if (raw == null) return null;
  final port = int.tryParse(raw);
  if (port == null) return null;
  if (port == reservedProxyPort) return null;
  if (port < minEvidencePort || port > maxPort) return null;
  return port;
}

/// Read a state-changing `V2RAYN_R_*` string hook ([key]) from [env].
/// Returns null unless [armed] and the variable is set.
String? evidenceStringHook(
  Map<String, String> env,
  String key, {
  bool armed = evidenceArmed,
}) {
  if (!armed) return null;
  return env[key];
}
