/// R4-34: the pure resource / background-task contract.
///
/// Upstream `TaskManager.UpdateTaskRunGeo` is driven by the global
/// `GuiItem.AutoUpdateInterval` (hours, `0` disables) inside the same periodic
/// loop that updates subscriptions: every hour tick it runs a Geo/SRS download
/// pass when the elapsed hours is a positive exact multiple of the interval.
/// This module is the UI-side contract that the persisted resource sources and
/// the global period are actually schedulable; the Rust engine performs the
/// real download behind the existing subscription-scheduler start, so no new
/// bridge surface is required.
library;

class ResourceAutoUpdatePlan {
  const ResourceAutoUpdatePlan({
    required this.intervalHours,
    required this.sources,
  });

  /// `GuiItem.AutoUpdateInterval`; `0` disables the periodic resource task.
  final int intervalHours;

  /// Non-empty configured remote resource templates
  /// (`ConstItem.GeoSourceUrl` / `SrsSourceUrl` / `SubConvertUrl` /
  /// `RouteRulesTemplateSourceUrl`). Built-in fallbacks apply only at download
  /// time, so an empty list here still means "the user configured nothing".
  final List<String> sources;

  bool get enabled => intervalHours > 0;
  bool get hasSources => sources.isNotEmpty;
  bool get active => enabled && hasSources;
}

/// Build the resource plan from the canonical `guiNConfig.json` map.
ResourceAutoUpdatePlan resourceAutoUpdatePlan(Map<String, dynamic> settings) {
  final gui = _group(settings, 'GuiItem');
  final constItem = _group(settings, 'ConstItem');
  final sources = <String>[
    for (final key in const <String>[
      'GeoSourceUrl',
      'SrsSourceUrl',
      'SubConvertUrl',
      'RouteRulesTemplateSourceUrl',
    ])
      if (_nonEmpty(constItem[key])) constItem[key].toString().trim(),
  ];
  return ResourceAutoUpdatePlan(
    intervalHours: _int(gui['AutoUpdateInterval']),
    sources: sources,
  );
}

/// The upstream cadence: a pass `hoursSinceStart` after the scheduler started is
/// due only when the interval is enabled, the elapsed hours is positive and an
/// exact multiple of the interval. `hoursSinceStart == 0` is the startup tick
/// and never downloads.
bool resourcePassDue(ResourceAutoUpdatePlan plan, int hoursSinceStart) =>
    plan.enabled &&
    hoursSinceStart > 0 &&
    hoursSinceStart % plan.intervalHours == 0;

Map<String, dynamic> _group(Map<String, dynamic> settings, String name) {
  final value = settings[name];
  return value is Map ? value.cast<String, dynamic>() : <String, dynamic>{};
}

int _int(Object? value) {
  if (value is int) return value;
  if (value is num) return value.toInt();
  if (value is String) return int.tryParse(value.trim()) ?? 0;
  return 0;
}

bool _nonEmpty(Object? value) => value is String && value.trim().isNotEmpty;
