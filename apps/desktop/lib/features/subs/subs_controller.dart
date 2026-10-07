import 'dart:convert';

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
    this.selectedIds = const <String>[],
    this.selectionAnchorId,
    this.busy = false,
    this.status,
    this.lastJobId,
    this.lastUpdatedMs,
  });

  final List<c.SubItemDto> items;
  final String? selectedId;

  /// Multi-select set (upstream `SubSettingViewModel.SelectedSources`
  /// counterpart). `selectedId` stays the primary/last-tapped row (upstream
  /// `SelectedSource`); a plain tap collapses this to the single row, so
  /// single-select callers keep working unchanged.
  final List<String> selectedIds;

  /// Fixed end of a Shift-extended range (upstream DataGrid Extended
  /// selection anchor). Plain select / Ctrl+click move it; Shift+click and
  /// Shift+arrows extend from it without moving it, so repeated extends grow
  /// the range instead of drifting.
  final String? selectionAnchorId;
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

  /// Rows the delete path acts on: the multi-select set when non-empty,
  /// otherwise the single primary (upstream `SelectedSources ??
  /// [SelectedSource]`).
  List<String> get deleteIds => selectedIds.isNotEmpty
      ? List<String>.unmodifiable(selectedIds)
      : (selectedId == null ? const <String>[] : <String>[selectedId!]);

  SubsState copyWith({
    List<c.SubItemDto>? items,
    String? selectedId,
    List<String>? selectedIds,
    String? selectionAnchorId,
    bool clearSelected = false,
    bool? busy,
    SubStatus? status,
    String? lastJobId,
    int? lastUpdatedMs,
  }) => SubsState(
    items: items ?? this.items,
    selectedId: clearSelected ? null : (selectedId ?? this.selectedId),
    selectedIds: clearSelected
        ? const <String>[]
        : (selectedIds ?? this.selectedIds),
    selectionAnchorId: clearSelected
        ? null
        : (selectionAnchorId ?? this.selectionAnchorId),
    busy: busy ?? this.busy,
    status: status ?? this.status,
    lastJobId: lastJobId ?? this.lastJobId,
    lastUpdatedMs: lastUpdatedMs ?? this.lastUpdatedMs,
  );
}

/// Subscription window controller (ACT-MAIN-019..023, F-SUB-001..011).
class SubsController extends Notifier<SubsState> {
  /// Stage-key prefix carrying the terminal per-group report.
  ///
  /// `job_view` already round-trips `stage_key`, so the Rust bridge parks the
  /// backend report there for the same job id (SR-01). A dedicated
  /// `subUpdateReport` getter replaces this carrier once FRB is regenerated.
  static const String reportStagePrefix = 'subs.report:';

  @override
  SubsState build() {
    return SubsState(items: _load());
  }

  c.SubsPageDto _page() => ref.read(bridgePortProvider).listSubItems();

  List<c.SubItemDto> _load() => _page().items;

  void reload() {
    state = state.copyWith(items: _load());
    _pruneSelection();
  }

  void select(String? id) {
    if (id == null) {
      state = state.copyWith(clearSelected: true);
      return;
    }
    state = state.copyWith(
      selectedId: id,
      selectedIds: <String>[id],
      selectionAnchorId: id,
    );
  }

  /// Ctrl+click toggle for the multi-select set (upstream DataGrid
  /// Extended selection). Removing the primary falls back to the last
  /// remaining row; emptying the set clears the primary too.
  void toggleMultiSelected(String id) {
    final current = state.selectedIds.toList();
    if (current.contains(id)) {
      current.remove(id);
      state = current.isEmpty
          ? state.copyWith(clearSelected: true)
          : state.copyWith(
              selectedId: current.last,
              selectedIds: current,
              selectionAnchorId: current.last,
            );
    } else {
      state = state.copyWith(
        selectedId: id,
        selectedIds: <String>[...current, id],
        selectionAnchorId: id,
      );
    }
  }

  /// Shift+click range over the current item order (inclusive), anchored at
  /// [anchorId] (the primary before the shift press).
  void selectRange(String anchorId, String focusId) {
    final ids = [for (final s in state.items) s.id];
    final a = ids.indexOf(anchorId);
    final b = ids.indexOf(focusId);
    if (a < 0 || b < 0) {
      select(focusId);
      return;
    }
    final lo = a < b ? a : b;
    final hi = a < b ? b : a;
    final range = ids.sublist(lo, hi + 1);
    state = state.copyWith(
      selectedId: focusId,
      selectedIds: range,
      selectionAnchorId: anchorId,
    );
  }

  /// Ctrl+A parity (upstream DataGrid Extended selects all rows; the primary
  /// stays where it is). Empty list is a no-op.
  void selectAll() {
    if (state.items.isEmpty) return;
    final all = [for (final s in state.items) s.id];
    state = state.copyWith(
      selectedId: state.selectedId ?? all.last,
      selectedIds: all,
      selectionAnchorId: state.selectionAnchorId ?? state.selectedId,
    );
  }

  /// Plain arrow-key move (upstream DataGrid row navigation): collapse to the
  /// neighbour row. Clamps at the ends; with no selection, starts at the
  /// first row.
  void movePrimary(int direction) {
    if (state.items.isEmpty) return;
    final ids = [for (final s in state.items) s.id];
    final current = state.selectedId == null
        ? (direction < 0 ? 1 : -1)
        : ids.indexOf(state.selectedId!);
    final next = (current + direction).clamp(0, ids.length - 1);
    select(ids[next]);
  }

  /// Shift+arrow extend (upstream DataGrid Extended keyboard range): grow the
  /// range from the fixed anchor by one row. The anchor never moves here, so
  /// repeated extends accumulate instead of drifting.
  void extendKeyboardSelection(int direction) {
    if (state.items.isEmpty) return;
    final ids = [for (final s in state.items) s.id];
    final anchor =
        (state.selectionAnchorId != null &&
            ids.contains(state.selectionAnchorId))
        ? state.selectionAnchorId!
        : (state.selectedId != null && ids.contains(state.selectedId))
        ? state.selectedId!
        : ids.first;
    final focus = (state.selectedId != null && ids.contains(state.selectedId))
        ? state.selectedId!
        : anchor;
    final next = (ids.indexOf(focus) + direction).clamp(0, ids.length - 1);
    selectRange(anchor, ids[next]);
  }

  /// Drop selected ids that no longer exist (deleted rows, reload races).
  /// Only ever shrinks the selection, never invents one.
  void _pruneSelection() {
    final live = {for (final s in state.items) s.id};
    final anchorLive =
        state.selectionAnchorId != null &&
        live.contains(state.selectionAnchorId);
    if (state.selectedIds.any((id) => !live.contains(id)) ||
        (state.selectedId != null && !live.contains(state.selectedId)) ||
        (state.selectionAnchorId != null && !anchorLive)) {
      final kept = state.selectedIds.where(live.contains).toList();
      state = kept.isEmpty
          ? state.copyWith(clearSelected: true)
          : state.copyWith(
              selectedId: kept.contains(state.selectedId)
                  ? state.selectedId
                  : kept.last,
              selectedIds: kept,
              selectionAnchorId: anchorLive
                  ? state.selectionAnchorId
                  : (kept.contains(state.selectedId)
                        ? state.selectedId
                        : kept.last),
            );
    }
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
      // SP-16: then re-resolve the current group (RefreshSubscriptions
      // parity — a created/edited group shows up, the current hit is kept,
      // never auto-switched to new).
      final profiles = ref.read(profilesControllerProvider.notifier);
      profiles.reload();
      profiles.resyncGroupFromSubs();
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
      // SP-16: then fall back a dangling current group to All in memory
      // (RefreshSubscriptions parity — the repair is never persisted).
      final profiles = ref.read(profilesControllerProvider.notifier);
      profiles.reload();
      profiles.resyncGroupFromSubs();
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

  /// Resolve a started job into a truthful terminal result from the backend's
  /// per-group report; a success is never invented from the Done state.
  Future<c.SubUpdateResult> _awaitJob(
    BridgePort bridge,
    c.SubUpdateResult started,
    List<String> subIds,
  ) async {
    final jobId = started.jobId!;
    c.JobDto? job;
    final deadline = DateTime.now().add(const Duration(minutes: 10));
    do {
      job = bridge.jobView(jobId);
      if (job != null && _isTerminal(job.state)) break;
      await Future<void>.delayed(const Duration(milliseconds: 100));
    } while (DateTime.now().isBefore(deadline));
    final targets = _targets(subIds);
    if (job == null || !_isTerminal(job.state)) {
      return _unconfirmed(jobId, targets);
    }
    // SR-01: trust the backend's per-group report for this exact job id.
    // Never reconstruct "all updated" from the Done state.
    final reported = _reportFromJob(job);
    if (reported != null) return reported;
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
        // Terminal but no report: the truth is unprovable, so say so instead
        // of inventing per-group success.
        return _unconfirmed(jobId, targets);
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

  /// Decode the backend report parked on the terminal job's stage key.
  ///
  /// Returns `null` when no report is present or it is malformed, so callers
  /// fall back to explicit (never fabricated) terminal handling.
  c.SubUpdateResult? _reportFromJob(c.JobDto job) {
    final stage = job.stageKey;
    if (stage == null || !stage.startsWith(reportStagePrefix)) return null;
    try {
      final decoded = jsonDecode(stage.substring(reportStagePrefix.length));
      if (decoded is! Map) return null;
      final rawEntries = decoded['entries'];
      if (rawEntries is! List) return null;
      final entries = <c.SubUpdateEntryDto>[
        for (final raw in rawEntries)
          if (raw is Map)
            c.SubUpdateEntryDto(
              subId: '${raw['sub_id'] ?? ''}',
              remarks: '${raw['remarks'] ?? ''}',
              status: '${raw['status'] ?? 'failed'}',
              added: raw['added'] is int ? raw['added'] as int : null,
              existing: raw['existing'] is int ? raw['existing'] as int : null,
              code: raw['code'] is String ? raw['code'] as String : null,
              message: raw['message'] is String
                  ? raw['message'] as String
                  : null,
            ),
      ];
      final cancelled =
          decoded['cancelled'] == true ||
          entries.any((e) => e.status == 'cancelled');
      final updated = entries.where((e) => e.status == 'updated').length;
      return c.SubUpdateResult(
        ok: !cancelled && updated > 0,
        success: updated,
        cancelled: cancelled,
        entries: entries,
        jobId: job.jobId,
        error: (!cancelled && updated == 0)
            ? c.ErrorDto(
                code: job.errorCode ?? 'E_UNAVAILABLE',
                messageKey: job.errorMessageKey ?? 'error.sub_update_failed',
                retryable: false,
              )
            : null,
      );
    } on FormatException {
      return null;
    }
  }

  c.SubUpdateResult _unconfirmed(String jobId, List<c.SubItemDto> targets) =>
      c.SubUpdateResult(
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

  List<c.SubItemDto> _targets(List<String> subIds) {
    final selected = subIds.isNotEmpty
        ? state.items.where((s) => subIds.contains(s.id))
        : state.items.where((s) => s.enabled);
    // Empty-URL groups are plain groups: they are never download targets.
    return selected.where((s) => s.url.trim().isNotEmpty).toList();
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

  /// FIX-09D normal-launch hook: start the periodic subscription updater.
  ///
  /// Independent of any test environment (not armed by an env var); idempotent
  /// so a repeated bootstrap or hot reload never spawns a second timer. Silent
  /// when already running so it does not overwrite startup status.
  void startScheduler({bool silent = false}) {
    final bridge = ref.read(bridgePortProvider);
    if (bridge.subSchedulerRunning()) return;
    final result = bridge.startSubScheduler();
    if (result.ok && !silent) {
      state = state.copyWith(
        status: const SubStatus(kind: 'success', message: '定时更新已启动'),
      );
    }
  }

  /// FIX-09D shutdown hook: stop the updater so no timer survives exit.
  ///
  /// Idempotent; a no-op when the scheduler is not running.
  void stopScheduler({bool silent = false}) {
    final bridge = ref.read(bridgePortProvider);
    if (!bridge.subSchedulerRunning()) return;
    bridge.stopSubScheduler();
    if (!silent) {
      state = state.copyWith(
        status: const SubStatus(kind: 'info', message: '定时更新已停止'),
      );
    }
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
    final skipped = result.entries.where((e) => e.status == 'skipped').length;
    final failed = result.entries
        .where((e) => e.status == 'failed' || e.status == 'preserved_error')
        .length;
    if (updated == 0) {
      if (failed > 0) return '更新失败 $failed（旧节点已保留）';
      if (skipped > 0) return '已跳过 $skipped 个分组（无 URL 或已禁用）';
      return '没有可更新的订阅';
    }
    final parts = <String>['成功 $updated'];
    if (skipped > 0) parts.add('跳过 $skipped');
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
