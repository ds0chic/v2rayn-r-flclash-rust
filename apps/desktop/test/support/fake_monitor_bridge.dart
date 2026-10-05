import 'dart:async';

import 'package:v2rayn_desktop/bridge/api/contract.dart' as contract;
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';

/// In-memory [MonitorBridge] for widget tests.
///
/// It never loads the native library or opens a socket. Streams are driven
/// explicitly from the test (`emitTraffic`, `emitLogs`) and every mutation is
/// recorded so a test can assert the UI reached the bridge.
class FakeMonitorBridge implements MonitorBridge {
  FakeMonitorBridge({
    this.clashApiSupported = false,
    this.clashMessage,
    List<m.ClashProxyDto>? proxies,
    List<m.ClashConnectionDto>? connections,
    List<m.LogLineDto>? initialLogs,
    this.proxyDelayValue = 42,
    this.clashMode,
    this.clashModes = const <String>['Rule', 'Global', 'Direct'],
    this.modeFails = false,
    this.closeAllFails = false,
    this.statsError,
  }) : proxies = proxies ?? <m.ClashProxyDto>[],
       connections = connections ?? <m.ClashConnectionDto>[],
       logs = List<m.LogLineDto>.of(initialLogs ?? const <m.LogLineDto>[]);

  bool clashApiSupported;
  String? clashMessage;
  List<m.ClashProxyDto> proxies;
  List<m.ClashConnectionDto> connections;
  List<m.LogLineDto> logs;
  int proxyDelayValue;
  String? clashMode;
  List<String> clashModes;
  bool modeFails;
  bool closeAllFails;
  String? delayUrl;

  /// When set, [statsSnapshot] reports it so a store bind/load failure is
  /// visible to the controller.
  contract.ErrorDto? statsError;

  /// Optional gates awaited by [clashConnections]/[clashProxies]. A test holds
  /// these to keep a read in flight while the applied session changes (R4-23
  /// late-response eviction / in-flight coalescing).
  Future<void>? connectionsGate;
  Future<void>? proxiesGate;

  /// Per-node `ServerStatItem` rows returned by [statsSnapshot] (R3-09b).
  List<m.NodeTrafficDto> statsNodes = const <m.NodeTrafficDto>[];

  // Recorded calls.
  final List<String> configured = <String>[];
  final Map<String, bool> pageVisibility = <String, bool>{};
  final List<String> selectedProxies = <String>[];
  final List<String> testedProxies = <String>[];
  final List<String> testedGroups = <String>[];
  final List<String> closedConnections = <String>[];
  int closeAllCount = 0;
  int clearStatsCount = 0;
  int clearLogsCount = 0;
  int clashProxiesCount = 0;
  int clashConnectionsCount = 0;
  int subscribeTrafficCount = 0;
  int subscribeLogsCount = 0;
  int? lastMinLevel;
  int setLogFilterCalls = 0;
  bool? lastCollectingPaused;
  bool? lastScrollPaused;
  String? activeNode;
  bool pollingStarted = false;
  int syncSessionCount = 0;

  // Synchronous broadcast so a test that `emit`s then pumps one frame sees the
  // state update; asynchronous delivery would need an extra microtask drain.
  final StreamController<m.TrafficBatchDto> _traffic =
      StreamController<m.TrafficBatchDto>.broadcast(sync: true);
  final StreamController<m.LogBatchDto> _logController =
      StreamController<m.LogBatchDto>.broadcast(sync: true);

  /// Push one traffic batch to subscribers.
  void emitTraffic({
    BigInt? proxyUp,
    BigInt? proxyDown,
    BigInt? directUp,
    BigInt? directDown,
    BigInt? proxyUpBps,
    BigInt? proxyDownBps,
    BigInt? directUpBps,
    BigInt? directDownBps,
    List<m.NodeTrafficDto>? nodes,
  }) {
    _traffic.add(
      m.TrafficBatchDto(
        epoch: BigInt.one,
        seq: BigInt.one,
        generation: BigInt.zero,
        proxyUp: proxyUp ?? BigInt.zero,
        proxyDown: proxyDown ?? BigInt.zero,
        directUp: directUp ?? BigInt.zero,
        directDown: directDown ?? BigInt.zero,
        proxyUpBps: proxyUpBps ?? BigInt.zero,
        proxyDownBps: proxyDownBps ?? BigInt.zero,
        directUpBps: directUpBps ?? BigInt.zero,
        directDownBps: directDownBps ?? BigInt.zero,
        nodes: nodes ?? const <m.NodeTrafficDto>[],
      ),
    );
  }

  /// Push one log batch to subscribers.
  void emitLogs(
    List<m.LogLineDto> lines, {
    BigInt? droppedLines,
    BigInt? truncatedLines,
    bool collectingPaused = false,
    bool scrollPaused = false,
  }) {
    _logController.add(
      m.LogBatchDto(
        epoch: BigInt.one,
        seq: BigInt.one,
        lines: lines,
        droppedLines: droppedLines ?? BigInt.zero,
        droppedBytes: BigInt.zero,
        truncatedLines:
            truncatedLines ??
            BigInt.from(lines.where((line) => line.truncated).length),
        collectingPaused: collectingPaused,
        scrollPaused: scrollPaused,
      ),
    );
  }

  @override
  void configure({
    required int core,
    required int statePort,
    required int statePort2,
    String? secret,
    required bool enableStatistics,
    required bool displayRealTimeSpeed,
    required int refreshIntervalMs,
  }) {
    configured.add('core=$core;$statePort;$statePort2');
  }

  @override
  void setActiveNode(String? indexId) => activeNode = indexId;

  @override
  bool monitorEnabled() => true;

  @override
  m.StatsSnapshotDto statsSnapshot() => m.StatsSnapshotDto(
    ok: true,
    enabled: true,
    displaySpeed: true,
    generation: BigInt.zero,
    proxyUp: BigInt.zero,
    proxyDown: BigInt.zero,
    directUp: BigInt.zero,
    directDown: BigInt.zero,
    nodes: statsNodes,
    error: statsError,
  );

  @override
  bool clearStats() {
    clearStatsCount++;
    return true;
  }

  @override
  Stream<m.TrafficBatchDto> subscribeTraffic() {
    subscribeTrafficCount++;
    return _traffic.stream;
  }

  @override
  m.LogPageDto getLogs(int offset, int limit) => m.LogPageDto(
    ok: true,
    lines: logs,
    total: logs.length,
    droppedLines: BigInt.zero,
    droppedBytes: BigInt.zero,
    truncatedLines: BigInt.zero,
    collectingPaused: false,
    scrollPaused: false,
  );

  @override
  bool clearLogs() {
    clearLogsCount++;
    logs = <m.LogLineDto>[];
    return true;
  }

  @override
  void setLogFilter(int minLevel, List<String> include, List<String> exclude) {
    setLogFilterCalls++;
    lastMinLevel = minLevel;
  }

  @override
  void setLogPause({
    required bool collectingPaused,
    required bool scrollPaused,
  }) {
    lastCollectingPaused = collectingPaused;
    lastScrollPaused = scrollPaused;
  }

  @override
  Stream<m.LogBatchDto> subscribeLogs() {
    subscribeLogsCount++;
    return _logController.stream;
  }

  @override
  m.PageVisibilityDto setPageVisible(String page, bool visible) {
    pageVisibility[page] = visible;
    return m.PageVisibilityDto(
      visible: visible,
      subscribed: visible,
      refreshIntervalMs: 2000,
    );
  }

  @override
  bool clashSupported() => clashApiSupported;

  @override
  void setDelayUrl(String? url) => delayUrl = url;

  @override
  Future<m.ClashModeDto> clashModeState() async {
    if (!clashApiSupported) {
      return m.ClashModeDto(
        ok: false,
        supported: false,
        message: clashMessage,
        mode: null,
        modes: const <String>[],
      );
    }
    return m.ClashModeDto(
      ok: true,
      supported: true,
      message: null,
      mode: clashMode,
      modes: clashModes,
    );
  }

  @override
  Future<m.MonitorActionResult> updateClashMode(String mode) async {
    if (modeFails) {
      return m.MonitorActionResult(ok: false, supported: true, message: null);
    }
    clashMode = mode;
    return _ok();
  }

  @override
  Future<m.ClashProxiesDto> clashProxies() async {
    clashProxiesCount++;
    final gate = proxiesGate;
    if (gate != null) await gate;
    if (!clashApiSupported) {
      return m.ClashProxiesDto(
        ok: false,
        supported: false,
        message: clashMessage,
        epoch: BigInt.zero,
        seq: BigInt.zero,
        items: const <m.ClashProxyDto>[],
        error: null,
      );
    }
    return m.ClashProxiesDto(
      ok: true,
      supported: true,
      message: clashMessage,
      epoch: BigInt.one,
      seq: BigInt.one,
      items: proxies,
      error: null,
    );
  }

  @override
  Future<m.MonitorActionResult> selectClashProxy(
    String group,
    String name,
  ) async {
    selectedProxies.add('$group/$name');
    final updated = proxies
        .map(
          (p) => p.name == group
              ? m.ClashProxyDto(
                  name: p.name,
                  proxyType: p.proxyType,
                  isGroup: p.isGroup,
                  now: name,
                  all: p.all,
                  delay: p.delay,
                  provider: p.provider,
                )
              : p,
        )
        .toList();
    proxies = updated;
    return _ok();
  }

  @override
  Future<m.DelayResultDto> clashProxyDelay(String name) async {
    testedProxies.add(name);
    return m.DelayResultDto(name: name, delay: proxyDelayValue);
  }

  @override
  Future<m.GroupDelayDto> clashGroupDelay(String group) async {
    testedGroups.add(group);
    final target = proxies.firstWhere(
      (p) => p.name == group,
      orElse: () => const m.ClashProxyDto(
        name: '',
        proxyType: '',
        isGroup: false,
        now: null,
        all: <String>[],
        delay: -1,
      ),
    );
    return m.GroupDelayDto(
      ok: true,
      supported: true,
      group: group,
      items: target.all
          .map((n) => m.DelayResultDto(name: n, delay: proxyDelayValue))
          .toList(),
      error: null,
    );
  }

  @override
  Future<m.ClashConnectionsDto> clashConnections() async {
    clashConnectionsCount++;
    final gate = connectionsGate;
    if (gate != null) await gate;
    if (!clashApiSupported) {
      return m.ClashConnectionsDto(
        ok: false,
        supported: false,
        message: clashMessage,
        uploadTotal: BigInt.zero,
        downloadTotal: BigInt.zero,
        items: const <m.ClashConnectionDto>[],
        error: null,
      );
    }
    return m.ClashConnectionsDto(
      ok: true,
      supported: true,
      message: null,
      uploadTotal: BigInt.from(100),
      downloadTotal: BigInt.from(200),
      items: connections,
      error: null,
    );
  }

  @override
  Future<m.MonitorActionResult> closeClashConnection(String id) async {
    closedConnections.add(id);
    connections = connections.where((c) => c.id != id).toList();
    return _ok();
  }

  @override
  Future<m.MonitorActionResult> closeAllClashConnections() async {
    closeAllCount++;
    if (closeAllFails) {
      return m.MonitorActionResult(ok: false, supported: true, message: null);
    }
    connections = <m.ClashConnectionDto>[];
    return _ok();
  }

  @override
  void startPolling() => pollingStarted = true;

  @override
  void syncSession() {
    syncSessionCount++;
    pollingStarted = true;
  }

  m.MonitorActionResult _ok() => m.MonitorActionResult(
    ok: true,
    supported: true,
    message: null,
    error: null,
  );

  void disposeStreams() {
    _traffic.close();
    _logController.close();
  }
}
