import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as contract;
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as rust;

/// Thin, testable seam over the T15a generated monitor bridge.
///
/// The application always uses [FrbMonitorBridge]; widget tests override
/// [monitorBridgeProvider] with an in-memory fake so no native library or
/// socket is touched. Every read returns the Rust DTOs unchanged.
abstract class MonitorBridge {
  void configure({
    required int core,
    required int statePort,
    required int statePort2,
    String? secret,
    required bool enableStatistics,
    required bool displayRealTimeSpeed,
    required int refreshIntervalMs,
  });

  void setActiveNode(String? indexId);

  bool monitorEnabled();

  rust.StatsSnapshotDto statsSnapshot();

  bool clearStats();

  Stream<rust.TrafficBatchDto> subscribeTraffic();

  rust.LogPageDto getLogs(int offset, int limit);

  bool clearLogs();

  void setLogFilter(int minLevel, List<String> include, List<String> exclude);

  void setLogPause({
    required bool collectingPaused,
    required bool scrollPaused,
  });

  Stream<rust.LogBatchDto> subscribeLogs();

  rust.PageVisibilityDto setPageVisible(String page, bool visible);

  bool clashSupported();

  Future<rust.ClashProxiesDto> clashProxies();

  Future<rust.MonitorActionResult> selectClashProxy(String group, String name);

  Future<rust.DelayResultDto> clashProxyDelay(String name);

  Future<rust.GroupDelayDto> clashGroupDelay(String group);

  Future<rust.ClashConnectionsDto> clashConnections();

  Future<rust.MonitorActionResult> closeClashConnection(String id);

  Future<rust.MonitorActionResult> closeAllClashConnections();

  void startPolling();
}

class FrbMonitorBridge implements MonitorBridge {
  const FrbMonitorBridge();

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
    rust.monitorConfigure(
      core: core,
      statePort: statePort,
      statePort2: statePort2,
      secret: secret,
      enableStatistics: enableStatistics,
      displayRealTimeSpeed: displayRealTimeSpeed,
      refreshIntervalMs: refreshIntervalMs,
    );
  }

  @override
  void setActiveNode(String? indexId) =>
      rust.monitorSetActiveNode(indexId: indexId);

  @override
  bool monitorEnabled() => rust.monitorEnabled();

  @override
  rust.StatsSnapshotDto statsSnapshot() => rust.statsSnapshot();

  @override
  bool clearStats() => rust.clearStats().ok;

  @override
  Stream<rust.TrafficBatchDto> subscribeTraffic() => rust.subscribeTraffic();

  @override
  rust.LogPageDto getLogs(int offset, int limit) =>
      rust.getLogs(offset: offset, limit: limit);

  @override
  bool clearLogs() => rust.clearLogs().ok;

  @override
  void setLogFilter(int minLevel, List<String> include, List<String> exclude) =>
      rust.setLogFilter(minLevel: minLevel, include: include, exclude: exclude);

  @override
  void setLogPause({
    required bool collectingPaused,
    required bool scrollPaused,
  }) => rust.setLogPause(
    collectingPaused: collectingPaused,
    scrollPaused: scrollPaused,
  );

  @override
  Stream<rust.LogBatchDto> subscribeLogs() => rust.subscribeLogs();

  @override
  rust.PageVisibilityDto setPageVisible(String page, bool visible) =>
      rust.setPageVisible(page: page, visible: visible);

  @override
  bool clashSupported() => rust.clashSupported();

  @override
  Future<rust.ClashProxiesDto> clashProxies() => rust.clashProxies();

  @override
  Future<rust.MonitorActionResult> selectClashProxy(
    String group,
    String name,
  ) => rust.selectClashProxy(group: group, name: name);

  @override
  Future<rust.DelayResultDto> clashProxyDelay(String name) =>
      rust.clashProxyDelay(name: name);

  @override
  Future<rust.GroupDelayDto> clashGroupDelay(String group) =>
      rust.clashGroupDelay(group: group);

  @override
  Future<rust.ClashConnectionsDto> clashConnections() =>
      rust.clashConnections();

  @override
  Future<rust.MonitorActionResult> closeClashConnection(String id) =>
      rust.closeClashConnection(id: id);

  @override
  Future<rust.MonitorActionResult> closeAllClashConnections() =>
      rust.closeAllClashConnections();

  @override
  void startPolling() => rust.monitorStartPolling();
}

final monitorBridgeProvider = Provider<MonitorBridge>(
  (ref) => const FrbMonitorBridge(),
);

/// Convenience access to the structured error on a monitor DTO.
String? monitorErrorText(contract.ErrorDto? error) {
  if (error == null) return null;
  final detail = error.detail;
  return detail == null || detail.isEmpty
      ? '${error.code} (${error.messageKey})'
      : '${error.code} (${error.messageKey}): $detail';
}
