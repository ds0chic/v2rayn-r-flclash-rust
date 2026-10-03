import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart' as m;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

final subsControllerProvider = NotifierProvider<SubsController, SubsState>(
  SubsController.new,
);

/// A structured status line for the subscription window / status bar.
class SubStatus {
  const SubStatus({required this.kind, required this.message, this.detail});

  /// `info` / `success` / `error` / `stage`.
  final String kind;
  final String message;
  final String? detail;

  bool get isError => kind == 'error';
  bool get isSuccess => kind == 'success';
}

class SubsState {
  const SubsState({
    required this.items,
    this.selectedId,
    this.busy = false,
    this.status,
    this.lastJobId,
    this.lastUpdatedMs,
  });

  final List<c.SubItemDto> items;
  final String? selectedId;
  final bool busy;
  final SubStatus? status;
  final String? lastJobId;
  final int? lastUpdatedMs;

  c.SubItemDto? get selected {
    for (final item in items) {
      if (item.id == selectedId) return item;
    }
    return null;
  }

  SubsState copyWith({
    List<c.SubItemDto>? items,
    String? selectedId,
    bool clearSelected = false,
    bool? busy,
    SubStatus? status,
    String? lastJobId,
    int? lastUpdatedMs,
  }) => SubsState(
    items: items ?? this.items,
    selectedId: clearSelected ? null : (selectedId ?? this.selectedId),
    busy: busy ?? this.busy,
    status: status ?? this.status,
    lastJobId: lastJobId ?? this.lastJobId,
    lastUpdatedMs: lastUpdatedMs ?? this.lastUpdatedMs,
  );
}

/// Subscription window controller (ACT-MAIN-019..023, F-SUB-001..011).
class SubsController extends Notifier<SubsState> {
  @override
  SubsState build() {
    return SubsState(items: _load());
  }

  c.SubsPageDto _page() => ref.read(bridgePortProvider).listSubItems();

  List<c.SubItemDto> _load() => _page().items;

  void reload() {
    state = state.copyWith(items: _load());
  }

  void select(String? id) {
    state = id == null
        ? state.copyWith(clearSelected: true)
        : state.copyWith(selectedId: id);
  }

  /// A fresh editable draft with the upstream defaults.
  c.SubItemDto newDraft() => c.SubItemDto(
    id: '',
    remarks: '',
    url: '',
    moreUrl: '',
    enabled: true,
    userAgent: '',
    sort: 0,
    autoUpdateInterval: 0,
    updateTime: 0,
  );

  /// Validate without persisting (used by the edit dialog).
  c.ErrorDto? validate(c.SubItemDto item) {
    final result = ref.read(bridgePortProvider).validateSubItem(item);
    return result.ok ? null : result.error;
  }

  c.SubItemDtoResult save(c.SubItemDto item) {
    final result = ref.read(bridgePortProvider).saveSubItem(item);
    if (result.ok) {
      reload();
      // Keep the top profile group chips in sync with the subscription list
      // (FIX-06): the toolbar rebuilds off the profiles controller, so a
      // saved plain group must refresh it, not only the subs state.
      ref.read(profilesControllerProvider.notifier).reload();
      state = state.copyWith(
        status: const SubStatus(kind: 'success', message: '订阅已保存'),
      );
    } else {
      state = state.copyWith(
        status: SubStatus(
          kind: 'error',
          message: '订阅保存失败',
          detail: result.error?.messageKey,
        ),
      );
    }
    return result;
  }

  c.DeleteSubsResult delete(List<String> ids) {
    final result = ref.read(bridgePortProvider).deleteSubItems(ids);
    if (result.ok) {
      reload();
      // Drop the deleted group chip from the top toolbar (FIX-06).
      ref.read(profilesControllerProvider.notifier).reload();
      state = state.copyWith(
        status: SubStatus(
          kind: 'success',
          message: '已删除 ${result.removed} 个订阅',
        ),
      );
    }
    return result;
  }

  void setEnabled(String id, bool enabled) {
    final result = ref.read(bridgePortProvider).setSubEnabled(id, enabled);
    if (result.ok) reload();
  }

  void reorder(List<String> ids) {
    ref.read(bridgePortProvider).reorderSubItems(ids);
    reload();
  }

  void setStatus(SubStatus status) {
    state = state.copyWith(status: status);
  }

  /// The F-SUB-003 update pipeline via an explicit run (job + cancellation).
  ///
  /// SET-03: the bridge mints and returns the real job id *before* the
  /// download starts, so [lastJobId] is set while the work is still running
  /// and [cancel] targets the live job. The terminal report is then resolved
  /// from the bound job (`job_view`) instead of being awaited blankly; a
  /// cancellation or failure never reports success and never replaces the
  /// old group (the Rust pipeline is candidate-first).
  Future<c.SubUpdateResult> update({
    List<String> subIds = const <String>[],
    bool viaProxy = false,
  }) async {
    state = state.copyWith(
      busy: true,
      status: SubStatus(
        kind: 'stage',
        message: viaProxy ? '正在通过代理更新订阅…' : '正在更新订阅…',
      ),
    );
    final bridge = ref.read(bridgePortProvider);
    final started = await bridge.updateSubscriptions(subIds, viaProxy);
    // Bind progress/cancel to the real job id immediately.
    if (started.jobId != null) {
      state = state.copyWith(lastJobId: started.jobId);
    }
    final result = _isStarted(started)
        ? await _awaitJob(bridge, started, subIds)
        : started;
    state = state.copyWith(
      busy: false,
      lastJobId: started.jobId ?? state.lastJobId,
      lastUpdatedMs: DateTime.now().millisecondsSinceEpoch,
      status: SubStatus(
        kind: result.ok ? 'success' : (result.cancelled ? 'info' : 'error'),
        message: _summarize(result),
        detail: _firstError(result),
      ),
    );
    reload();
    return result;
  }

  /// Whether the bridge accepted the run asynchronously and only returned the
  /// job id (empty entries, no request error).
  bool _isStarted(c.SubUpdateResult result) =>
      result.jobId != null && result.entries.isEmpty && result.error == null;

  bool _isTerminal(m.JobState state) =>
      state == m.JobState.done ||
      state == m.JobState.failed ||
      state == m.JobState.cancelled;

  /// Resolve a started job into a truthful terminal result. Node counts are
  /// derived from the engine before/after so a success is never invented.
  Future<c.SubUpdateResult> _awaitJob(
    BridgePort bridge,
    c.SubUpdateResult started,
    List<String> subIds,
  ) async {
    final jobId = started.jobId!;
    final before = _countsBySub(bridge);
    c.JobDto? job;
    final deadline = DateTime.now().add(const Duration(minutes: 10));
    do {
      job = bridge.jobView(jobId);
      if (job != null && _isTerminal(job.state)) break;
      await Future<void>.delayed(const Duration(milliseconds: 100));
    } while (DateTime.now().isBefore(deadline));
    final targets = _targets(subIds);
    if (job == null || !_isTerminal(job.state)) {
      return c.SubUpdateResult(
        ok: false,
        success: 0,
        cancelled: false,
        entries: <c.SubUpdateEntryDto>[
          for (final s in targets)
            c.SubUpdateEntryDto(
              subId: s.id,
              remarks: s.remarks,
              status: 'failed',
              code: 'E_UNAVAILABLE',
              message: 'error.sub_update_unconfirmed',
            ),
        ],
        jobId: jobId,
        error: const c.ErrorDto(
          code: 'E_UNAVAILABLE',
          messageKey: 'error.sub_update_unconfirmed',
          retryable: true,
        ),
      );
    }
    switch (job.state) {
      case m.JobState.cancelled:
        return c.SubUpdateResult(
          ok: false,
          success: 0,
          cancelled: true,
          entries: <c.SubUpdateEntryDto>[
            for (final s in targets)
              c.SubUpdateEntryDto(
                subId: s.id,
                remarks: s.remarks,
                status: 'cancelled',
              ),
          ],
          jobId: jobId,
        );
      case m.JobState.done:
        final after = _countsBySub(bridge);
        final entries = <c.SubUpdateEntryDto>[
          for (final s in targets)
            c.SubUpdateEntryDto(
              subId: s.id,
              remarks: s.remarks,
              status: 'updated',
              added: _positiveDelta(after[s.id], before[s.id]),
              existing: before[s.id] ?? 0,
            ),
        ];
        final updated = entries.length;
        return c.SubUpdateResult(
          ok: updated > 0,
          success: updated,
          cancelled: false,
          entries: entries,
          jobId: jobId,
        );
      default:
        final code = job.errorCode ?? 'E_UNAVAILABLE';
        final messageKey = job.errorMessageKey ?? 'error.sub_update_failed';
        return c.SubUpdateResult(
          ok: false,
          success: 0,
          cancelled: false,
          entries: <c.SubUpdateEntryDto>[
            for (final s in targets)
              c.SubUpdateEntryDto(
                subId: s.id,
                remarks: s.remarks,
                status: 'failed',
                code: code,
                message: messageKey,
              ),
          ],
          jobId: jobId,
          error: c.ErrorDto(
            code: code,
            messageKey: messageKey,
            retryable: false,
          ),
        );
    }
  }

  int _positiveDelta(int? after, int? before) {
    final a = after ?? 0;
    final b = before ?? 0;
    return a > b ? a - b : 0;
  }

  Map<String, int> _countsBySub(BridgePort bridge) {
    final counts = <String, int>{};
    for (final profile in bridge.queryAllProfiles()) {
      counts.update(profile.subid, (v) => v + 1, ifAbsent: () => 1);
    }
    return counts;
  }

  List<c.SubItemDto> _targets(List<String> subIds) {
    if (subIds.isNotEmpty) {
      return state.items.where((s) => subIds.contains(s.id)).toList();
    }
    return state.items.where((s) => s.enabled).toList();
  }

  /// Cancel the in-flight update job (idempotent). Calls through to the
  /// Rust job manager; the returned outcome is shown so a fake cancel
  /// cannot be mistaken for a real one.
  void cancel() {
    final jobId = state.lastJobId;
    if (jobId == null) return;
    final result = ref.read(bridgePortProvider).cancelJob(jobId);
    state = state.copyWith(
      status: SubStatus(kind: 'info', message: '已请求取消：${result.outcome.name}'),
    );
  }

  void startScheduler() {
    final result = ref.read(bridgePortProvider).startSubScheduler();
    if (result.ok) {
      state = state.copyWith(
        status: const SubStatus(kind: 'success', message: '定时更新已启动'),
      );
    }
  }

  void stopScheduler() {
    ref.read(bridgePortProvider).stopSubScheduler();
    state = state.copyWith(
      status: const SubStatus(kind: 'info', message: '定时更新已停止'),
    );
  }

  String _summarize(c.SubUpdateResult result) {
    if (result.cancelled) return '更新已取消';
    if (result.entries.isEmpty) {
      return result.error == null ? '没有可更新的订阅' : '更新未成功，旧节点已保留';
    }
    final updated = result.entries.where((e) => e.status == 'updated').length;
    final preserved = result.entries
        .where((e) => e.status.startsWith('preserved'))
        .length;
    final failed = result.entries
        .where((e) => e.status == 'failed' || e.status == 'preserved_error')
        .length;
    if (updated == 0 && failed > 0) {
      return '更新失败 $failed（旧节点已保留）';
    }
    final parts = <String>['成功 $updated'];
    if (preserved > 0) parts.add('保留旧节点 $preserved');
    if (failed > 0) parts.add('失败 $failed');
    return '更新完成：${parts.join('，')}';
  }

  String? _firstError(c.SubUpdateResult result) {
    for (final entry in result.entries) {
      if (entry.status == 'failed' || entry.status == 'preserved_error') {
        return '${entry.remarks}: ${entry.message ?? entry.code ?? ''}';
      }
    }
    // Request-level failures (e.g. E_PROXY_UNAVAILABLE when "via proxy" is
    // requested without a local proxy endpoint) carry no per-entry rows.
    final error = result.error;
    if (error != null) {
      return '${error.code}: ${error.messageKey}';
    }
    return null;
  }
}
