import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/settings.dart' as settings;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
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

  void clearStatus() => state = state.copyWith(status: null);

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
