import 'dart:convert';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/settings.dart' as settings;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
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
    this.revision = 0,
    this.document = const <String, dynamic>{},
    this.groupRevisions = const <String, int>{},
    this.status,
    this.needsCoreRestart = false,
    this.needsAppRestart = false,
    this.needsNextLaunch = false,
  });

  final bool loaded;
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

  /// Load once from the engine. Safe to call repeatedly.
  SettingsViewState load() {
    settings.SettingsLoadDto result;
    try {
      result = ref.read(bridgePortProvider).getSettings();
    } catch (_) {
      // No native library (pure widget tests): keep the local defaults.
      final doc = defaultSettingsJson();
      state = SettingsViewState(loaded: true, revision: 0, document: doc);
      _applyImmediate(doc);
      return state;
    }
    if (!result.ok || result.settingsJson.isEmpty) {
      state = state.copyWith(
        loaded: false,
        status: result.error?.messageKey ?? 'error.settings_load_failed',
      );
      return state;
    }
    final document = _decode(result.settingsJson);
    state = SettingsViewState(
      loaded: true,
      revision: result.revision.toInt(),
      document: document,
      groupRevisions: _decodeGroupRevisions(result.groupRevisionsJson),
    );
    _applyImmediate(document);
    return state;
  }

  /// A deep copy of the current document, safe to edit in a dialog.
  Map<String, dynamic> draft() => _deepCopy(state.document);

  /// Persist the whole document with the optimistic revision check.
  settings.SaveSettingsResult saveDocument(Map<String, dynamic> draft) {
    _normalizeCoreBasicBindings(draft);
    _normalizeRootCertProvider(draft);
    final result = ref
        .read(bridgePortProvider)
        .saveSettingsJson(jsonEncode(draft), state.revision);
    if (result.ok) {
      state = state.copyWith(
        loaded: true,
        revision: result.newRevision?.toInt() ?? state.revision,
        document: _deepCopy(draft),
        status: _statusFor(result),
        needsCoreRestart: result.restartCoreFields.isNotEmpty,
        needsAppRestart: result.restartAppFields.isNotEmpty,
        needsNextLaunch: result.nextLaunchFields.isNotEmpty,
      );
      _applyImmediate(draft);
    } else {
      state = state.copyWith(
        status: result.error?.messageKey ?? 'error.settings_save_failed',
      );
    }
    return result;
  }

  /// Replace one top-level group (used by the theme/hotkey windows).
  settings.SaveSettingsResult saveGroup(String group, Object? value) {
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
  Future<SettingsApplyOutcome> saveAndApply(Map<String, dynamic> draft) async {
    final previousAutoRun = _documentAutoRun(state.document);
    final result = saveDocument(draft);
    if (!result.ok) {
      return SettingsApplyOutcome(
        ok: false,
        saved: false,
        applied: false,
        message: _saveFailureMessage(result.error?.messageKey),
      );
    }

    var autostartFailed = false;
    if (_draftAutoRun(draft) != previousAutoRun) {
      autostartFailed = !_writeAutostart(_draftAutoRun(draft));
    }

    final statusKey = _statusKeyFor(result);
    var applied = false;
    String? applyMessage;
    try {
      await ref.read(runtimeControllerProvider.notifier).applyActive();
      final error = ref.read(runtimeControllerProvider).error;
      applied = error == null;
      if (!applied) {
        applyMessage = '配置已保存，但应用失败：${error.messageKey}';
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

  static Map<String, dynamic> _decode(String raw) {
    try {
      final decoded = jsonDecode(raw);
      if (decoded is Map<String, dynamic>) return decoded;
    } catch (_) {}
    return defaultSettingsJson();
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
