import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

/// Consumer view of `ClashUIItem` (upstream `ConfigItems.ClashUIItem`) for the
/// Clash monitor tabs.
///
/// The frozen upstream `OptionSettingWindow` exposes no Clash controls, so the
/// values are edited from the settings window's "历史保留" section and read
/// here. Semantics mirror `ClashProxiesViewModel` / `ClashConnectionsViewModel`:
/// a 1s poller gated by `AutoRefresh` and `% RefreshInterval` (non-positive
/// interval disables refresh), and `SortingSelected` picking delay (0) or name
/// (1) ordering.
class ClashUiConfig {
  const ClashUiConfig({
    this.proxiesAutoRefresh = false,
    this.proxiesRefreshInterval = 2,
    this.proxiesSorting = 0,
    this.connectionsAutoRefresh = false,
    this.connectionsRefreshInterval = 2,
    this.connectionsColumns = const <Map<String, dynamic>>[],
  });

  final bool proxiesAutoRefresh;

  /// Seconds between automatic proxy refreshes; `<= 0` disables it.
  final int proxiesRefreshInterval;

  /// `0` = sort by delay, `1` = sort by name; other values keep the core order.
  final int proxiesSorting;

  final bool connectionsAutoRefresh;

  /// Seconds between automatic connection refreshes; `<= 0` disables it.
  final int connectionsRefreshInterval;

  /// Raw persisted `ConnectionsColumnItem` rows (Name/Width/Index). The view
  /// normalizes them with `resolveVisibleColumns`; empty means upstream
  /// defaults (upstream `RestoreUI` early-returns on a missing list).
  final List<Map<String, dynamic>> connectionsColumns;

  bool get proxiesRefreshEnabled =>
      proxiesAutoRefresh && proxiesRefreshInterval > 0;

  bool get connectionsRefreshEnabled =>
      connectionsAutoRefresh && connectionsRefreshInterval > 0;

  Duration get proxiesRefreshPeriod =>
      Duration(seconds: proxiesRefreshInterval);

  Duration get connectionsRefreshPeriod =>
      Duration(seconds: connectionsRefreshInterval);
}

/// Wave B (FLD-CFG-131..135) shared poll wiring: the monitor tabs must drive
/// their timers from this canonical period, never from an unpersisted toggle.
/// A null return means the poller stays stopped (auto-refresh off or a
/// non-positive interval, upstream parity).
Duration? clashPollPeriod({
  required bool autoRefresh,
  required int intervalSeconds,
}) {
  if (!autoRefresh || intervalSeconds <= 0) return null;
  return Duration(seconds: intervalSeconds);
}

const ClashUiConfig defaultClashUiConfig = ClashUiConfig();

/// Parse the persisted settings document into a [ClashUiConfig]. Missing keys
/// fall back to the same defaults as `domain::settings::ClashUiItem`.
ClashUiConfig clashUiConfigFromDocument(Map<String, dynamic> document) {
  final raw = document['ClashUIItem'];
  final group = raw is Map ? raw : const <String, dynamic>{};
  int asInt(Object? value, int fallback) =>
      value is num ? value.toInt() : fallback;
  bool asBool(Object? value, bool fallback) => value is bool ? value : fallback;
  return ClashUiConfig(
    proxiesAutoRefresh: asBool(group['ProxiesAutoRefresh'], false),
    proxiesRefreshInterval: asInt(group['ProxiesRefreshInterval'], 2),
    proxiesSorting: asInt(group['ProxiesSorting'], 0),
    connectionsAutoRefresh: asBool(group['ConnectionsAutoRefresh'], false),
    connectionsRefreshInterval: asInt(group['ConnectionsRefreshInterval'], 2),
    connectionsColumns: asColumnRows(group['ConnectionsColumnItem']),
  );
}

/// Copy the persisted `ConnectionsColumnItem` rows (Name/Width/Index) out of
/// the document; anything else falls back to an empty list (upstream defaults).
List<Map<String, dynamic>> asColumnRows(Object? value) {
  if (value is! List) return const <Map<String, dynamic>>[];
  final rows = <Map<String, dynamic>>[];
  for (final row in value) {
    if (row is Map<String, dynamic>) {
      rows.add(Map<String, dynamic>.of(row));
    } else if (row is Map) {
      rows.add(Map<String, dynamic>.from(row));
    }
  }
  return rows;
}

/// Return a copy of the document's `ClashUIItem` group with [changes] applied,
/// preserving unknown keys. Used when a monitor tab writes a toggle back so the
/// value survives the next reopen (upstream `WhenAnyValue(...).Subscribe`).
Map<String, dynamic> clashUiGroupWith(
  Map<String, dynamic> document,
  Map<String, Object> changes,
) {
  final current = document['ClashUIItem'];
  final group = current is Map<String, dynamic>
      ? Map<String, dynamic>.of(current)
      : <String, dynamic>{};
  group.addAll(changes);
  return group;
}

final clashUiConfigProvider = Provider<ClashUiConfig>((ref) {
  final document = ref.watch(settingsControllerProvider).document;
  if (document.isEmpty) return defaultClashUiConfig;
  return clashUiConfigFromDocument(document);
});
