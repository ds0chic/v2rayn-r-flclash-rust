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
  });

  final bool proxiesAutoRefresh;

  /// Seconds between automatic proxy refreshes; `<= 0` disables it.
  final int proxiesRefreshInterval;

  /// `0` = sort by delay, `1` = sort by name; other values keep the core order.
  final int proxiesSorting;

  final bool connectionsAutoRefresh;

  /// Seconds between automatic connection refreshes; `<= 0` disables it.
  final int connectionsRefreshInterval;

  bool get proxiesRefreshEnabled =>
      proxiesAutoRefresh && proxiesRefreshInterval > 0;

  bool get connectionsRefreshEnabled =>
      connectionsAutoRefresh && connectionsRefreshInterval > 0;

  Duration get proxiesRefreshPeriod =>
      Duration(seconds: proxiesRefreshInterval);

  Duration get connectionsRefreshPeriod =>
      Duration(seconds: connectionsRefreshInterval);
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
  );
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
