import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

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
    this.viaProxy = false,
    this.busy = false,
    this.stage,
    this.checks = const <c.CoreUpdateDto>[],
    this.status,
    this.lastSpec,
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
    return UpdateState(
      targets: targets,
      selected: <String>{
        for (final target in targets)
          if (target.supported) target.core,
      },
    );
  }

  void toggleCore(String core, bool selected) {
    final next = <String>{...state.selected};
    if (selected) {
      next.add(core);
    } else {
      next.remove(core);
    }
    state = state.copyWith(selected: next);
  }

  void setPrerelease(bool value) => state = state.copyWith(prerelease: value);

  void setViaProxy(bool value) => state = state.copyWith(viaProxy: value);

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

  Future<void> checkOnly() async {
    if (state.busy) return;
    state = state.copyWith(
      busy: true,
      stage: '正在检查更新…',
      checks: const <c.CoreUpdateDto>[],
    );
    final result = await ref
        .read(bridgePortProvider)
        .t16CheckUpdates(_selectedCores, state.prerelease, state.viaProxy);
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
    final check = await ref
        .read(bridgePortProvider)
        .t16CheckUpdates(_selectedCores, state.prerelease, state.viaProxy);
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
    final apply = await ref
        .read(bridgePortProvider)
        .t16ApplyCoreUpdate(_selectedCores, state.prerelease, state.viaProxy);
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

  /// Stage the application update, then actually launch the external runner
  /// and exit so it can replace the flat install root and relaunch (RR-04).
  ///
  /// The spec is produced by `t16_apply_app_update_spec`; we never fabricate a
  /// success — a missing helper or a failed spawn is reported, not hidden.
  Future<void> applyAppUpdate() async {
    if (state.busy) return;
    state = state.copyWith(busy: true, stage: '正在准备应用自身更新…');
    final spec = await ref.read(bridgePortProvider).t16ApplyAppUpdateSpec();
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
    // Runner waits on this PID, replaces files, then relaunches.
    _exitApp();
  }
}
