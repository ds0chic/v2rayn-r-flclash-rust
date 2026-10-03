import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

/// In-memory [PlatformBridge] for widget tests.
///
/// It never loads the native library and never touches the host proxy, the Run
/// key or any OS facility. It records calls so tests can assert that a mode
/// toggle went through the bridge and that state is read back from a backend.
class FakePlatformBridge implements PlatformBridge {
  FakePlatformBridge({
    SysProxyMode initialMode = SysProxyMode.unchanged,
    this.pacRunning = false,
    this.uwpToolAvailable = true,
  }) : lastDesiredMode = initialMode;

  /// The last `desiredMode` passed to [getSystemProxyState].
  SysProxyMode lastDesiredMode;
  bool pacRunning;
  bool uwpToolAvailable;

  /// Every `setSystemProxy` mode applied, in order.
  final List<SysProxyMode> appliedModes = <SysProxyMode>[];

  /// Autostart state keyed by Run value name.
  final Map<String, bool> autostart = <String, bool>{};

  /// Whether the next apply should fail (fault injection).
  bool failNextApply = false;

  /// Whether restore should report a user conflict.
  bool restoreConflict = false;

  /// PAC server state driven by [pacStart]/[pacStop].
  String? pacText;
  int pacStartCount = 0;
  int pacStopCount = 0;

  /// Whether the next PAC start should fail (fault injection).
  bool failNextPac = false;

  @override
  PlatformView getSystemProxyState(int desiredMode) {
    final mode = SysProxyMode.fromValue(desiredMode);
    lastDesiredMode = mode;
    final enabled = mode == SysProxyMode.forcedChange;
    return PlatformView(
      loaded: true,
      desiredMode: mode,
      enabled: enabled,
      server: enabled ? '127.0.0.1:10809' : null,
      bypass: enabled ? '<local>' : null,
      autoConfigUrl: mode == SysProxyMode.pac
          ? 'http://127.0.0.1:11808/pac'
          : null,
      hasOwnership: enabled,
      pacRunning: mode == SysProxyMode.pac && pacRunning,
      pacUrl: mode == SysProxyMode.pac ? 'http://127.0.0.1:11808/pac' : null,
      pacPort: mode == SysProxyMode.pac ? 11808 : null,
    );
  }

  @override
  PlatformActionResult setSystemProxy({
    required int mode,
    String? server,
    String? bypass,
    String? autoConfigUrl,
  }) {
    if (failNextApply) {
      failNextApply = false;
      return const PlatformActionResult(
        ok: false,
        error: PlatformErrorView(
          code: 'E_PLATFORM_BACKEND',
          messageKey: 'error.platform_backend',
        ),
      );
    }
    appliedModes.add(SysProxyMode.fromValue(mode));
    return const PlatformActionResult(ok: true);
  }

  @override
  PlatformActionResult restoreSystemProxy() {
    if (restoreConflict) {
      restoreConflict = false;
      return const PlatformActionResult(
        ok: true,
        clean: false,
        conflicts: <ProxyConflictView>[
          ProxyConflictView(field: 'Server', currentValue: 'user:1'),
        ],
      );
    }
    return const PlatformActionResult(ok: true);
  }

  @override
  PlatformActionResult restoreSystemProxyOnExit(int desiredMode) =>
      restoreSystemProxy();

  @override
  bool getAutostart(String name) => autostart[name] ?? false;

  @override
  bool setAutostart({
    required String name,
    required bool enabled,
    required String exe,
    required String args,
  }) {
    autostart[name] = enabled;
    return true;
  }

  @override
  String autostartValueName(String startupPath) =>
      'v2rayNAutoRun_${startupPath.hashCode.toRadixString(16)}';

  @override
  bool validateCustomProxyScript({String? pacPath, String? scriptPath}) =>
      pacPath == null && scriptPath == null;

  @override
  bool resolveUwpLoopbackTool({String? binDir}) => uwpToolAvailable;

  @override
  PacHandleView pacStart({
    required String pacText,
    String? proxyRule,
    int port = 0,
  }) {
    if (failNextPac) {
      failNextPac = false;
      return const PacHandleView(
        ok: false,
        error: PlatformErrorView(
          code: 'E_PAC_START',
          messageKey: 'error.pac_start',
        ),
      );
    }
    pacStartCount++;
    this.pacText = pacText;
    pacRunning = true;
    return const PacHandleView(
      ok: true,
      running: true,
      url: 'http://127.0.0.1:11808/pac',
      port: 11808,
    );
  }

  @override
  PacHandleView pacStartFromFile({
    required String pacPath,
    String? proxyRule,
    int port = 0,
  }) => pacStart(pacText: 'file:$pacPath', proxyRule: proxyRule, port: port);

  @override
  bool pacStop() {
    pacStopCount++;
    pacRunning = false;
    return true;
  }

  @override
  PacHandleView pacState() => PacHandleView(
    ok: true,
    running: pacRunning,
    url: pacRunning ? 'http://127.0.0.1:11808/pac' : null,
    port: pacRunning ? 11808 : null,
  );

  @override
  void initBackend(String backend) {
    // No-op for the fake.
  }
}
