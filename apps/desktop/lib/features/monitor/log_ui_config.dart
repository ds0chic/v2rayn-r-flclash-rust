import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

/// Consumer view of `MsgUIItem` (upstream `ConfigItems.MsgUIItem`) for the
/// log tab (Wave B G-12 / FLD-CFG-065 MainMsgFilter, FLD-CFG-066 AutoRefresh).
///
/// Upstream semantics: `MainMsgFilter` is the persisted keyword the message
/// view opens with (null/empty = no filter; an invalid regex is rejected and
/// the previous filter is kept); `AutoRefresh` freezes only the page-local
/// view when off (null falls back to on, i.e. live tailing) while the core
/// keeps collecting. Filtering never drops already-collected lines.
class LogUiConfig {
  const LogUiConfig({this.keyword = '', this.autoRefresh = true});

  /// Persisted `MainMsgFilter`; empty means no filtering.
  final String keyword;

  /// Persisted `AutoRefresh`; null upstream falls back to true (live tail).
  final bool autoRefresh;
}

const LogUiConfig defaultLogUiConfig = LogUiConfig();

/// Parse the persisted settings document into a [LogUiConfig]. Missing keys
/// and wrong types fall back to the upstream defaults (`domain` keeps both
/// fields nullable with the same fallbacks).
LogUiConfig logUiConfigFromDocument(Map<String, dynamic> document) {
  final raw = document['MsgUIItem'];
  final group = raw is Map ? raw : const <String, dynamic>{};
  final filter = group['MainMsgFilter'];
  final refresh = group['AutoRefresh'];
  return LogUiConfig(
    keyword: filter is String ? filter : '',
    autoRefresh: refresh is bool ? refresh : true,
  );
}

/// Whether [pattern] is a usable log filter: empty always passes, otherwise it
/// must compile as a regex (upstream `MsgFilter` is regex-based). An invalid
/// pattern must be rejected with the previous filter kept (FLD-CFG-065).
bool isValidLogFilter(String pattern) {
  if (pattern.isEmpty) return true;
  try {
    RegExp(pattern);
    return true;
  } on FormatException {
    return false;
  }
}

/// Copy of the document's `MsgUIItem` group with [changes] applied, preserving
/// unknown keys. Used when the log tab writes its filter/switch back so the
/// value survives the next reopen (same persist-first contract as the Clash
/// tabs' `clashUiGroupWith`).
Map<String, dynamic> msgUiGroupWith(
  Map<String, dynamic> document,
  Map<String, Object?> changes,
) {
  final current = document['MsgUIItem'];
  final group = current is Map<String, dynamic>
      ? Map<String, dynamic>.of(current)
      : <String, dynamic>{};
  group.addAll(changes);
  return group;
}

final logUiConfigProvider = Provider<LogUiConfig>((ref) {
  final document = ref.watch(settingsControllerProvider).document;
  if (document.isEmpty) return defaultLogUiConfig;
  return logUiConfigFromDocument(document);
});
