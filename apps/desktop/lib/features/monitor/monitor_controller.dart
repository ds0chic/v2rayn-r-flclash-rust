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
    this.autoRefresh = true,
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
  final bool autoRefresh;
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
  ///
  /// Filtering is presentation-only: it never pauses collection and never
  /// changes the Rust buffer. The keyword follows upstream `MsgFilter` (regex);
  /// an invalid pattern falls back to a case-insensitive substring match.
  List<m.LogLineDto> get visibleLogs {
    final pattern = keyword.trim();
    if (pattern.isEmpty) {
      return logs.where((line) => line.level >= minLevel).toList();
    }
    final regex = _compileKeyword(pattern);
    return logs.where((line) {
      if (line.level < minLevel) return false;
      if (regex != null) return regex.hasMatch(line.text);
      return line.text.toLowerCase().contains(pattern.toLowerCase());
    }).toList();
  }

  /// Today's upload/download aggregated from the live per-node `ServerStatItem`
  /// rows (upstream `StatisticsManager` today semantics). These are the
  /// per-node today counters the Rust monitor resets on the date rollover, not
  /// the session-cumulative `proxyUp`/`proxyDown` rates.
  BigInt get todayUp => _sumNodeToday((node) => node.todayUp);
  BigInt get todayDown => _sumNodeToday((node) => node.todayDown);

  /// Whether any per-node today rows are available. When false the status bar
  /// shows `--` rather than a fabricated zero (no applied session, or
  /// statistics disabled/still unbound).
  bool get hasTodayNodes => nodes.isNotEmpty;

  BigInt _sumNodeToday(Object? Function(m.NodeTrafficDto) pick) {
    var total = BigInt.zero;
    for (final node in nodes) {
      total += _platformBytes(pick(node));
    }
    return total;
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
    bool? autoRefresh,
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
      autoRefresh: autoRefresh ?? this.autoRefresh,
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

RegExp? _compileKeyword(String pattern) {
  try {
    return RegExp(pattern);
  } on FormatException {
    return null;
  }
}

/// Normalize a `PlatformInt64` wire counter to a non-negative [BigInt] (int on
/// native, BigInt on web; negative/unknown values clamp to zero).
BigInt _platformBytes(Object? value) {
  if (value is BigInt) return value.isNegative ? BigInt.zero : value;
  if (value is int) return value > 0 ? BigInt.from(value) : BigInt.zero;
  if (value is num) {
    final v = value.toInt();
    return v > 0 ? BigInt.from(v) : BigInt.zero;
  }
  return BigInt.zero;
}

/// Applied-session facts that justify (re)binding the Rust monitor source.
///
/// A `RuntimeView` that only advanced its event sequence (heartbeat) or carried
/// a log line keeps the same signature, so the poller never rebuilds its source
/// for unrelated churn. `state`/`ports`/`sessionId` cover the applied core,
/// endpoint and stop/switch; statistics toggles are applied inside the Rust
/// `sync_from_engine_session` signature.
String monitorSessionSignature(RuntimeView view) => <String>[
  view.state,
  view.sessionId ?? '',
  view.ports.join(','),
  view.appliedRevision?.toString() ?? '',
].join('|');

final monitorControllerProvider =
    NotifierProvider<MonitorController, MonitorState>(MonitorController.new);

class MonitorController extends Notifier<MonitorState> {
  StreamSubscription<m.TrafficBatchDto>? _trafficSub;
  StreamSubscription<m.LogBatchDto>? _logSub;
  final Set<String> _visiblePages = <String>{};
  bool _logsPageVisible = false;
  String? _lastSessionSig;

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
  ///
  /// Only a real applied-session change re-syncs: an unrelated `RuntimeView`
  /// update (heartbeat sequence, log line, error text) leaves the signature
  /// unchanged and never triggers `monitorStartPolling`, so the Rust poller
  /// keeps its source instead of rebuilding (and resetting) it.
  void syncRuntimeSession(RuntimeView view) {
    final signature = monitorSessionSignature(view);
    if (signature == _lastSessionSig) return;
    _lastSessionSig = signature;
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
    // A paused collection must not surface the raw lines the Rust fan-out may
    // still carry (RT-19): the read model only appends when collection is live.
    // Hidden pages and a manual auto-refresh pause also freeze the view while
    // the Rust ring keeps accumulating, so a later reload shows the tail.
    final append =
        !batch.collectingPaused && _logsPageVisible && state.autoRefresh;
    state = state.copyWith(
      logs: append
          ? _trimLogs(<m.LogLineDto>[...state.logs, ...batch.lines])
          : state.logs,
      droppedLines: batch.droppedLines,
      truncatedLines: batch.truncatedLines,
      collectingPaused: batch.collectingPaused,
      scrollPaused: batch.scrollPaused,
    );
  }

  static List<m.LogLineDto> _trimLogs(List<m.LogLineDto> merged) =>
      merged.length > maxDisplayedLogs
      ? merged.sublist(merged.length - maxDisplayedLogs)
      : merged;

  /// Reload the newest retained lines from the Rust ring (called when the page
  /// or auto-refresh resumes; the subscription stays idle until then).
  void _reloadLogs() {
    final probe = _bridge.getLogs(0, 0);
    final offset = probe.total > maxDisplayedLogs
        ? probe.total - maxDisplayedLogs
        : 0;
    final page = _bridge.getLogs(offset, maxDisplayedLogs);
    state = state.copyWith(
      logs: page.lines,
      logTotal: page.total,
      droppedLines: page.droppedLines,
      truncatedLines: page.truncatedLines,
      collectingPaused: page.collectingPaused,
      scrollPaused: page.scrollPaused,
    );
  }

  /// Toggle a page's visibility; the Rust side keeps collecting while hidden,
  /// only the page-local UI refresh is frozen.
  void setPageVisible(String page, bool visible) {
    if (visible) {
      _visiblePages.add(page);
    } else {
      _visiblePages.remove(page);
    }
    if (page == 'logs') _logsPageVisible = visible;
    _bridge.setPageVisible(page, visible);
    if (!visible) return;
    _ensureStreams();
    if (page == 'logs') _reloadLogs();
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

  /// Level filtering is presentation-only; the Rust buffer keeps every line so
  /// lowering the level again reveals the retained history unchanged.
  void setMinLevel(int level) => state = state.copyWith(minLevel: level);

  void setKeyword(String keyword) => state = state.copyWith(keyword: keyword);

  /// Toggle the presentation auto-refresh. When resumed, the frozen view is
  /// resynced from the Rust tail; collection is never affected.
  void setAutoRefresh(bool enabled) {
    state = state.copyWith(autoRefresh: enabled);
    if (enabled && _logsPageVisible) _reloadLogs();
  }

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

  /// `ClearMsg`: clear the Rust ring and show the upstream clear marker line.
  void clearLogs() {
    _bridge.clearLogs();
    state = state.copyWith(
      logs: const <m.LogLineDto>[
        m.LogLineDto(
          text: '----- Message cleared -----',
          level: 2,
          truncated: false,
        ),
      ],
      logTotal: 0,
    );
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
      // A failed ServerStatItem store bind/load is visible here (and retried on
      // the next applied-session change) instead of being silently dropped.
      error: snap.error?.code,
      clearError: snap.error == null,
    );
  }
}
