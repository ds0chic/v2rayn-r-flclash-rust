import '../../bridge/api/contract.dart' as c;
import 'l10n.dart';

/// Maps a stable contract key (`messageKey` / `code` / `statusKey`) to a human,
/// localizable string. Covered key families:
///
///  * bridge `ErrorDto.messageKey` (`error.*`) -> cause + next action;
///  * window status keys (`settings.saved*`) -> upstream OperationSuccess /
///    NeedRebootTips wording;
///  * the `action.*` / `errorUnknow` fallbacks.
///
/// A key that is not in the table is funnelled through [fallbackFor], which
/// guarantees the user never sees a raw dotted key and never sees a bare code:
/// the English generic wording is shown and the technical code is preserved for
/// the diagnostics/details popup only.
class ErrorLocalizer {
  const ErrorLocalizer(this._l10n);

  final L10n _l10n;

  /// Localize an [ErrorDto] to a readable cause + action. The `code` is kept out
  /// of the headline; callers that want it for diagnostics render
  /// `label (code)`. A `detail` payload is never shown here (it may carry
  /// sensitive fragments) — it belongs to an explicitly expanded details view.
  String error(ErrorDtoLike error) {
    final text = _byMessageKey(error.messageKey);
    if (text != null) return text;
    if (error.retryable) return _l10n.t('actionReasonBackendDown');
    return _l10n.t('errorUnknown');
  }

  /// Localize a raw `messageKey` string (status keys included).
  String key(String? messageKey) {
    final direct = _byMessageKey(messageKey);
    if (direct != null) return direct;
    return fallbackFor(messageKey);
  }

  /// Guarantee readable text: known key -> translation, a bare dotted contract
  /// key -> generic failure, already-human text -> passed through unchanged.
  /// Never returns a raw contract key.
  String fallbackFor(String? rawKey) {
    if (rawKey == null || rawKey.isEmpty) return _l10n.t('errorUnknown');
    // A relayed, already-human message (e.g. "配置已保存，但应用失败：...") must be
    // shown as-is; only a bare `namespace.key` token is a raw contract key.
    if (!_bareKeyPattern.hasMatch(rawKey)) return rawKey;
    return _l10n.t('OperationFailed');
  }

  /// Matches a single opaque contract key such as `error.revision_stale`.
  static final RegExp _bareKeyPattern = RegExp(
    r'^[a-z][a-z0-9_]*(\.[a-z0-9_]+)+$',
  );

  /// Append the stable code for a diagnostics surface (never the headline).
  static String withCode(String label, String code) => '$label ($code)';

  String? _byMessageKey(String? key) {
    switch (key) {
      case 'settings.saved':
        return _l10n.t('settingsSaved');
      case 'settings.saved_need_core_restart':
        return _l10n.t('settingsNeedCoreRestart');
      case 'settings.saved_need_app_restart':
        return _l10n.t('settingsNeedAppRestart');
      case 'settings.saved_need_next_launch':
        return _l10n.t('settingsNeedNextLaunch');
      case 'settings.saved_not_applied':
        return _l10n.t('settingsSavedNotApplied');
      case 'settings.saved_autostart_failed':
        return _l10n.t('settingsSavedAutostartFailed');
    }
    final mapped = _errorKeyMap[key];
    if (mapped != null) return _l10n.t(mapped);
    final validate = _validateKeyMap[key];
    if (validate != null) return _l10n.t(validate);
    return null;
  }

  /// Window-local validation keys -> localizable resource keys.
  static const Map<String, String> _validateKeyMap = <String, String>{
    'validate.local_port': 'validateLocalPort',
    'validate.fragment': 'validateFragment',
  };

  /// `error.<name>` contract key -> localizable resource key.
  static const Map<String, String> _errorKeyMap = <String, String>{
    'error.unknown': 'errorUnknown',
    'error.revision_stale': 'errorRevisionStale',
    'error.not_found': 'errorNotFound',
    'error.url_required': 'errorUrlRequired',
    'error.url_invalid': 'errorUrlInvalid',
    'error.invalid_uri': 'errorInvalidUri',
    'error.import_nothing': 'errorImportNothing',
    'error.remarks_required': 'errorRemarksRequired',
    'error.delete_failed': 'errorDeleteFailed',
    'error.no_profiles_selected': 'errorNoProfilesSelected',
    'error.not_wired': 'errorNotWired',
    'error.codegen_failed': 'errorCodegenFailed',
    'error.runtime_timeout': 'errorRuntimeTimeout',
    'error.runtime_apply_failed': 'errorRuntimeApplyFailed',
    'error.sub_headers_invalid': 'errorSubHeadersInvalid',
    'error.sub_update_unconfirmed': 'errorSubUpdateUnconfirmed',
    'error.backup_invalid': 'errorBackupInvalid',
    'error.webdav_permission': 'errorWebdavPermission',
    'error.order_persist': 'errorOrderPersist',
    'error.routing_rules_invalid': 'errorRoutingRulesInvalid',
    'error.rule_mode_invalid': 'errorRuleModeInvalid',
    'error.settings_json': 'errorSettingsJson',
    'error.settings_load_failed': 'errorSettingsLoadFailed',
    'error.settings_save_failed': 'errorSettingsSaveFailed',
    'error.bridge_unavailable': 'errorBridgeUnavailable',
    'error.proxy_unavailable': 'errorProxyUnavailable',
    'error.pac_file_read': 'errorPacFileRead',
    'error.sysproxy_no_running_session': 'errorSysproxyNoRunningSession',
    'error.speedtest_udp_unsupported': 'errorSpeedtestUdpUnsupported',
    'error.custom_file_not_found': 'errorCustomFileNotFound',
    'error.template_core_unsupported': 'errorTemplateCoreUnsupported',
    'error.template_json_invalid': 'errorTemplateJsonInvalid',
    'error.group_children_required': 'errorGroupChildrenRequired',
    'error.config_corrupt': 'errorConfigCorrupt',
    'error.webdav_tls': 'errorWebdavTls',
  };
}

/// Structural view of an error needed by [ErrorLocalizer]; lets both the bridge
/// `ErrorDto` and the runtime/platform error views feed the same localizer.
abstract interface class ErrorDtoLike {
  String get messageKey;

  /// Whether the error is safe to retry. Errors arriving from the runtime or
  /// platform views are conservatively treated as non-retryable (they surface as
  /// a cause + action instead of a blind "retry connection").
  bool get retryable;
}

/// Adapter so a real bridge [c.ErrorDto] can be localised directly.
class BridgeErrorView implements ErrorDtoLike {
  const BridgeErrorView(this.dto);

  final c.ErrorDto dto;

  @override
  String get messageKey => dto.messageKey;

  @override
  bool get retryable => dto.retryable;
}

/// Adapter for the runtime/platform error projections (`RuntimeErrorView` /
/// `PlatformErrorView`), which carry `code`/`messageKey`/`detail` only.
class SimpleErrorView implements ErrorDtoLike {
  const SimpleErrorView({required this.messageKey, this.retryable = false});

  @override
  final String messageKey;

  @override
  final bool retryable;
}
