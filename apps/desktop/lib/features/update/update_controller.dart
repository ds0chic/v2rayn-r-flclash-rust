import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

final updateControllerProvider =
    NotifierProvider<UpdateController, UpdateState>(UpdateController.new);

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
    state = state.copyWith(
      busy: false,
      clearStage: true,
      checks: result.checks,
      status: UpdateStatus(
        kind: updates > 0 ? 'success' : 'info',
        message: updates > 0 ? '发现 $updates 个可更新目标' : '全部为最新版本',
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

  Future<void> stageAppUpdateSpec() async {
    state = state.copyWith(busy: true, stage: '正在准备应用自身更新…');
    final spec = await ref.read(bridgePortProvider).t16ApplyAppUpdateSpec();
    state = state.copyWith(
      busy: false,
      clearStage: true,
      lastSpec: spec,
      status: spec.ok
          ? const UpdateStatus(kind: 'info', message: '应用更新需要外部进程执行（本程序不执行替换）')
          : UpdateStatus(
              kind: 'error',
              message: '应用更新不可用',
              detail: _detail(spec.error),
            ),
    );
  }
}
