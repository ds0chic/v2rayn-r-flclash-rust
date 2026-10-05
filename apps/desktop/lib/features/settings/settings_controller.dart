import 'dart:convert';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as contract;
import 'package:v2rayn_desktop/bridge/api/settings.dart' as settings;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

/// Riverpod view of the persisted `guiNConfig.json` settings document.
///
/// The document is kept as canonical JSON (`PascalCase` keys, integers for
/// enums) so unknown keys and the null/empty-string distinction survive a UI
/// round trip. Saving goes through the Rust engine's optimistic revision check;
/// the `immediate` subset of fields is applied to the live UI right after a
/// successful load or save.
class SettingsViewState {
  const SettingsViewState({
    this.loaded = false,
    this.loadFailed = false,
    this.revision = 0,
    this.document = const <String, dynamic>{},
    this.groupRevisions = const <String, int>{},
    this.status,
    this.needsCoreRestart = false,
    this.needsAppRestart = false,
    this.needsNextLaunch = false,
  });

  final bool loaded;

  /// A load attempt ran and failed. Saving on top of a failed load would
  /// persist defaults over the real stored config (AUD-ROOT-03).
  final bool loadFailed;
  final int revision;
  final Map<String, dynamic> document;
  final Map<String, int> groupRevisions;

  /// Last user-facing status message (success/reboot note/error).
  final String? status;
  final bool needsCoreRestart;
  final bool needsAppRestart;
  final bool needsNextLaunch;

  Map<String, dynamic> group(String key) {
    final value = document[key];
    return value is Map<String, dynamic> ? value : <String, dynamic>{};
  }

  SettingsViewState copyWith({
    bool? loaded,
    bool? loadFailed,
    int? revision,
    Map<String, dynamic>? document,
    Map<String, int>? groupRevisions,
    String? status,
    bool? needsCoreRestart,
    bool? needsAppRestart,
    bool? needsNextLaunch,
  }) {
    return SettingsViewState(
      loaded: loaded ?? this.loaded,
      loadFailed: loadFailed ?? this.loadFailed,
      revision: revision ?? this.revision,
      document: document ?? this.document,
      groupRevisions: groupRevisions ?? this.groupRevisions,
      status: status ?? this.status,
      needsCoreRestart: needsCoreRestart ?? this.needsCoreRestart,
      needsAppRestart: needsAppRestart ?? this.needsAppRestart,
      needsNextLaunch: needsNextLaunch ?? this.needsNextLaunch,
    );
  }
}

/// Result of a settings save that also drives the real side effects.
///
/// Persistence and application are reported separately so a save that succeeded
/// but failed to apply (or failed to write autostart) is never reported as a
/// full success (R4-13/D14, UFS-06). [ok] is true only when every required
/// effect ran; [saved] stays true once the document is persisted, so the UI can
/// show "已保存/未应用" and offer a retry instead of claiming nothing happened.
class SettingsApplyOutcome {
  const SettingsApplyOutcome({
    required this.ok,
    required this.saved,
    required this.applied,
    this.message,
    this.statusKey,
  });

  final bool ok;
  final bool saved;
  final bool applied;

  /// User-facing failure text; only set when [ok] is false.
  final String? message;

  /// Stable success message key (`settings.saved_need_core_restart`, ...).
  final String? statusKey;
}

final settingsControllerProvider =
    NotifierProvider<SettingsController, SettingsViewState>(
      SettingsController.new,
    );

class SettingsController extends Notifier<SettingsViewState> {
  @override
  SettingsViewState build() => const SettingsViewState();

  /// Revision captured when a draft was created, keyed by draft identity. A
  /// save must submit the revision the editor started from, not whatever the
  /// controller has advanced to since (AUD-DESK-02).
  final Map<int, int> _draftRevisions = <int, int>{};

  /// Last autostart value confirmed written to the OS, or null while unknown.
  /// A persisted `AutoRun` value is not proof the Run key write succeeded, so a
  /// failed write must be retried on the next save (AUD-DESK-01).
  bool? _autostartApplied;

  /// Load once from the engine. Safe to call repeatedly.
  SettingsViewState load() {
    settings.SettingsLoadDto result;
    try {
      result = ref.read(bridgePortProvider).getSettings();
    } catch (_) {
      // A missing/broken bridge is a load failure, never an editable default
      // document (AUD-ROOT-03): saving defaults would overwrite the real
      // stored config. Tests that need a document inject it explicitly.
      state = state.copyWith(
        loaded: false,
        loadFailed: true,
        status: 'error.settings_load_failed',
      );
      return state;
    }
    if (!result.ok || result.settingsJson.isEmpty) {
      state = state.copyWith(
        loaded: false,
        loadFailed: true,
        status: result.error?.messageKey ?? 'error.settings_load_failed',
      );
      return state;
    }
    final document = _tryDecode(result.settingsJson);
    if (document == null) {
      state = state.copyWith(
        loaded: false,
        loadFailed: true,
        status: 'error.settings_load_failed',
      );
      return state;
    }
    state = SettingsViewState(
      loaded: true,
      loadFailed: false,
      revision: result.revision.toInt(),
      document: document,
      groupRevisions: _decodeGroupRevisions(result.groupRevisionsJson),
    );
    _autostartApplied = _documentAutoRun(document);
    _applyImmediate(document);
    return state;
  }

  /// A deep copy of the current document, safe to edit in a dialog. The copy
  /// remembers the revision it was taken from so [saveDocument] can submit it.
  Map<String, dynamic> draft() {
    final copy = _deepCopy(state.document);
    _draftRevisions[identityHashCode(copy)] = state.revision;
    return copy;
  }

  int _revisionFor(Map<String, dynamic> draft, int? expectedRevision) =>
      expectedRevision ??
      _draftRevisions[identityHashCode(draft)] ??
      state.revision;

  /// Persist the whole document with the optimistic revision check.
  ///
  /// [expectedRevision] overrides the captured draft revision (used by the
  /// independent settings window, whose draft crosses a JSON boundary).
  settings.SaveSettingsResult saveDocument(
    Map<String, dynamic> draft, {
    int? expectedRevision,
  }) {
    if (!state.loaded || state.loadFailed) return _notLoadedSaveFailure();
    _normalizeCoreBasicBindings(draft);
    _normalizeRootCertProvider(draft);
    final result = ref
        .read(bridgePortProvider)
        .saveSettingsJson(jsonEncode(draft), _revisionFor(draft, expectedRevision));
    if (result.ok) {
      final newRevision = result.newRevision?.toInt() ?? state.revision;
      state = state.copyWith(
        loaded: true,
        revision: newRevision,
        document: _deepCopy(draft),
        status: _statusFor(result),
        needsCoreRestart: result.restartCoreFields.isNotEmpty,
        needsAppRestart: result.restartAppFields.isNotEmpty,
        needsNextLaunch: result.nextLaunchFields.isNotEmpty,
      );
      _draftRevisions[identityHashCode(draft)] = newRevision;
      // A whole save advances every group counter in the engine; refresh the
      // authoritative revisions so the next group save is not stale
      // (AUD-ROOT-01).
      _refreshAuthoritativeRevisions();
      _applyImmediate(draft);
    } else {
      state = state.copyWith(
        status: result.error?.messageKey ?? 'error.settings_save_failed',
      );
    }
    return result;
  }

  void _refreshAuthoritativeRevisions() {
    try {
      final result = ref.read(bridgePortProvider).getSettings();
      if (!result.ok || result.settingsJson.isEmpty) return;
      final document = _tryDecode(result.settingsJson);
      if (document == null) return;
      state = state.copyWith(
        revision: result.revision.toInt(),
        document: document,
        groupRevisions: _decodeGroupRevisions(result.groupRevisionsJson),
      );
      _applyImmediate(document);
    } catch (_) {
      // The save already succeeded; a failed re-read keeps the local view.
    }
  }

  static settings.SaveSettingsResult _notLoadedSaveFailure() =>
      settings.SaveSettingsResult(
        ok: false,
        changes: const [],
        restartCoreFields: const [],
        restartAppFields: const [],
        nextLaunchFields: const [],
        error: const contract.ErrorDto(
          code: 'E_SETTINGS_NOT_LOADED',
          messageKey: 'error.settings_load_failed',
          retryable: false,
        ),
      );

  /// Replace one top-level group (used by the theme/hotkey windows).
  settings.SaveSettingsResult saveGroup(String group, Object? value) {
    // A failed load must block any mutation; a never-loaded controller only
    // ever writes a group patch that the engine still revision-checks.
    if (state.loadFailed) return _notLoadedSaveFailure();
    final groupRevision = state.groupRevisions[group] ?? 0;
    final result = ref
        .read(bridgePortProvider)
        .saveSettingsGroup(group, jsonEncode(value), groupRevision);
    if (result.ok) {
      final document = _deepCopy(state.document);
      document[group] = value;
      final revisions = Map<String, int>.of(state.groupRevisions);
      revisions[group] = groupRevision + 1;
      state = state.copyWith(
        loaded: true,
        revision: result.newRevision?.toInt() ?? state.revision,
        document: document,
        groupRevisions: revisions,
        status: _statusFor(result),
        needsCoreRestart: result.restartCoreFields.isNotEmpty,
        needsAppRestart: result.restartAppFields.isNotEmpty,
        needsNextLaunch: result.nextLaunchFields.isNotEmpty,
      );
      _applyImmediate(document);
    } else {
      state = state.copyWith(
        status: result.error?.messageKey ?? 'error.settings_save_failed',
      );
    }
    return result;
  }

  /// Persist [draft], sync autostart when it changed, then await the real plan
  /// apply and report each stage honestly (R4-13/D14, UFS-06).
  ///
  /// Upstream `OptionSettingViewModel.SaveSettingAsync` writes autostart after
  /// the settings save, then `MainWindowViewModel.Reload` re-applies the plan.
  /// A save failure returns before any side effect. A save that succeeds but
  /// whose apply (or autostart write) fails returns `ok: false` with `saved:
  /// true` and a user-facing message; the persisted value is still visible on
  /// reopen, but the result is never faked as a full success.
  Future<SettingsApplyOutcome> saveAndApply(
    Map<String, dynamic> draft, {
    int? expectedRevision,
  }) async {
    final previousAutoRun = _documentAutoRun(state.document);
    final result = saveDocument(draft, expectedRevision: expectedRevision);
    if (!result.ok) {
      return SettingsApplyOutcome(
        ok: false,
        saved: false,
        applied: false,
        message: _saveFailureMessage(result.error?.messageKey),
      );
    }

    // Autostart must be retried until the OS write is confirmed: a persisted
    // `AutoRun` value is not proof the Run key exists (AUD-DESK-01).
    var autostartFailed = false;
    final desiredAutoRun = _draftAutoRun(draft);
    if (desiredAutoRun != previousAutoRun ||
        _autostartApplied != desiredAutoRun) {
      final written = _writeAutostart(desiredAutoRun);
      autostartFailed = !written;
      if (written) _autostartApplied = desiredAutoRun;
    }

    final statusKey = _statusKeyFor(result);
    var applied = false;
    String? applyMessage;
    try {
      applied = await ref
          .read(runtimeControllerProvider.notifier)
          .applyActive();
      if (!applied) {
        final error = ref.read(runtimeControllerProvider).error;
        applyMessage = '配置已保存，但应用失败：${error?.messageKey ?? 'unknown'}';
      }
    } on Object catch (e) {
      applyMessage = '配置已保存，但应用失败：$e';
    }

    if (!applied) {
      ref.read(uiShellControllerProvider.notifier).setMessage(applyMessage);
      return SettingsApplyOutcome(
        ok: false,
        saved: true,
        applied: false,
        message: applyMessage,
        statusKey: statusKey,
      );
    }

    // Platform stage: the persisted system-proxy mode is part of this
    // operation; a failure keeps "saved, platform not applied" visible and
    // retryable instead of reporting a full success (AUD-ROOT-05 /
    // AUD-DESK-03). Driven through the platform controller's static hook: a
    // direct provider read here would close a dependency cycle with the
    // controller's own settings listener.
    String? platformMessage;
    try {
      final platformResult = PlatformController.applySavedMode(draft);
      if (platformResult != null && !platformResult.ok) {
        final key =
            platformResult.error?.messageKey ??
            platformResult.error?.code ??
            'unknown';
        platformMessage = '配置已保存，但系统代理应用失败：$key';
      }
    } on Object catch (e) {
      platformMessage = '配置已保存，但系统代理应用失败：$e';
    }
    if (platformMessage != null) {
      ref.read(uiShellControllerProvider.notifier).setMessage(platformMessage);
      return SettingsApplyOutcome(
        ok: false,
        saved: true,
        applied: true,
        message: platformMessage,
        statusKey: statusKey,
      );
    }

    if (autostartFailed) {
      const message = '配置已保存，但开机自启写入失败';
      ref.read(uiShellControllerProvider.notifier).setMessage(message);
      return const SettingsApplyOutcome(
        ok: false,
        saved: true,
        applied: true,
        message: message,
      );
    }
    ref
        .read(uiShellControllerProvider.notifier)
        .setMessage(statusMessageFor(statusKey));
    return SettingsApplyOutcome(
      ok: true,
      saved: true,
      applied: true,
      statusKey: statusKey,
    );
  }

  static String _statusKeyFor(settings.SaveSettingsResult result) {
    if (result.restartAppFields.isNotEmpty) {
      return 'settings.saved_need_app_restart';
    }
    if (result.restartCoreFields.isNotEmpty) {
      return 'settings.saved_need_core_restart';
    }
    if (result.nextLaunchFields.isNotEmpty) {
      return 'settings.saved_need_next_launch';
    }
    return 'settings.saved';
  }

  /// Frozen `ResUI.OperationSuccess` / `NeedRebootTips` wording for the main
  /// window status line, so the restart hint survives the settings window close.
  static String statusMessageFor(String key) {
    switch (key) {
      case 'settings.saved_need_app_restart':
        return '操作成功。请点击设置菜单重启应用。';
      case 'settings.saved_need_core_restart':
        return '操作成功，请重启服务';
      case 'settings.saved_need_next_launch':
        return '操作成功，下次启动生效';
      default:
        return '操作成功';
    }
  }

  static String _saveFailureMessage(String? key) {
    switch (key) {
      case 'error.settings_load_failed':
        return '读取配置失败';
      case 'error.settings_save_failed':
        return '保存配置失败';
      default:
        return '操作失败，请检查并重试';
    }
  }

  static bool _documentAutoRun(Map<String, dynamic> document) {
    final gui = document['GuiItem'];
    return gui is Map && gui['AutoRun'] == true;
  }

  static bool _draftAutoRun(Map<String, dynamic> draft) {
    final gui = draft['GuiItem'];
    return gui is Map && gui['AutoRun'] == true;
  }

  bool _writeAutostart(bool enabled) {
    try {
      final bridge = ref.read(platformBridgeProvider);
      final exe = Platform.resolvedExecutable;
      final name = bridge.autostartValueName(exe);
      return bridge.setAutostart(
        name: name,
        enabled: enabled,
        exe: exe,
        args: '',
      );
    } on Object {
      return false;
    }
  }

  void clearStatus() => state = state.copyWith(status: null);

  /// Upstream `OptionSettingViewModel.SaveSettingAsync`: `SendThrough.TrimEx()`
  /// and `BindInterface.TrimEx()`. A whitespace/empty value is written back as
  /// `null`, a non-empty value is trimmed.
  static void _normalizeCoreBasicBindings(Map<String, dynamic> draft) {
    final core = draft['CoreBasicItem'];
    if (core is! Map) return;
    for (final key in const <String>['SendThrough', 'BindInterface']) {
      final value = core[key];
      if (value is! String) continue;
      final trimmed = value.trim();
      core[key] = trimmed.isEmpty ? null : trimmed;
    }
  }

  /// Upstream `ConfigHandler.LoadConfig` parity: an out-of-list
  /// `GuiItem.RootCertProvider` is forced to `system`. Missing/null is left to
  /// the Rust defaults so unknown-field preservation stays intact.
  static void _normalizeRootCertProvider(Map<String, dynamic> draft) {
    final gui = draft['GuiItem'];
    if (gui is Map) {
      final current = gui['RootCertProvider'];
      if (current is String && !rootCertProviders.contains(current)) {
        gui['RootCertProvider'] = defaultRootCertProvider;
      }
    }
  }

  void _applyImmediate(Map<String, dynamic> document) {
    ref
        .read(uiShellControllerProvider.notifier)
        .applySettingsDocument(document);
    final ui = document['UiItem'];
    if (ui is Map<String, dynamic>) {
      ref
          .read(profilesControllerProvider.notifier)
          .setDoubleClick2Activate(ui['DoubleClick2Activate'] == true);
    }
  }

  String? _statusFor(settings.SaveSettingsResult result) {
    if (result.restartAppFields.isNotEmpty) {
      return 'settings.saved_need_app_restart';
    }
    if (result.restartCoreFields.isNotEmpty) {
      return 'settings.saved_need_core_restart';
    }
    if (result.nextLaunchFields.isNotEmpty) {
      return 'settings.saved_need_next_launch';
    }
    return 'settings.saved';
  }

  static Map<String, dynamic>? _tryDecode(String raw) {
    try {
      final decoded = jsonDecode(raw);
      if (decoded is Map<String, dynamic>) return decoded;
    } catch (_) {}
    return null;
  }

  static Map<String, int> _decodeGroupRevisions(String raw) {
    try {
      final decoded = jsonDecode(raw);
      if (decoded is Map) {
        return decoded.map(
          (k, v) => MapEntry(k.toString(), (v as num).toInt()),
        );
      }
    } catch (_) {}
    return const <String, int>{};
  }

  static Map<String, dynamic> _deepCopy(Map<String, dynamic> source) =>
      jsonDecode(jsonEncode(source)) as Map<String, dynamic>;
}
