import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';

/// Live monitor read model: traffic counters, log lines, Clash proxies and
/// connections. Every value comes from the bridge; nothing is fabricated.
class MonitorState {
  MonitorState({
    this.configured = false,
    BigInt? proxyUp,
    BigInt? proxyDown,
    BigInt? directUp,
    BigInt? directDown,
    BigInt? proxyUpBps,
    BigInt? proxyDownBps,
    BigInt? directUpBps,
    BigInt? directDownBps,
    BigInt? generation,
    this.hasTraffic = false,
    this.logs = const <m.LogLineDto>[],
    this.logTotal = 0,
    BigInt? droppedLines,
    BigInt? truncatedLines,
    this.collectingPaused = false,
    this.scrollPaused = false,
    this.minLevel = 0,
    this.keyword = '',
    this.clashSupported = false,
    this.proxies = const <m.ClashProxyDto>[],
    this.proxyDelays = const <String, int>{},
    this.proxiesMessage,
    this.clashMode,
    this.clashModes = const <String>[],
    this.modeMessage,
    this.connections = const <m.ClashConnectionDto>[],
    this.nodes = const <m.NodeTrafficDto>[],
    BigInt? connectionsUpload,
    BigInt? connectionsDownload,
    this.connectionsMessage,
    this.error,
  }) : proxyUp = proxyUp ?? BigInt.zero,
       proxyDown = proxyDown ?? BigInt.zero,
       directUp = directUp ?? BigInt.zero,
       directDown = directDown ?? BigInt.zero,
       proxyUpBps = proxyUpBps ?? BigInt.zero,
       proxyDownBps = proxyDownBps ?? BigInt.zero,
       directUpBps = directUpBps ?? BigInt.zero,
       directDownBps = directDownBps ?? BigInt.zero,
       generation = generation ?? BigInt.zero,
       droppedLines = droppedLines ?? BigInt.zero,
       truncatedLines = truncatedLines ?? BigInt.zero,
       connectionsUpload = connectionsUpload ?? BigInt.zero,
       connectionsDownload = connectionsDownload ?? BigInt.zero;

  final bool configured;
  final BigInt proxyUp;
  final BigInt proxyDown;
  final BigInt directUp;
  final BigInt directDown;
  final BigInt proxyUpBps;
  final BigInt proxyDownBps;
  final BigInt directUpBps;
  final BigInt directDownBps;
  final BigInt generation;
  final bool hasTraffic;
  final List<m.LogLineDto> logs;
  final int logTotal;
  final BigInt droppedLines;
  final BigInt truncatedLines;
  final bool collectingPaused;
  final bool scrollPaused;
  final int minLevel;
  final String keyword;
  final bool clashSupported;
  final List<m.ClashProxyDto> proxies;
  final Map<String, int> proxyDelays;
  final String? proxiesMessage;

  /// Live Clash routing mode (`Rule`/`Global`/`Direct`) and the selectable set.
  final String? clashMode;
  final List<String> clashModes;
  final String? modeMessage;
  final List<m.ClashConnectionDto> connections;

  /// Latest per-node `ServerStatItem` rows reported by the active session.
  final List<m.NodeTrafficDto> nodes;
  final BigInt connectionsUpload;
  final BigInt connectionsDownload;
  final String? connectionsMessage;
  final String? error;

  /// Logs that pass the current level + keyword filter.
  List<m.LogLineDto> get visibleLogs {
    final needle = keyword.trim().toLowerCase();
    return logs.where((line) {
      if (line.level < minLevel) return false;
      if (needle.isNotEmpty && !line.text.toLowerCase().contains(needle)) {
        return false;
      }
      return true;
    }).toList();
  }

  MonitorState copyWith({
    bool? configured,
    BigInt? proxyUp,
    BigInt? proxyDown,
    BigInt? directUp,
    BigInt? directDown,
    BigInt? proxyUpBps,
    BigInt? proxyDownBps,
    BigInt? directUpBps,
    BigInt? directDownBps,
    BigInt? generation,
    bool? hasTraffic,
    List<m.LogLineDto>? logs,
    int? logTotal,
    BigInt? droppedLines,
    BigInt? truncatedLines,
    bool? collectingPaused,
    bool? scrollPaused,
    int? minLevel,
    String? keyword,
    bool? clashSupported,
    List<m.ClashProxyDto>? proxies,
    Map<String, int>? proxyDelays,
    String? proxiesMessage,
    bool clearProxiesMessage = false,
    String? clashMode,
    bool clearClashMode = false,
    List<String>? clashModes,
    String? modeMessage,
    bool clearModeMessage = false,
    List<m.ClashConnectionDto>? connections,
    List<m.NodeTrafficDto>? nodes,
    BigInt? connectionsUpload,
    BigInt? connectionsDownload,
    String? connectionsMessage,
    bool clearConnectionsMessage = false,
    String? error,
    bool clearError = false,
  }) {
    return MonitorState(
      configured: configured ?? this.configured,
      proxyUp: proxyUp ?? this.proxyUp,
      proxyDown: proxyDown ?? this.proxyDown,
      directUp: directUp ?? this.directUp,
      directDown: directDown ?? this.directDown,
      proxyUpBps: proxyUpBps ?? this.proxyUpBps,
      proxyDownBps: proxyDownBps ?? this.proxyDownBps,
      directUpBps: directUpBps ?? this.directUpBps,
      directDownBps: directDownBps ?? this.directDownBps,
      generation: generation ?? this.generation,
      hasTraffic: hasTraffic ?? this.hasTraffic,
      logs: logs ?? this.logs,
      logTotal: logTotal ?? this.logTotal,
      droppedLines: droppedLines ?? this.droppedLines,
      truncatedLines: truncatedLines ?? this.truncatedLines,
      collectingPaused: collectingPaused ?? this.collectingPaused,
      scrollPaused: scrollPaused ?? this.scrollPaused,
      minLevel: minLevel ?? this.minLevel,
      keyword: keyword ?? this.keyword,
      clashSupported: clashSupported ?? this.clashSupported,
      proxies: proxies ?? this.proxies,
      proxyDelays: proxyDelays ?? this.proxyDelays,
      proxiesMessage: clearProxiesMessage
          ? null
          : (proxiesMessage ?? this.proxiesMessage),
      clashMode: clearClashMode ? null : (clashMode ?? this.clashMode),
      clashModes: clashModes ?? this.clashModes,
      modeMessage: clearModeMessage ? null : (modeMessage ?? this.modeMessage),
      connections: connections ?? this.connections,
      nodes: nodes ?? this.nodes,
      connectionsUpload: connectionsUpload ?? this.connectionsUpload,
      connectionsDownload: connectionsDownload ?? this.connectionsDownload,
      connectionsMessage: clearConnectionsMessage
          ? null
          : (connectionsMessage ?? this.connectionsMessage),
      error: clearError ? null : (error ?? this.error),
    );
  }
}

/// Maximum log lines retained in the Dart read model. The Rust ring is the
/// authoritative buffer; this only bounds widget memory.
const int maxDisplayedLogs = 2000;

final monitorControllerProvider =
    NotifierProvider<MonitorController, MonitorState>(MonitorController.new);

class MonitorController extends Notifier<MonitorState> {
  StreamSubscription<m.TrafficBatchDto>? _trafficSub;
  StreamSubscription<m.LogBatchDto>? _logSub;
  final Set<String> _visiblePages = <String>{};

  MonitorBridge get _bridge => ref.read(monitorBridgeProvider);

  @override
  MonitorState build() {
    ref.onDispose(() {
      _trafficSub?.cancel();
      _logSub?.cancel();
    });
    // Normal user entry for statistics: mirror the applied runtime session into
    // the Rust monitor. A core apply pushes the real core / statistics ports /
    // active node and starts polling; a stop or core switch clears the old
    // session's ports so its collection stops. Page visibility never reaches
    // this path, so hiding a page can only pause page-local UI refresh, never
    // the active session's collection or persistence.
    ref.listen<RuntimeView>(runtimeControllerProvider, (_, next) {
      syncRuntimeSession(next);
    });
    return MonitorState();
  }

  /// Push the applied-session facts into the Rust monitor and, when a session
  /// is actually applied, load the current snapshot and subscribe to streams.
  void syncRuntimeSession(RuntimeView view) {
    _bridge.syncSession();
    if (!view.hasAppliedEndpoint) return;
    refreshStats();
    _ensureStreams();
    state = state.copyWith(
      configured: true,
      clashSupported: _bridge.clashSupported(),
    );
  }

  /// Configure the Rust monitor for the running session. A no-op second call
  /// simply re-applies the facts.
  void configure({
    required int core,
    required int statePort,
    required int statePort2,
    String? secret,
    required bool enableStatistics,
    required bool displayRealTimeSpeed,
    required int refreshIntervalMs,
  }) {
    _bridge.configure(
      core: core,
      statePort: statePort,
      statePort2: statePort2,
      secret: secret,
      enableStatistics: enableStatistics,
      displayRealTimeSpeed: displayRealTimeSpeed,
      refreshIntervalMs: refreshIntervalMs,
    );
    state = state.copyWith(
      configured: true,
      clashSupported: _bridge.clashSupported(),
    );
    _ensureStreams();
  }

  void setActiveNode(String? indexId) => _bridge.setActiveNode(indexId);

  void _ensureStreams() {
    _trafficSub ??= _bridge.subscribeTraffic().listen(_onTraffic);
    _logSub ??= _bridge.subscribeLogs().listen(_onLogs);
  }

  void _onTraffic(m.TrafficBatchDto batch) {
    state = state.copyWith(
      hasTraffic: true,
      generation: batch.generation,
      proxyUp: batch.proxyUp,
      proxyDown: batch.proxyDown,
      directUp: batch.directUp,
      directDown: batch.directDown,
      proxyUpBps: batch.proxyUpBps,
      proxyDownBps: batch.proxyDownBps,
      directUpBps: batch.directUpBps,
      directDownBps: batch.directDownBps,
      nodes: batch.nodes,
    );
  }

  void _onLogs(m.LogBatchDto batch) {
    final merged = <m.LogLineDto>[...state.logs, ...batch.lines];
    final trimmed = merged.length > maxDisplayedLogs
        ? merged.sublist(merged.length - maxDisplayedLogs)
        : merged;
    state = state.copyWith(
      logs: trimmed,
      droppedLines: batch.droppedLines,
      truncatedLines: batch.truncatedLines,
      collectingPaused: batch.collectingPaused,
      scrollPaused: batch.scrollPaused,
    );
  }

  /// Toggle a page's visibility; streams stay subscribed only while visible.
  void setPageVisible(String page, bool visible) {
    if (visible) {
      _visiblePages.add(page);
    } else {
      _visiblePages.remove(page);
    }
    _bridge.setPageVisible(page, visible);
    if (visible) _ensureStreams();
  }

  Future<void> refreshProxies() async {
    final result = await _bridge.clashProxies();
    state = state.copyWith(
      clashSupported: result.supported,
      proxies: result.items,
      proxiesMessage: result.supported
          ? null
          : (result.message ?? '当前内核不提供 Clash API'),
      clearProxiesMessage: result.supported && result.message == null,
      error: result.error?.code,
      clearError: result.error == null,
    );
  }

  Future<void> refreshConnections() async {
    final result = await _bridge.clashConnections();
    state = state.copyWith(
      clashSupported: result.supported,
      connections: result.items,
      connectionsUpload: result.uploadTotal,
      connectionsDownload: result.downloadTotal,
      connectionsMessage: result.supported
          ? null
          : (result.message ?? '当前内核不提供 Clash API'),
      clearConnectionsMessage: result.supported && result.message == null,
      error: result.error?.code,
      clearError: result.error == null,
    );
  }

  Future<void> refreshClashMode() async {
    final result = await _bridge.clashModeState();
    state = state.copyWith(
      clashSupported: result.supported,
      clashMode: result.mode,
      clashModes: result.modes,
      modeMessage: result.supported
          ? (result.ok ? null : monitorErrorText(result.error))
          : (result.message ?? '当前内核不提供 Clash API'),
      clearModeMessage: result.supported && result.ok,
      clearClashMode: !result.ok,
      error: result.error?.code,
      clearError: result.error == null,
    );
  }

  /// Switch the Clash routing mode; refreshes the live value on success.
  Future<bool> setClashMode(String mode) async {
    final result = await _bridge.updateClashMode(mode);
    if (result.ok) {
      await refreshClashMode();
    } else {
      state = state.copyWith(
        modeMessage: result.supported
            ? (monitorErrorText(result.error) ??
                  result.message ??
                  'Clash 模式切换失败')
            : (result.message ?? '当前内核不提供 Clash API'),
        error: result.error?.code,
        clearError: result.error == null,
      );
    }
    return result.ok;
  }

  /// Push the settings delay-probe URL into Rust (null restores default).
  void setDelayUrl(String? url) => _bridge.setDelayUrl(url);

  Future<bool> selectProxy(String group, String name) async {
    final result = await _bridge.selectClashProxy(group, name);
    if (result.ok) await refreshProxies();
    return result.ok;
  }

  Future<int> testProxy(String name) async {
    final result = await _bridge.clashProxyDelay(name);
    final delays = Map<String, int>.of(state.proxyDelays)
      ..[result.name] = result.delay;
    state = state.copyWith(proxyDelays: delays);
    return result.delay;
  }

  Future<void> testGroup(String group) async {
    final result = await _bridge.clashGroupDelay(group);
    final delays = Map<String, int>.of(state.proxyDelays);
    for (final item in result.items) {
      delays[item.name] = item.delay;
    }
    state = state.copyWith(proxyDelays: delays);
  }

  Future<bool> closeConnection(String id) async {
    final result = await _bridge.closeClashConnection(id);
    if (result.ok) await refreshConnections();
    return result.ok;
  }

  Future<bool> closeAllConnections() async {
    final result = await _bridge.closeAllClashConnections();
    if (result.ok) await refreshConnections();
    return result.ok;
  }

  void setMinLevel(int level) {
    state = state.copyWith(minLevel: level);
    _bridge.setLogFilter(level, const <String>[], const <String>[]);
  }

  void setKeyword(String keyword) => state = state.copyWith(keyword: keyword);

  void setCollectingPaused(bool paused) {
    _bridge.setLogPause(
      collectingPaused: paused,
      scrollPaused: state.scrollPaused,
    );
    state = state.copyWith(collectingPaused: paused);
  }

  void setScrollPaused(bool paused) {
    _bridge.setLogPause(
      collectingPaused: state.collectingPaused,
      scrollPaused: paused,
    );
    state = state.copyWith(scrollPaused: paused);
  }

  void clearLogs() {
    _bridge.clearLogs();
    state = state.copyWith(logs: const <m.LogLineDto>[], logTotal: 0);
  }

  bool clearStats() {
    final ok = _bridge.clearStats();
    if (ok) {
      state = state.copyWith(
        proxyUp: BigInt.zero,
        proxyDown: BigInt.zero,
        directUp: BigInt.zero,
        directDown: BigInt.zero,
        proxyUpBps: BigInt.zero,
        proxyDownBps: BigInt.zero,
        directUpBps: BigInt.zero,
        directDownBps: BigInt.zero,
      );
    }
    return ok;
  }

  void startPolling() => _bridge.startPolling();

  /// Test-only synchronous refresh of the stats snapshot.
  void refreshStats() {
    final snap = _bridge.statsSnapshot();
    state = state.copyWith(
      proxyUp: snap.proxyUp,
      proxyDown: snap.proxyDown,
      directUp: snap.directUp,
      directDown: snap.directDown,
      generation: snap.generation,
      nodes: snap.nodes,
      configured: true,
    );
  }
}
