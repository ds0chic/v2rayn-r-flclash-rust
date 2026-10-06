import 'dart:convert';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/shell/desktop_integration.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

final updateControllerProvider =
    NotifierProvider<UpdateController, UpdateState>(UpdateController.new);

/// Launches the external self-update runner. Returns once the detached helper
/// process has been handed off; it never waits for the helper to finish.
typedef RunnerLauncher = Future<void> Function(
  String helperExe,
  List<String> args,
  String? workingDirectory,
);

/// Hands control over to the runner by exiting the application.
typedef AppExit = void Function();

Future<void> _defaultLaunchRunner(
  String helperExe,
  List<String> args,
  String? workingDirectory,
) async {
  await Process.start(
    helperExe,
    args,
    mode: ProcessStartMode.detached,
    workingDirectory: workingDirectory,
  );
}

void _defaultExitApp() {
  // Give the success status one frame to render, then exit so the runner (which
  // waits on this PID) can replace the flat install root and relaunch.
  Future<void>.delayed(const Duration(milliseconds: 500), () => exit(0));
}

/// A structured message (no fake percent; stage text only).
class UpdateStatus {
  const UpdateStatus({required this.kind, required this.message, this.detail});

  /// `info` / `stage` / `success` / `error`.
  final String kind;
  final String message;
  final String? detail;

  bool get isError => kind == 'error';
  bool get isSuccess => kind == 'success';
}

class UpdateState {
  const UpdateState({
    required this.targets,
    this.selected = const <String>{},
    this.prerelease = false,
    this.viaProxy = true,
    this.busy = false,
    this.stage,
    this.checks = const <c.CoreUpdateDto>[],
    this.status,
    this.lastSpec,
    this.missingCores = const <String>[],
  });

  final List<c.UpdateTargetDto> targets;
  final Set<String> selected;
  final bool prerelease;
  final bool viaProxy;
  final bool busy;
  final String? stage;
  final List<c.CoreUpdateDto> checks;
  final UpdateStatus? status;
  final c.ExternalSpecDto? lastSpec;

  /// Cores the runtime reported missing (`error.core_not_found`, plan §3.7).
  /// A non-empty list drives the window's explicit install/repair entry.
  final List<String> missingCores;

  bool get canApply =>
      !busy && checks.any((check) => check.supported && check.hasUpdate);

  c.CoreUpdateDto? checkFor(String core) {
    for (final check in checks) {
      if (check.core == core) return check;
    }
    return null;
  }

  UpdateState copyWith({
    List<c.UpdateTargetDto>? targets,
    Set<String>? selected,
    bool? prerelease,
    bool? viaProxy,
    bool? busy,
    String? stage,
    bool clearStage = false,
    List<c.CoreUpdateDto>? checks,
    UpdateStatus? status,
    c.ExternalSpecDto? lastSpec,
    List<String>? missingCores,
  }) => UpdateState(
    targets: targets ?? this.targets,
    selected: selected ?? this.selected,
    prerelease: prerelease ?? this.prerelease,
    viaProxy: viaProxy ?? this.viaProxy,
    busy: busy ?? this.busy,
    stage: clearStage ? null : (stage ?? this.stage),
    checks: checks ?? this.checks,
    status: status ?? this.status,
    lastSpec: lastSpec ?? this.lastSpec,
    missingCores: missingCores ?? this.missingCores,
  );
}

/// Check-update window controller (F-APP-005/007, F-CORE-002/003).
class UpdateController extends Notifier<UpdateState> {
  UpdateController({RunnerLauncher? launchRunner, AppExit? exitApp})
    : _launchRunner = launchRunner ?? _defaultLaunchRunner,
      _exitApp = exitApp ?? _defaultExitApp;

  final RunnerLauncher _launchRunner;
  final AppExit _exitApp;

  @override
  UpdateState build() {
    final targets = ref.read(bridgePortProvider).t16UpdateTargets();
    // Upstream `CheckUpdateViewModel.Init`: the two toggles and the selected
    // core list are seeded from the persisted `CheckUpdateItem`, with the
    // C# defaults `UpdateViaProxy=true` and `SelectedCoreTypes=null` (=> all
    // selected, `UpdateService.cs:119`).
    final persisted = _readPersistedCheckUpdate();
    final selectedTypes = persisted['SelectedCoreTypes'];
    return UpdateState(
      targets: targets,
      selected: <String>{
        for (final target in targets)
          if (target.supported &&
              (selectedTypes is! List ||
                  selectedTypes
                      .map((type) => type.toString())
                      .contains(target.core)))
            target.core,
      },
      prerelease: persisted['CheckPreReleaseUpdate'] == true,
      viaProxy: persisted['UpdateViaProxy'] != false,
    );
  }

  /// The persisted `CheckUpdateItem` group, read from the storage owner (the
  /// stored `guiNConfig.json`) so a saved/reopened value drives the check.
  Map<String, dynamic> _readPersistedCheckUpdate() {
    try {
      final loaded = ref.read(bridgePortProvider).getSettings();
      if (!loaded.ok || loaded.settingsJson.isEmpty) {
        return const <String, dynamic>{};
      }
      final decoded = jsonDecode(loaded.settingsJson);
      if (decoded is Map<String, dynamic>) {
        final group = decoded['CheckUpdateItem'];
        if (group is Map<String, dynamic>) return group;
      }
    } catch (_) {}
    return const <String, dynamic>{};
  }

  /// Persist the given toggles/selection into the `CheckUpdateItem` group.
  /// Upstream writes these back on every change
  /// (`CheckUpdateViewModel.OnCheckPreReleaseUpdateChanged` -> `SaveConfig`),
  /// so reopening the window restores them instead of resetting to defaults.
  ///
  /// Returns whether the write succeeded. Callers (SP-27) persist *first* and
  /// only move the in-memory state on success; a failed write keeps the old
  /// state and surfaces an error instead of optimistically showing the new
  /// selection as saved (SP-12 audit contract 3).
  bool _persistCheckUpdate(Map<String, dynamic> group) {
    try {
      final notifier = ref.read(settingsControllerProvider.notifier);
      if (!ref.read(settingsControllerProvider).loaded) {
        notifier.load();
      }
      final result = notifier.saveGroup('CheckUpdateItem', group);
      if (!result.ok) {
        final error = result.error;
        state = state.copyWith(
          status: UpdateStatus(
            kind: 'error',
            message: '更新选项保存失败',
            detail: error == null
                ? 'unknown'
                : '${error.code} / ${error.messageKey}',
          ),
        );
        return false;
      }
      return true;
    } catch (error) {
      state = state.copyWith(
        status: UpdateStatus(
          kind: 'error',
          message: '更新选项保存失败',
          detail: error.toString(),
        ),
      );
      return false;
    }
  }

  Map<String, dynamic> _checkUpdateGroup({
    bool? prerelease,
    bool? viaProxy,
    Set<String>? selected,
  }) => <String, dynamic>{
    'CheckPreReleaseUpdate': prerelease ?? state.prerelease,
    'UpdateViaProxy': viaProxy ?? state.viaProxy,
    'SelectedCoreTypes': (selected ?? state.selected).toList()..sort(),
  };

  void toggleCore(String core, bool selected) {
    if (selected == state.selected.contains(core)) return;
    final next = <String>{...state.selected};
    if (selected) {
      next.add(core);
    } else {
      next.remove(core);
    }
    if (_persistCheckUpdate(_checkUpdateGroup(selected: next))) {
      state = state.copyWith(selected: next);
    }
  }

  void setPrerelease(bool value) {
    if (value == state.prerelease) return;
    if (_persistCheckUpdate(_checkUpdateGroup(prerelease: value))) {
      state = state.copyWith(prerelease: value);
    }
  }

  void setViaProxy(bool value) {
    if (value == state.viaProxy) return;
    if (_persistCheckUpdate(_checkUpdateGroup(viaProxy: value))) {
      state = state.copyWith(viaProxy: value);
    }
  }

  List<String> get _selectedCores => state.targets
      .where(
        (target) => target.supported && state.selected.contains(target.core),
      )
      .map((target) => target.core)
      .toList();

  String _detail(c.ErrorDto? error) {
    if (error == null) return 'unknown';
    return error.detail == null
        ? '${error.code} / ${error.messageKey}'
        : '${error.code} / ${error.messageKey}: ${error.detail}';
  }

  /// Run a bridge call, converting a thrown/native exception into a reported
  /// error and clearing `busy` so the window never latches (R4-29/D23). A
  /// normal `ok=false` result is *not* an exception and is handled by callers.
  Future<T?> _bridgeCall<T>(
    Future<T> Function() call,
    String failureMessage,
  ) async {
    try {
      return await call();
    } catch (error) {
      state = state.copyWith(
        busy: false,
        clearStage: true,
        status: UpdateStatus(
          kind: 'error',
          message: failureMessage,
          detail: error.toString(),
        ),
      );
      return null;
    }
  }

  Future<void> checkOnly() async {
    if (state.busy) return;
    state = state.copyWith(
      busy: true,
      stage: '正在检查更新…',
      checks: const <c.CoreUpdateDto>[],
    );
    final result = await _bridgeCall(
      () => ref
          .read(bridgePortProvider)
          .t16CheckUpdates(_selectedCores, state.prerelease, state.viaProxy),
      '检查更新失败',
    );
    if (result == null) return;
    if (!result.ok) {
      state = state.copyWith(
        busy: false,
        clearStage: true,
        status: UpdateStatus(
          kind: 'error',
          message: '检查更新失败',
          detail: _detail(result.error),
        ),
      );
      return;
    }
    final updates = result.checks.where((c) => c.hasUpdate).length;
    final blocked = result.checks.where((c) => !c.supported).length;
    final String message;
    if (updates > 0) {
      message = blocked > 0
          ? '发现 $updates 个可更新目标（$blocked 个无法检查）'
          : '发现 $updates 个可更新目标';
    } else if (blocked > 0) {
      message = '没有可更新目标；$blocked 个目标无法检查（发行源未配置）';
    } else {
      message = '全部为最新版本';
    }
    state = state.copyWith(
      busy: false,
      clearStage: true,
      checks: result.checks,
      status: UpdateStatus(
        kind: updates > 0 ? 'success' : 'info',
        message: message,
      ),
    );
  }

  Future<void> checkAndApply() async {
    if (state.busy) return;
    state = state.copyWith(
      busy: true,
      stage: '正在检查更新…',
      checks: const <c.CoreUpdateDto>[],
    );
    final check = await _bridgeCall(
      () => ref
          .read(bridgePortProvider)
          .t16CheckUpdates(_selectedCores, state.prerelease, state.viaProxy),
      '检查更新失败',
    );
    if (check == null) return;
    if (!check.ok) {
      state = state.copyWith(
        busy: false,
        clearStage: true,
        status: UpdateStatus(
          kind: 'error',
          message: '检查更新失败',
          detail: _detail(check.error),
        ),
      );
      return;
    }
    state = state.copyWith(checks: check.checks, stage: '正在下载并安装内核更新…');
    final apply = await _bridgeCall(
      () => ref
          .read(bridgePortProvider)
          .t16ApplyCoreUpdate(_selectedCores, state.prerelease, state.viaProxy),
      '内核更新失败',
    );
    if (apply == null) return;
    if (!apply.ok) {
      state = state.copyWith(
        busy: false,
        clearStage: true,
        status: UpdateStatus(
          kind: 'error',
          message: '内核更新失败',
          detail: _detail(apply.error),
        ),
      );
      return;
    }
    state = state.copyWith(
      busy: false,
      clearStage: true,
      status: UpdateStatus(
        kind: 'success',
        message: '已更新 ${apply.applied.length} 个内核，跳过 ${apply.skipped.length} 个',
      ),
    );
  }

  /// Seed the cores the runtime just reported missing so the window offers an
  /// immediate install/repair entry (plan §3.7). No update call is made here;
  /// the user's button drives the existing T16 check+apply pipeline.
  void seedMissingCores(List<String> cores) {
    state = state.copyWith(
      missingCores: List<String>.unmodifiable(
        cores.where((core) => core.trim().isNotEmpty),
      ),
    );
  }

  /// Install/repair exactly [cores] through the existing T16 check+apply
  /// pipeline. A failure is surfaced verbatim and the missing list is kept so
  /// the user can retry; success is never fabricated.
  Future<void> installCores(List<String> cores) async {
    if (state.busy || cores.isEmpty) return;
    state = state.copyWith(
      busy: true,
      stage: '正在安装/修复内核…',
      checks: const <c.CoreUpdateDto>[],
    );
    final check = await _bridgeCall(
      () => ref
          .read(bridgePortProvider)
          .t16CheckUpdates(cores, state.prerelease, state.viaProxy),
      '检查更新失败',
    );
    if (check == null) return;
    if (!check.ok) {
      state = state.copyWith(
        busy: false,
        clearStage: true,
        status: UpdateStatus(
          kind: 'error',
          message: '检查更新失败',
          detail: _detail(check.error),
        ),
      );
      return;
    }
    state = state.copyWith(checks: check.checks, stage: '正在下载并安装内核…');
    final apply = await _bridgeCall(
      () => ref
          .read(bridgePortProvider)
          .t16ApplyCoreUpdate(cores, state.prerelease, state.viaProxy),
      '内核安装失败，可重试',
    );
    if (apply == null) return;
    if (!apply.ok) {
      state = state.copyWith(
        busy: false,
        clearStage: true,
        status: UpdateStatus(
          kind: 'error',
          message: '内核安装失败，可重试',
          detail: _detail(apply.error),
        ),
      );
      return;
    }
    state = state.copyWith(
      busy: false,
      clearStage: true,
      missingCores: const <String>[],
      status: UpdateStatus(
        kind: 'success',
        message: '已安装 ${apply.applied.length} 个内核，可再次启动',
      ),
    );
  }

  Future<void> installMissingCores() =>
      installCores(List<String>.of(state.missingCores));

  /// Stage the application update, then actually launch the external runner
  /// and exit so it can replace the flat install root and relaunch (RR-04).
  ///
  /// The spec is produced by `t16_apply_app_update_spec`; we never fabricate a
  /// success — a missing helper or a failed spawn is reported, not hidden.
  Future<void> applyAppUpdate() async {
    if (state.busy) return;
    state = state.copyWith(busy: true, stage: '正在准备应用自身更新…');
    final spec = await _bridgeCall(
      () => ref.read(bridgePortProvider).t16ApplyAppUpdateSpec(),
      '应用更新不可用',
    );
    if (spec == null) return;
    if (!spec.ok) {
      state = state.copyWith(
        busy: false,
        clearStage: true,
        lastSpec: spec,
        status: UpdateStatus(
          kind: 'error',
          message:
              spec.error?.messageKey == 'error.update_app_source_unconfigured'
              ? '应用自身发行源未配置'
              : '应用更新不可用',
          detail: _detail(spec.error),
        ),
      );
      return;
    }
    final helper = spec.helperExe;
    if (helper == null || helper.isEmpty) {
      state = state.copyWith(
        busy: false,
        clearStage: true,
        lastSpec: spec,
        status: const UpdateStatus(
          kind: 'error',
          message: '启动更新程序失败',
          detail: '升级程序路径缺失（helper_exe 为空）',
        ),
      );
      return;
    }
    final installRoot = spec.installRoot;
    try {
      await _launchRunner(
        helper,
        spec.args,
        (installRoot == null || installRoot.isEmpty) ? null : installRoot,
      );
    } catch (error) {
      state = state.copyWith(
        busy: false,
        clearStage: true,
        lastSpec: spec,
        status: UpdateStatus(
          kind: 'error',
          message: '启动更新程序失败',
          detail: _detail(spec.error) == 'unknown'
              ? error.toString()
              : '${error.toString()}（${_detail(spec.error)}）',
        ),
      );
      return;
    }
    state = state.copyWith(
      busy: false,
      clearStage: true,
      lastSpec: spec,
      status: const UpdateStatus(kind: 'success', message: '已启动更新程序，应用将退出'),
    );
    // Runner waits on this PID, replaces files, then relaunches. Prefer the
    // real desktop lifecycle so the core is stopped and stats flushed before
    // the hand-off; fall back to the injected exit in tests / no-integration.
    await handoffExit();
  }

  /// Exit after a successful runner hand-off.
  ///
  /// When the real desktop integration is present it runs the bounded shutdown
  /// first (stop core -> drain/flush -> platform restore) so the runner does
  /// not replace files behind a live core holding them, then exits. Otherwise
  /// the injected [AppExit] is used unchanged.
  Future<void> handoffExit() async {
    final integration = ref.read(desktopIntegrationProvider).value;
    if (integration != null) {
      await integration.exitForUpdate();
    } else {
      _exitApp();
    }
  }
}
