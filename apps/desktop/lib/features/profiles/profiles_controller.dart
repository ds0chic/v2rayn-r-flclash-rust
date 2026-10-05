import 'dart:async';
import 'dart:convert';
import 'dart:math' show max;

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/groups.dart' as groups;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/api/speedtest.dart' as speedtest;
import 'package:v2rayn_desktop/features/profiles/profile_dedup.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';
import 'package:v2rayn_desktop/features/profiles/profile_fields.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

final bridgePortProvider = Provider<BridgePort>((ref) => const FrbBridgePort());

final uiStateStoreProvider = Provider<UiStateStore>(
  (ref) => FileUiStateStore(),
);

/// Whether the node table registers row drag-reorder, read from the persisted
/// `UiItem.EnableDragDropSort` (FLD-CFG-076 / upstream `ProfilesView.xaml:27-34`).
///
/// Upstream only wires the drag handlers when this is true; when false a row
/// has no `Draggable`/`DragTarget` at all. Tests override this provider instead
/// of mutating the locked settings store.
final profilesEnableDragDropSortProvider = Provider<bool>(
  (ref) => readEnableDragDropSort(ref.read(bridgePortProvider)),
);

/// Resolve `UiItem.EnableDragDropSort` from the settings document, defaulting to
/// false when settings are unavailable or malformed.
@visibleForTesting
bool readEnableDragDropSort(BridgePort bridge) {
  try {
    final load = bridge.getSettings();
    if (!load.ok || load.settingsJson.isEmpty) return false;
    final decoded = jsonDecode(load.settingsJson);
    if (decoded is Map<String, dynamic>) {
      final ui = decoded['UiItem'];
      if (ui is Map<String, dynamic>) return ui['EnableDragDropSort'] == true;
    }
  } catch (_) {
    // No native library (pure widget tests) or malformed settings.
  }
  return false;
}

/// Resolve `GuiItem.KeepOlderDedupl` from the settings document.
///
/// Frozen `ConfigHandler.DedupServerList` (`:1166-1168`) reverses the list when
/// the flag is false, so the *newer* entry wins. The flag defaults to false in
/// the frozen `ConfigItems.KeepOlderDedupl`; when settings are unavailable the
/// conservative `true` (keep the older entry) is used so a transient read
/// failure can never silently delete data.
@visibleForTesting
bool readKeepOlderDedupl(BridgePort bridge) {
  try {
    final load = bridge.getSettings();
    if (!load.ok || load.settingsJson.isEmpty) return true;
    final decoded = jsonDecode(load.settingsJson);
    if (decoded is Map<String, dynamic>) {
      final gui = decoded['GuiItem'];
      if (gui is Map<String, dynamic>) {
        final value = gui['KeepOlderDedupl'];
        if (value is bool) return value;
      }
    }
  } catch (_) {
    // No native library (pure widget tests) or malformed settings.
  }
  return true;
}

final profileRowCountProvider = Provider<int>((ref) => 10000);

final profilesControllerProvider =
    NotifierProvider<ProfilesController, ProfilesState>(ProfilesController.new);

class TableEvent {
  const TableEvent({
    required this.seq,
    required this.action,
    required this.detail,
  });

  final int seq;
  final String action;
  final String detail;
}

/// Outcome of `移除重复` (ACT-PROF-003). Distinguishes "no duplicates found"
/// from a real delete failure and records whether the active node was removed,
/// so the caller can show the true error and run the active-node fallback
/// (R3-PROF-04).
class DedupOutcome {
  const DedupOutcome({
    required this.ok,
    this.removed = 0,
    required this.hadDuplicates,
    this.activeRemoved = false,
    this.errorCode,
    this.errorMessageKey,
  });

  final bool ok;
  final int removed;
  final bool hadDuplicates;
  final bool activeRemoved;
  final String? errorCode;
  final String? errorMessageKey;
}

/// Outcome of resolving the explicit-start target (R4-02).
///
/// [target] is the node the toolbar must apply, frozen at click time. When the
/// resolved target differed from the persisted default, [changed] is true and
/// it has already been persisted (with a desired-revision bump on the Rust
/// side). A persist failure leaves the previous default untouched and reports
/// [errorCode] instead.
class StartTargetOutcome {
  const StartTargetOutcome({
    required this.target,
    required this.persisted,
    this.changed = false,
    this.errorCode,
  });

  final String? target;
  final bool persisted;
  final bool changed;
  final String? errorCode;
}

/// Effective configuration for one speedtest run.
///
/// Values are read from the persisted `SpeedTestItem` at start time (so a
/// user-configured URL/timeout is really used, ISSUE T15b-URL) and fall back to
/// the upstream defaults when settings are unavailable (pure widget tests).
class SpeedTestConfig {
  const SpeedTestConfig({
    this.pageSize = 1000,
    this.mixedConcurrency = 10,
    this.timeoutSecs = 10,
    this.speedTestUrl = 'https://cachefly.cachefly.net/50mb.test',
    this.speedPingTestUrl = 'https://www.gstatic.com/generate_204',
    this.ipapiUrl,
    this.udpTestTarget,
    this.delayIntervalSecs = 1,
  });

  final int pageSize;
  final int mixedConcurrency;
  final int timeoutSecs;
  final String speedTestUrl;
  final String speedPingTestUrl;
  final String? ipapiUrl;
  final String? udpTestTarget;
  final int delayIntervalSecs;
}

class ProfilesState {
  const ProfilesState({
    required this.all,
    required this.visible,
    required this.filter,
    this.filterInput = '',
    required this.sort,
    required this.selected,
    required this.events,
    required this.doubleClick2Activate,
    required this.rustCount,
    required this.columns,
    this.profiles = const <c.ProfileDto>[],
    this.primaryId,
    this.activeId,
    this.groupSubId,
    this.lastAckSeq,
    this.lastAckRustCount,
    this.blockingBusy = false,
    this.lastBlockingMs,
    this.speedTestJobId,
    this.speedTestKind,
    this.speedTestRunning = false,
    this.speedTestStage = '',
    this.speedTestMessage,
    this.speedTestGeneration = 0,
    this.orderMessage,
  });

  final List<ProfileSummary> all;
  final List<ProfileSummary> visible;

  /// Committed text filter applied to [visible] (upstream `_serverFilter`).
  final String filter;

  /// Raw text currently typed in the filter box, which may be ahead of
  /// [filter] until Enter commits it (upstream binds the box to `ServerFilter`
  /// but only refreshes on Enter/clear).
  final String filterInput;
  final SortSpec sort;

  /// Batch selection (Ctrl/Shift/marquee). Kept separate from [primaryId].
  final Set<String> selected;

  /// The independent "current row" / main target, mirroring upstream
  /// `DataGrid.SelectedItem` (`SelectedProfile`). Single-object commands
  /// (edit / share / set-active / full-config export) use this instead of
  /// requiring `selected.length == 1`, so a multi-selection still has a main
  /// row to act on (R3-PROF-01). Null when no row is current/visible.
  final String? primaryId;

  final List<TableEvent> events;
  final bool doubleClick2Activate;
  final int rustCount;

  /// Full stored profiles (editor + batch actions). Loaded on demand.
  final List<c.ProfileDto> profiles;

  /// Persisted active node id, if any.
  final String? activeId;

  /// Selected subscription group filter (`null` = all groups).
  final String? groupSubId;

  /// Full column set in display order; hidden columns are retained so their
  /// position/width survives toggling (LAY-PROFILES-003).
  final List<ProfileColumn> columns;
  final int? lastAckSeq;
  final int? lastAckRustCount;
  final bool blockingBusy;
  final int? lastBlockingMs;

  /// Active speedtest job (T15b). `speedTestStage` carries the real stage key
  /// (`Speedtesting` / `SpeedtestingCompleted` / `SpeedtestingStop`); no
  /// synthetic percentage is ever produced.
  final String? speedTestJobId;
  final int? speedTestKind;
  final bool speedTestRunning;
  final String speedTestStage;
  final String? speedTestMessage;

  /// Monotonic speedtest run generation. Bumped on every start and cancel so a
  /// poller/completion belonging to an older run can be detected and dropped
  /// instead of settling or summarizing a newer run (R4-22).
  final int speedTestGeneration;

  /// Last drag/header/result order-persistence failure, shown in the status
  /// line (`ProfileExItem.Sort` write). Cleared by the next successful write.
  final String? orderMessage;

  int get selectedCount => selected.length;
  int get totalCount => all.length;
  TableEvent? get lastEvent => events.isEmpty ? null : events.last;

  List<ProfileColumn> get visibleColumns =>
      columns.where((c) => c.visible).toList();

  ProfilesState copyWith({
    List<ProfileSummary>? all,
    List<ProfileSummary>? visible,
    String? filter,
    String? filterInput,
    SortSpec? sort,
    Set<String>? selected,
    List<TableEvent>? events,
    bool? doubleClick2Activate,
    int? rustCount,
    List<ProfileColumn>? columns,
    List<c.ProfileDto>? profiles,
    String? primaryId,
    bool clearPrimary = false,
    String? activeId,
    bool clearActive = false,
    String? groupSubId,
    bool clearGroup = false,
    int? lastAckSeq,
    int? lastAckRustCount,
    bool? blockingBusy,
    int? lastBlockingMs,
    String? speedTestJobId,
    int? speedTestKind,
    bool? speedTestRunning,
    String? speedTestStage,
    String? speedTestMessage,
    int? speedTestGeneration,
    String? orderMessage,
    bool clearOrderMessage = false,
    bool clearSpeedTestJob = false,
  }) {
    return ProfilesState(
      all: all ?? this.all,
      visible: visible ?? this.visible,
      filter: filter ?? this.filter,
      filterInput: filterInput ?? this.filterInput,
      sort: sort ?? this.sort,
      selected: selected ?? this.selected,
      events: events ?? this.events,
      doubleClick2Activate: doubleClick2Activate ?? this.doubleClick2Activate,
      rustCount: rustCount ?? this.rustCount,
      columns: columns ?? this.columns,
      profiles: profiles ?? this.profiles,
      primaryId: clearPrimary ? null : (primaryId ?? this.primaryId),
      activeId: clearActive ? null : (activeId ?? this.activeId),
      groupSubId: clearGroup ? null : (groupSubId ?? this.groupSubId),
      lastAckSeq: lastAckSeq ?? this.lastAckSeq,
      lastAckRustCount: lastAckRustCount ?? this.lastAckRustCount,
      blockingBusy: blockingBusy ?? this.blockingBusy,
      lastBlockingMs: lastBlockingMs ?? this.lastBlockingMs,
      speedTestJobId: clearSpeedTestJob
          ? null
          : (speedTestJobId ?? this.speedTestJobId),
      speedTestKind: speedTestKind ?? this.speedTestKind,
      speedTestRunning: speedTestRunning ?? this.speedTestRunning,
      speedTestStage: speedTestStage ?? this.speedTestStage,
      speedTestMessage: speedTestMessage ?? this.speedTestMessage,
      speedTestGeneration: speedTestGeneration ?? this.speedTestGeneration,
      orderMessage: clearOrderMessage
          ? null
          : (orderMessage ?? this.orderMessage),
    );
  }
}

class ProfilesController extends Notifier<ProfilesState> {
  static const columnSection = 'column_layout';

  BridgePort get _bridge => ref.read(bridgePortProvider);
  UiStateStore get _store => ref.read(uiStateStoreProvider);

  int _seq = 0;
  Timer? _testPoller;

  /// Monotonic speedtest run generation (see [ProfilesState.speedTestGeneration]).
  int _speedTestGeneration = 0;

  /// Ordered base rows (before the live speedtest/statistics overlay) from the
  /// last structural read. The 150 ms poll re-applies the overlay onto this
  /// cached base instead of re-reading the whole profile table (D08).
  List<ProfileSummary> _baseSummaries = const <ProfileSummary>[];

  /// Latest accepted table-query generation. A newer reload/search bumps it so
  /// a late (older) page result can never overwrite a newer one.
  int _queryGeneration = 0;

  /// Job ids of every run started but not yet confirmed finished. Upstream
  /// `SpeedtestService.ExitLoop` cancels *all* in-flight runs, so the controller
  /// tracks each real job id (never just the latest) and a Stop binds to all of
  /// them instead of leaking an unstoppable job (R4-22).
  final Set<String> _activeSpeedTestJobs = <String>{};

  /// Direction for "按测试结果排序" (DelayVal). Upstream reuses the header sort
  /// toggle, so repeated invocations flip ascending/descending. Failed and
  /// untested rows always sink to the bottom regardless of direction.
  bool _resultSortAscending = true;

  @override
  ProfilesState build() {
    final count = ref.read(profileRowCountProvider);
    final snapshot = _bridge.fetchProfileSnapshot(count);
    _baseSummaries = snapshot.summaries;
    final rows = _bridge.applyLiveOverlay(snapshot.summaries);
    final columns = _applyStoredLayout(defaultProfileColumns());
    ref.onDispose(() => _testPoller?.cancel());
    return ProfilesState(
      all: rows,
      visible: List<ProfileSummary>.of(rows),
      filter: '',
      sort: const SortSpec(),
      selected: const <String>{},
      events: const <TableEvent>[],
      doubleClick2Activate: false,
      rustCount: _bridge.rustProfileCount(),
      columns: columns,
      profiles: snapshot.profiles,
      activeId: _bridge.getActiveProfile(),
    );
  }

  /// While a run is active, poll the Rust `ProfileExItem` store and refresh the
  /// table. The Rust side still coalesces results into 50-100 ms batches; the UI
  /// simply reads the closure at a bounded cadence and never fabricates a
  /// percentage.
  ///
  /// The poller captures the run [generation] and its [targets]. A tick whose
  /// generation is no longer current (a newer start/cancel bumped the counter)
  /// stops without touching state, so a stale poller can neither settle nor
  /// summarize a newer run (R4-22). [targets] is the exact frozen scope of that
  /// run, so the completion verdict never summarizes another run's nodes.
  void _startTestPolling(int generation, List<String> targets) {
    _testPoller?.cancel();
    _testPoller = Timer.periodic(const Duration(milliseconds: 150), (timer) {
      if (generation != _speedTestGeneration) {
        timer.cancel();
        if (identical(_testPoller, timer)) _testPoller = null;
        return;
      }
      _refreshLive();
      if (_bridge.speedTestActiveJobs() == 0) {
        timer.cancel();
        if (identical(_testPoller, timer)) _testPoller = null;
        _activeSpeedTestJobs.clear();
        final cancelled = state.speedTestStage == 'SpeedtestingStop';
        state = state.copyWith(
          speedTestRunning: false,
          speedTestStage: cancelled
              ? 'SpeedtestingStop'
              : 'SpeedtestingCompleted',
          speedTestMessage: cancelled ? '已停止测速' : _summarizeSpeedTest(targets),
        );
      }
    });
  }

  /// Build a user-facing verdict for a settled run: how many nodes succeeded,
  /// failed (with the first real reason) or never produced a result.
  String _summarizeSpeedTest(List<String> targets) {
    if (targets.isEmpty) return '测速完成：没有可测试节点';
    final byId = <String, speedtest.SpeedTestResultDto>{
      for (final r in _bridge.speedTestResults()) r.indexId: r,
    };
    var success = 0;
    var failed = 0;
    var unknown = 0;
    String? firstFailure;
    for (final id in targets) {
      final result = byId[id];
      if (result == null || result.delay == 0) {
        unknown++;
      } else if (result.delay > 0) {
        success++;
      } else {
        failed++;
        firstFailure ??= _failureReason(result.message);
      }
    }
    final parts = <String>[
      if (success > 0) '成功 $success',
      if (failed > 0) '失败 $failed',
      if (unknown > 0) '未完成 $unknown',
    ];
    final base = parts.isEmpty ? '无结果' : parts.join('，');
    if (failed > 0 && success == 0 && firstFailure != null) {
      return '测速完成：$base（$firstFailure）';
    }
    return '测速完成：$base';
  }

  /// The runner reuses the `Speedtesting` progress text as a placeholder when a
  /// real ping times out without an error code; never surface that as a reason.
  /// Structured probe failures arrive as stable `speedtest.*` keys (HTTPS/TLS
  /// classification, UX-TEST-02) and are mapped to a human reason here.
  String _failureReason(String message) {
    final trimmed = message.trim();
    if (trimmed.isEmpty || trimmed == 'Speedtesting') return '连接失败';
    const reasons = <String, String>{
      'speedtest.resolve_failed': '域名解析失败',
      'speedtest.connect_failed': '连接失败',
      'speedtest.timeout': '请求超时',
      'speedtest.tls_failed': 'TLS 证书校验失败',
      'speedtest.http_status': 'HTTP 状态错误',
      'speedtest.protocol_error': '协议错误',
      'speedtest.cancelled': '已取消',
      'error.speedtest_url': '测速地址无效',
      'error.test_session_unreachable': '测试内核不可用',
      'error.port_conflict': '测试端口冲突',
    };
    return reasons[trimmed] ??
        (trimmed.startsWith('speedtest.') ? '连接失败' : trimmed);
  }

  /// Read the persisted `SpeedTestItem` from the engine, falling back to the
  /// upstream defaults. Reading directly (instead of via the settings feature)
  /// keeps this oriented around the bridge and avoids an import cycle.
  SpeedTestConfig _resolveSpeedTestConfig() {
    var config = const SpeedTestConfig();
    try {
      final load = _bridge.getSettings();
      if (load.ok && load.settingsJson.isNotEmpty) {
        final decoded = jsonDecode(load.settingsJson);
        if (decoded is Map<String, dynamic>) {
          final item = decoded['SpeedTestItem'];
          if (item is Map<String, dynamic>) {
            int? asInt(String key) => (item[key] as num?)?.toInt();
            String? asText(String key) {
              final value = item[key] as String?;
              final trimmed = value?.trim();
              return (trimmed == null || trimmed.isEmpty) ? null : trimmed;
            }

            config = SpeedTestConfig(
              pageSize: asInt('SpeedTestPageSize') ?? config.pageSize,
              mixedConcurrency:
                  asInt('MixedConcurrencyCount') ?? config.mixedConcurrency,
              timeoutSecs: asInt('SpeedTestTimeout') ?? config.timeoutSecs,
              speedTestUrl: asText('SpeedTestUrl') ?? config.speedTestUrl,
              speedPingTestUrl:
                  asText('SpeedPingTestUrl') ?? config.speedPingTestUrl,
              ipapiUrl: asText('IPAPIUrl') ?? config.ipapiUrl,
              udpTestTarget: asText('UdpTestTarget') ?? config.udpTestTarget,
              delayIntervalSecs:
                  asInt('SpeedTestDelayInterval') ?? config.delayIntervalSecs,
            );
          }
        }
      }
    } catch (_) {
      // No native library (pure widget tests) or malformed settings.
    }
    return config;
  }

  // -- T06a profile editor / repository actions ---------------------------

  /// A fresh draft for a new node of [configType].
  ///
  /// The draft inherits the currently selected group (upstream
  /// `AddServerAsync` -> `ProfileItem{Subid = _config.SubIndexId}`), so a node
  /// added while a group is visible lands in that group instead of "no group".
  ProfileDraft newDraft(ConfigType configType) {
    return ProfileDraft()
      ..configType = configType
      ..coreType = ProfileCapabilities.defaultCore(configType)
      ..subid = state.groupSubId ?? ''
      ..remarks = ''
      ..address = ''
      ..port = 443
      ..network = 'raw';
  }

  c.ProfileDto? profileById(String id) {
    for (final p in state.profiles) {
      if (p.indexId == id) return p;
    }
    return _bridge.getProfile(id);
  }

  /// Reload stored profiles and the node table from the backend.
  ///
  /// The visible set is recomputed with the same group + text filter as
  /// [_recompute] (upstream `RefreshServers` resolves the current `SubIndexId`
  /// and filter), so a save/delete/refresh after a group switch cannot leak
  /// hidden nodes from another group into the visible set (PR-18).
  void reload() {
    final count = ref.read(profileRowCountProvider);
    // One structural read of the store: the ordered base rows and the full
    // DTOs come from the same cursor-following pass, so a reload no longer
    // queries the whole table twice (D08/D09).
    final snapshot = _bridge.fetchProfileSnapshot(count);
    _baseSummaries = snapshot.summaries;
    final rows = _bridge.applyLiveOverlay(snapshot.summaries);
    final active = _bridge.getActiveProfile();
    _queryGeneration++;
    state = _recompute(
      state.copyWith(
        all: rows,
        profiles: snapshot.profiles,
        activeId: active,
        clearActive: active == null,
      ),
    );
    _log('reload', 'profiles=${state.profiles.length} rows=${rows.length}');
  }

  /// Incremental refresh used by the speedtest poll: re-join the live result +
  /// statistics overlay onto the cached base rows without re-reading the
  /// profile table. Results are read fresh every tick, so none are lost.
  ///
  /// The poll never performs a structural full read: a node added/removed while
  /// a run is active is picked up by the event-driven [reload], which also
  /// refreshes `_baseSummaries`. An idle tick whose overlay produced the same
  /// values skips the O(n log n) filter+sort instead of rebuilding the whole
  /// read model every 150 ms (D08); a real result/statistics change still
  /// rebuilds.
  void _refreshLive() {
    if (_baseSummaries.isEmpty) return;
    final rows = _bridge.applyLiveOverlay(_baseSummaries);
    if (_overlayUnchanged(rows, state.all)) return;
    state = _recompute(state.copyWith(all: rows));
  }

  /// Cheap value check of the fields [BridgePort.applyLiveOverlay] can change.
  /// A tick with no new delay/speed/IP/statistics must not churn the table.
  static bool _overlayUnchanged(
    List<ProfileSummary> next,
    List<ProfileSummary> previous,
  ) {
    if (identical(next, previous)) return true;
    if (next.length != previous.length) return false;
    for (var i = 0; i < next.length; i++) {
      final a = next[i];
      final b = previous[i];
      if (a.id != b.id ||
          a.delay != b.delay ||
          a.speed != b.speed ||
          a.ipInfo != b.ipInfo ||
          a.todayUp != b.todayUp ||
          a.todayDown != b.todayDown ||
          a.totalUp != b.totalUp ||
          a.totalDown != b.totalDown) {
        return false;
      }
    }
    return true;
  }

  /// Async search entry with a generation guard: a newer query bumps the
  /// generation, so an older in-flight result is dropped instead of
  /// overwriting the latest one (the "old search must not cover new results"
  /// contract). The bridge read itself stays synchronous; the guard is what
  /// makes a future async page loader safe.
  Future<void> search(String query) async {
    final generation = ++_queryGeneration;
    await Future<void>.value();
    if (generation != _queryGeneration) return;
    state = _recompute(state.copyWith(filter: query, filterInput: query));
  }

  /// Persist a draft through the real bridge (optimistic revision).
  ///
  /// A brand-new node is selected after the refresh (`_pendingSelectIndexId`
  /// semantics, upstream `ProfilesViewModel.RefreshServersBiz:366-375`), so an
  /// add does not leave the table without a current row (R3-PROF-10).
  c.SaveProfileResult saveDraft(c.ProfileDto draft) {
    final isNew = draft.indexId.trim().isEmpty;
    final result = _bridge.saveProfile(draft, _bridge.profileRevision());
    if (result.ok) {
      reload();
      if (isNew) {
        _selectDefaultRow(pendingId: result.profile?.indexId);
      }
      _log('save-profile', 'id=${result.profile?.indexId ?? draft.indexId}');
    } else {
      _log('save-profile-failed', result.error?.code ?? 'unknown');
    }
    return result;
  }

  /// Default the current row after a refresh that left the table non-empty:
  /// pending (just-saved/generated) node, then the active node, then the first
  /// row. Faithful port of upstream
  /// `ProfilesViewModel.RefreshServersBiz:366-375`.
  void _selectDefaultRow({String? pendingId}) {
    final rows = state.visible;
    if (rows.isEmpty) return;
    final ids = rows.map((r) => r.id).toSet();
    String? target;
    if (pendingId != null && ids.contains(pendingId)) {
      target = pendingId;
    }
    if (target == null &&
        state.activeId != null &&
        ids.contains(state.activeId)) {
      target = state.activeId;
    }
    target ??= rows.first.id;
    state = state.copyWith(selected: <String>{target}, primaryId: target);
    _log('default-select', 'id=$target');
  }

  c.DeleteProfilesResult deleteSelected() {
    final result = _bridge.deleteProfiles(state.selected.toList());
    if (result.ok) {
      final removed = state.selected;
      state = state.copyWith(selected: const <String>{});
      reload();
      _log('delete', 'removed=${result.removed} ids=${removed.length}');
    }
    return result;
  }

  c.CopyProfilesResult copySelected() {
    final result = _bridge.copyProfiles(state.selected.toList());
    if (result.ok) {
      reload();
      _log('copy', 'copies=${result.copies.length}');
    }
    return result;
  }

  c.SaveProfileResult renameProfile(String id, String remarks) {
    final result = _bridge.setProfileRemarks(id, remarks);
    if (result.ok) {
      reload();
      _log('remarks', 'id=$id');
    }
    return result;
  }

  c.SimpleResult setActive(String? id) {
    final result = _bridge.setActiveProfile(id);
    if (result.ok) {
      state = state.copyWith(activeId: id, clearActive: id == null);
      _log('set-active', 'id=${id ?? "(none)"}');
    }
    return result;
  }

  /// Resolve the explicit-start target frozen at click time (R4-02).
  ///
  /// Precedence mirrors [resolveSingleTarget]: explicit context target, then
  /// the independent current row, then a lone selection, then the persisted
  /// default, then the upstream `ConfigHandler.SetDefaultServer` fallback. A
  /// different target is persisted first; the caller then applies exactly
  /// [StartTargetOutcome.target]. This never claims a run and never leaves a
  /// fake in-memory active on a rejected persist.
  StartTargetOutcome prepareStartTarget({String? frozenTargetId}) {
    final explicit = frozenTargetId?.trim();
    String? target;
    if (explicit != null && explicit.isNotEmpty) {
      target = explicit;
    } else if (state.primaryId != null && state.primaryId!.isNotEmpty) {
      target = state.primaryId;
    } else if (state.selected.length == 1) {
      target = state.selected.first;
    }
    target ??= state.activeId;
    target ??= _defaultRecoverableActive();
    if (target == null) {
      return const StartTargetOutcome(target: null, persisted: false);
    }
    if (state.activeId == target) {
      return StartTargetOutcome(target: target, persisted: true);
    }
    final result = setActive(target);
    if (!result.ok) {
      return StartTargetOutcome(
        target: target,
        persisted: false,
        errorCode: result.error?.code,
      );
    }
    return StartTargetOutcome(target: target, persisted: true, changed: true);
  }

  /// Upstream `ConfigHandler.SetDefaultServer` fallback after a missing or
  /// invalid default: first `Port > 0` node of the current visible list that
  /// still exists in the store, else the first stored `Port > 0` node.
  String? _defaultRecoverableActive() {
    final stored = <String, c.ProfileDto>{
      for (final p in state.profiles) p.indexId: p,
    };
    for (final row in state.visible) {
      final profile = stored[row.id];
      if (profile != null && profile.port > 0) return row.id;
    }
    for (final profile in state.profiles) {
      if (profile.port > 0) return profile.indexId;
    }
    return null;
  }

  List<ProfileColumn> _applyStoredLayout(List<ProfileColumn> defaults) {
    final section = _store.loadSection(columnSection);
    if (section == null) {
      final legacy = _store.loadColumnWidths();
      if (legacy.isEmpty) return defaults;
      return defaults
          .map(
            (c) => legacy.containsKey(c.key)
                ? c.copyWith(width: legacy[c.key])
                : c,
          )
          .toList();
    }

    final order = (section['order'] as List?)?.cast<String>() ?? const [];
    final widths = (section['widths'] as Map?) ?? const {};
    final visible = (section['visible'] as Map?) ?? const {};

    final byKey = <String, ProfileColumn>{for (final c in defaults) c.key: c};
    final ordered = <ProfileColumn>[];
    for (final key in order) {
      final column = byKey.remove(key);
      if (column != null) ordered.add(column);
    }
    ordered.addAll(defaults.where((c) => byKey.containsKey(c.key)));

    return ordered
        .map(
          (c) => c.copyWith(
            width: widths[c.key] is num
                ? (widths[c.key] as num).toDouble()
                : c.width,
            visible: visible.containsKey(c.key)
                ? visible[c.key] == true
                : c.visible,
          ),
        )
        .toList();
  }

  ProfilesState _recompute(ProfilesState base) {
    final filtered = applyFilter(base.all, base.filter);
    final String? group = base.groupSubId;
    final List<ProfileSummary> grouped;
    if (group == null) {
      grouped = filtered;
    } else {
      final subById = <String, String>{
        for (final p in base.profiles) p.indexId: p.subid,
      };
      grouped = filtered.where((r) => _rowSubId(subById, r) == group).toList();
    }
    final sorted = applySort(grouped, base.visibleColumns, base.sort);
    // Upstream `RefreshServersBiz` rebuilds the visible list and the selection
    // from it; a row that is no longer visible (group/filter change, delete)
    // must not stay selected, otherwise a batch/keyboard action would hit a
    // hidden node (RE-PROF-05).
    final visibleIds = sorted.map((r) => r.id).toSet();
    // The primary/current row must stay addressable: a refresh/filter that
    // hides it clears the main row instead of leaving a dangling target.
    final primaryVisible =
        base.primaryId != null && visibleIds.contains(base.primaryId);
    return base.copyWith(
      visible: sorted,
      selected: base.selected.where(visibleIds.contains).toSet(),
      primaryId: primaryVisible ? base.primaryId : null,
      clearPrimary: !primaryVisible,
    );
  }

  /// Whether [id] is currently visible in the table (group + text filter).
  ///
  /// The context-menu hidden-object contract: a command captured on a row that
  /// a later refresh/filter no longer shows must be refused, not silently run
  /// against the live state (R3-PROF-02).
  bool isVisibleTarget(String id) => state.visible.any((r) => r.id == id);

  /// All rows of the current group, ignoring the live text filter.
  ///
  /// Frozen `ConfigHandler.SortServers` receives `ProfileModels(subId, "")`,
  /// so a header sort reorders the whole group (hidden rows included) and must
  /// not be shrunk to the current search result (R3-PROF-03).
  List<ProfileSummary> _groupScope(ProfilesState base) {
    final group = base.groupSubId;
    if (group == null) return List<ProfileSummary>.of(base.all);
    final subById = <String, String>{
      for (final p in base.profiles) p.indexId: p.subid,
    };
    return base.all.where((r) => _rowSubId(subById, r) == group).toList();
  }

  /// Owning subscription id of a table row.
  ///
  /// The stored profiles are the authority (`ProfileDto.subid`, matching
  /// `setGroupSubId(sub.id)` and the groups-panel `counts[p.subid]`); the
  /// summary `subRemarks` slot only carries it as display text, so it is
  /// merely the fallback for generated rows without a stored profile.
  String _rowSubId(Map<String, String> subById, ProfileSummary row) {
    return subById[row.id] ?? row.subRemarks;
  }

  /// Select the subscription group shown in the node table (`null` = all).
  ///
  /// Switching to a non-empty group defaults the current row (active, else
  /// first) instead of leaving the table with no selection, matching upstream
  /// `RefreshServersBiz` (R3-PROF-10).
  void setGroupSubId(String? subId) {
    state = _recompute(
      state.copyWith(groupSubId: subId, clearGroup: subId == null),
    );
    if (state.selected.isEmpty && state.visible.isNotEmpty) {
      _selectDefaultRow();
    }
    _log('group-filter', 'sub=${subId ?? "(all)"}');
  }

  /// Subscription rows for the groups panel.
  List<c.SubItemDto> subItems() {
    try {
      return _bridge.listSubItems().items;
    } catch (_) {
      return const [];
    }
  }

  /// Move the given profiles to a subscription group (ACT-PROF-013).
  ///
  /// The real persistence path is the existing `saveProfile` seam: load each
  /// stored `ProfileDto`, set its `subid`/`subRemarks`, and save it back under
  /// the same optimistic revision. The bridge exposes no dedicated
  /// move-to-group API, so this reuses the documented profile save. Empty
  /// [subId] moves the nodes to the "no group" bucket. Returns false when a
  /// target no longer exists or any save is rejected; no partial fake success.
  bool moveProfilesToGroup(
    List<String> ids,
    String subId, {
    String subRemarks = '',
  }) {
    if (ids.isEmpty) return false;
    var allOk = true;
    for (final id in ids) {
      final dto = profileById(id);
      if (dto == null) {
        allOk = false;
        continue;
      }
      final moved = c.ProfileDto(
        indexId: dto.indexId,
        configType: dto.configType,
        coreType: dto.coreType,
        configVersion: dto.configVersion,
        subid: subId,
        isSub: dto.isSub,
        preSocksPort: dto.preSocksPort,
        displayLog: dto.displayLog,
        remarks: dto.remarks,
        address: dto.address,
        port: dto.port,
        password: dto.password,
        username: dto.username,
        network: dto.network,
        muxEnabled: dto.muxEnabled,
        finalmask: dto.finalmask,
        security: dto.security,
        protoExtra: dto.protoExtra,
        transportExtra: dto.transportExtra,
        extraJson: dto.extraJson,
      );
      final result = saveDraft(moved);
      if (!result.ok) allOk = false;
    }
    reload();
    _log(
      'move-to-group',
      'ids=${ids.length} sub=${subId.isEmpty ? "(none)" : subId}',
    );
    _echo('move-to-group');
    return allOk;
  }

  /// Generate the "all nodes" policy group for one subscription.
  c.SaveProfileResult genGroupAll(String subId) {
    final result = _bridge.genGroupAll(subId);
    if (result.ok) {
      reload();
      _log('gen-group-all', 'sub=$subId');
    }
    return result;
  }

  /// Generate one policy group per matching region.
  ///
  /// Returns the real bridge result so the caller can select the first created
  /// group (upstream `_pendingSelectIndexId`) and report the exact outcome.
  groups.GroupGenResult genGroupRegion(String subId) {
    final result = _bridge.genGroupRegion(subId);
    if (result.ok) {
      reload();
      _log('gen-group-region', 'sub=$subId count=${result.profiles.length}');
    }
    return result;
  }

  /// Select the first generated group that is present in the current view.
  ///
  /// Upstream sets `_pendingSelectIndexId` and selects it after the refresh.
  /// The generated group lives in the current subscription view, so it is in
  /// `state.all` for the real backend; in pure widget tests where the summary
  /// list is synthetic it may be absent, in which case the selection is left
  /// untouched instead of inventing a row.
  void selectGenerated(List<String> ids) {
    if (ids.isEmpty) return;
    final known = state.all.map((r) => r.id).toSet();
    final target = ids.firstWhere(known.contains, orElse: () => '');
    if (target.isEmpty) return;
    state = state.copyWith(selected: selectSingle(target));
    _log('gen-group-select', 'id=$target');
  }

  /// Apply a text filter immediately (programmatic/test seam).
  ///
  /// The UI filter box does not use this: it follows upstream
  /// `ServerFilterChanged`, which only refreshes on clear, plus Enter submit
  /// ([updateFilterInput]/[submitFilter]).
  void setFilter(String value) {
    _queryGeneration++;
    state = _recompute(state.copyWith(filter: value, filterInput: value));
  }

  /// Record raw filter-box text without applying it.
  ///
  /// Upstream `ServerFilterChanged` only calls `RefreshServers` when the box is
  /// cleared (frozen `ProfilesViewModel.cs:343-350`); a non-empty query stays
  /// pending until Enter (`TxtServerFilter_PreviewKeyDown` ->
  /// `RefreshServers`). Clearing the box refreshes immediately.
  void updateFilterInput(String value) {
    if (value.trim().isEmpty) {
      _queryGeneration++;
      state = _recompute(state.copyWith(filter: '', filterInput: value));
      _log('refresh', 'filter="" -> ${state.visible.length}');
      return;
    }
    state = state.copyWith(filterInput: value);
  }

  /// Commit the pending filter-box text (Enter). The query is applied to the
  /// current group through the same [applyFilter] used by every refresh.
  void submitFilter() {
    _queryGeneration++;
    state = _recompute(state.copyWith(filter: state.filterInput));
    _log('refresh', 'filter="${state.filter}" -> ${state.visible.length}');
  }

  void sortBy(String key) {
    final sort = state.sort.next(key);
    // Frozen `ConfigHandler.SortServers` sorts `ProfileModels(subId, "")`: the
    // whole current group, ignoring the live text filter. The persisted `Sort`
    // therefore covers hidden rows too, while the table still shows the
    // filtered subsequence in the same order (R3-PROF-03).
    final scoped = applySort(_groupScope(state), state.visibleColumns, sort);
    state = _recompute(state.copyWith(sort: sort));
    _persistOrder(ids: scoped.map((r) => r.id).toList());
    _log('sort', '$key ${sort.direction.name} scoped=${scoped.length}');
  }

  /// Persist the current visible row order into the upstream
  /// `ProfileExItem.Sort` field (PR-15 / FIX-10B).
  ///
  /// Mirrors `ConfigHandler.SortServers`/`MoveServer`: the whole visible list
  /// is written as `(position + 1) * 10` through the real
  /// `speedtestApplyProfileOrder` bridge, so the order survives a restart. An
  /// empty list or a single row carries no ordering information and is never
  /// written (no fake persistence, no error).
  c.SimpleResult _persistOrder({List<String>? ids}) {
    final ordered = ids ?? state.visible.map((r) => r.id).toList();
    if (ordered.length < 2) return const c.SimpleResult(ok: true);
    final result = _bridge.applyProfileOrder(ordered);
    if (result.ok) {
      if (state.orderMessage != null) {
        state = state.copyWith(clearOrderMessage: true);
      }
      return result;
    }
    final code = result.error?.code ?? 'unknown';
    final detail = result.error?.messageKey;
    final message = (detail == null || detail.isEmpty)
        ? '排序保存失败：$code'
        : '排序保存失败：$code（$detail）';
    state = state.copyWith(orderMessage: message);
    _log('order-persist-failed', code);
    return result;
  }

  /// `按测试结果排序` (ACT-PROF-020): order the visible rows by the measured
  /// delay ascending, unknown/failed delays (-1) last. Operates on the real
  /// `ProfileSummary.delay` from the speedtest result overlay; no synthetic
  /// ordering is produced.
  void sortByResult() {
    // Route through the same DelayVal sort as a header click, so the active
    // sort is synced into `state.sort`: a later reload/`_recompute` re-applies
    // it instead of falling back to the previously clicked column (R3-PROF-03).
    // `applySort` sinks failed/untested (`<= 0`) delays in both directions.
    final ascending = _resultSortAscending;
    _resultSortAscending = !ascending;
    final sort = SortSpec(
      columnKey: 'DelayVal',
      direction: ascending ? SortDirection.ascending : SortDirection.descending,
    );
    state = _recompute(state.copyWith(sort: sort));
    _persistOrder();
    _log(
      'sort-result',
      'rows=${state.visible.length} direction=${ascending ? "asc" : "desc"}',
    );
    _echo('sort-result');
  }

  void selectRow(String id, {bool ctrl = false, bool shift = false}) {
    Set<String> next;
    if (shift) {
      // Range from the main row / anchor, not the arbitrary last set entry
      // (R3-PROF-10). The anchor stays the current row after the extend.
      final anchor =
          state.primaryId ??
          (state.selected.isEmpty ? id : state.selected.last);
      next = extendSelection(state.visible, state.selected, anchor, id);
    } else if (ctrl) {
      next = toggleSelection(state.selected, id);
    } else {
      next = selectSingle(id);
    }
    // Shift extends from the main row, so that row stays current; a plain or
    // Ctrl click makes the clicked row current.
    final primary = shift ? (state.primaryId ?? id) : id;
    state = state.copyWith(selected: next, primaryId: primary);
    _log('select', 'id=$id selected=${next.length} primary=$primary');
  }

  /// Drag-select: replace the selection with the inclusive visible range
  /// between the drag anchor and the row under the pointer (WPF DataGrid
  /// press-and-drag parity). The anchor stays the current/main row so the
  /// target does not drift while the range grows.
  void selectRange(String anchorId, String currentId) {
    final next = rangeSelection(state.visible, anchorId, currentId);
    if (setEquals(next, state.selected) && state.primaryId == anchorId) return;
    state = state.copyWith(selected: next, primaryId: anchorId);
    _log('select', 'range=$anchorId..$currentId selected=${next.length}');
  }

  void selectAll() {
    final ids = state.visible.map((r) => r.id).toSet();
    final primary = state.primaryId != null && ids.contains(state.primaryId)
        ? state.primaryId
        : (state.visible.isEmpty ? null : state.visible.first.id);
    state = state.copyWith(
      selected: ids,
      primaryId: primary,
      clearPrimary: primary == null,
    );
    _log(ProfileAction.selectAll, 'selected=${ids.length}');
    _echo(ProfileAction.selectAll);
  }

  void clearSelection() {
    state = state.copyWith(selected: const <String>{}, clearPrimary: true);
    _log(ProfileAction.escape, 'selection cleared');
    _echo(ProfileAction.escape);
  }

  /// Move the current row one step up/down with the arrow keys. When nothing
  /// is selected the first (down) / last (up) row is selected, matching the
  /// usual DataGrid navigation contract without touching move-up/down order.
  void navigateSelection(int delta) {
    if (delta == 0) return;
    final rows = state.visible;
    if (rows.isEmpty) return;
    // Move from the current/main row (`primaryId`), so a multi-selection
    // advances from where the user is instead of the first selected entry
    // (R3-PROF-10). Fall back to the last selected id, then the edges.
    final currentId =
        state.primaryId ??
        (state.selected.isEmpty ? null : state.selected.last);
    var index = currentId == null
        ? (delta > 0 ? -1 : rows.length)
        : rows.indexWhere((r) => r.id == currentId);
    if (index < 0) {
      index = delta > 0 ? -1 : rows.length;
    }
    final target = (index + delta).clamp(0, rows.length - 1);
    final id = rows[target].id;
    state = state.copyWith(selected: selectSingle(id), primaryId: id);
    _log(
      delta < 0 ? ProfileAction.navigateUp : ProfileAction.navigateDown,
      'id=$id',
    );
  }

  void handleRightTap(String id) {
    // The row under the pointer becomes the current/main row; an existing
    // multi-selection is preserved (upstream DataGrid context menu).
    if (!state.selected.contains(id)) {
      state = state.copyWith(selected: selectSingle(id), primaryId: id);
    } else {
      state = state.copyWith(primaryId: id);
    }
    _log(ProfileAction.contextMenu, 'kept=${state.selected.length}');
  }

  /// Re-bind the selection to the target ids captured when the context menu
  /// opened, so an open menu always commands the rows it was opened on even if
  /// the selection drifted meanwhile.
  ///
  /// Returns false without touching the selection when the snapshot is empty or
  /// any target id is no longer **visible** (refresh/delete/filter); the caller
  /// then closes the stale menu instead of acting on a hidden/other row. Note
  /// this checks the current view (`state.visible`), not merely the store, so a
  /// text filter that hides a captured target refuses the command (R3-PROF-02).
  bool restoreContextTargets(List<String> ids) {
    if (ids.isEmpty) return false;
    final visible = state.visible.map((r) => r.id).toSet();
    if (!ids.every(visible.contains)) return false;
    final target = ids.toSet();
    final primary = state.primaryId != null && target.contains(state.primaryId)
        ? state.primaryId
        : ids.first;
    if (!setEquals(state.selected, target) || state.primaryId != primary) {
      state = state.copyWith(selected: target, primaryId: primary);
      _log(ProfileAction.contextMenu, 'restore=${ids.length}');
    }
    return true;
  }

  void handleDoubleClick(String id) {
    final action = state.doubleClick2Activate
        ? ProfileAction.activate
        : ProfileAction.edit;
    selectRow(id);
    _log(action, 'double-click id=$id');
    _echo(action);
  }

  /// Record a table action for the event log without changing selection.
  void logAction(String action, String detail) {
    _log(action, detail);
    _echo(action);
  }

  /// Drop the accumulated event log. Used by integration evidence so a later
  /// command's `restore=`/action events are unambiguous; no product meaning.
  void resetEvents() {
    state = state.copyWith(events: const <TableEvent>[]);
  }

  void handleDragStart(String id) {
    _log(ProfileAction.dragStart, 'id=$id');
  }

  /// `拖动排序` (ACT-PROF-033): move [sourceId] to [targetId]'s position in the
  /// visible order and persist it via `ProfileExItem.Sort`.
  ///
  /// The reorder is expressed on the current visible list (the current group +
  /// text filter), matching `ConfigHandler.MoveServer` operating on the visible
  /// `ProfileItems`. A no-op (same row, unknown id, single row) leaves the
  /// order untouched and writes nothing.
  void handleDrop(String sourceId, String targetId) {
    _log(ProfileAction.drop, '$sourceId -> $targetId');
    if (sourceId == targetId) return;
    final rows = List<ProfileSummary>.of(state.visible);
    if (rows.length < 2) return;
    final from = rows.indexWhere((r) => r.id == sourceId);
    final to = rows.indexWhere((r) => r.id == targetId);
    if (from < 0 || to < 0) return;
    final moved = rows.removeAt(from);
    rows.insert(to, moved);
    // Manual drag defines the order; clear any active column sort so the
    // dragged order is what the user sees and what gets persisted.
    state = _recompute(
      state.copyWith(all: _withVisibleOrder(rows), sort: const SortSpec()),
    );
    _persistOrder();
  }

  /// Reflect a new visible order back onto `all`, keeping hidden rows in place.
  ///
  /// [orderedVisible] is the full current visible set; the hidden rows keep
  /// their relative slots so a later group switch/filter reset is not
  /// scrambled by a drag performed in one group.
  List<ProfileSummary> _withVisibleOrder(List<ProfileSummary> orderedVisible) {
    final visibleIds = orderedVisible.map((r) => r.id).toSet();
    final iterator = orderedVisible.iterator;
    return <ProfileSummary>[
      for (final row in state.all)
        if (visibleIds.contains(row.id))
          (iterator.moveNext() ? iterator.current : row)
        else
          row,
    ];
  }

  void toggleDoubleClick2Activate() {
    state = state.copyWith(doubleClick2Activate: !state.doubleClick2Activate);
    _log(
      'toggle-double-click',
      'DoubleClick2Activate=${state.doubleClick2Activate}',
    );
  }

  /// Apply the persisted `UIItem.DoubleClick2Activate` value.
  void setDoubleClick2Activate(bool value) {
    if (state.doubleClick2Activate == value) return;
    state = state.copyWith(doubleClick2Activate: value);
    _log('double-click', 'DoubleClick2Activate=$value');
  }

  bool handleKeyEvent(KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) return false;
    final keyboard = HardwareKeyboard.instance;
    final action = actionForKey(
      key: event.logicalKey,
      ctrl: keyboard.isControlPressed,
      shift: keyboard.isShiftPressed,
      alt: keyboard.isAltPressed,
    );
    if (action == null) return false;
    emitAction(action);
    return true;
  }

  void emitAction(String action) {
    switch (action) {
      case ProfileAction.selectAll:
        selectAll();
        return;
      case ProfileAction.autofitColumns:
        autofitColumns();
        return;
      case ProfileAction.escape:
        // ACT-PROF-038: Esc only stops a running test; it must keep the
        // selection (upstream `LstProfiles_PreviewKeyDown(Escape)` calls
        // `ServerSpeedtestStop()` and nothing else, R3-PROF-10).
        if (state.speedTestRunning) {
          cancelSpeedTest();
        }
        _log(ProfileAction.escape, 'selection kept=${state.selected.length}');
        _echo(ProfileAction.escape);
        return;
      case ProfileAction.tcping:
      case ProfileAction.realping:
      case ProfileAction.speedtest:
      case ProfileAction.mixedTest:
      case ProfileAction.fastRealping:
      case ProfileAction.udpTest:
        startSpeedTest(action);
        return;
      case ProfileAction.removeInvalid:
        removeInvalidResults();
        return;
      case ProfileAction.stopTest:
        cancelSpeedTest();
        return;
      case ProfileAction.moveTop:
        moveSelected(toStart: true);
        _log(action, 'selected=${state.selected.length}');
        return;
      case ProfileAction.moveBottom:
        moveSelected(toEnd: true);
        _log(action, 'selected=${state.selected.length}');
        return;
      case ProfileAction.moveUp:
        moveSelected(delta: -1);
        _log(action, 'selected=${state.selected.length}');
        return;
      case ProfileAction.moveDown:
        moveSelected(delta: 1);
        _log(action, 'selected=${state.selected.length}');
        return;
      case ProfileAction.navigateUp:
        navigateSelection(-1);
        return;
      case ProfileAction.navigateDown:
        navigateSelection(1);
        return;
      default:
        _log(action, 'selected=${state.selected.length}');
        _echo(action);
    }
  }

  void pingSelected() {
    if (state.selected.isEmpty) {
      _log(ProfileAction.tcping, 'no-selection');
      return;
    }
    final id = state.selected.first;
    final delay = _bridge.pingProfile(id);
    _log(ProfileAction.tcping, 'id=$id delay=$delay');
  }

  // -- T15b speedtest actions -------------------------------------------

  /// Map a UI action to `domain::SpeedTestAction` (0..5), or null.
  static int? speedTestKindForAction(String action) {
    switch (action) {
      case ProfileAction.tcping:
        return 0;
      case ProfileAction.realping:
        return 1;
      case ProfileAction.udpTest:
        return 2;
      case ProfileAction.speedtest:
        return 3;
      case ProfileAction.mixedTest:
        return 4;
      case ProfileAction.fastRealping:
        return 5;
      default:
        return null;
    }
  }

  /// Start a speedtest job for [action].
  ///
  /// Upstream `ServerSpeedtest`:
  ///   * `Mixedtest` / `FastRealping` (Fast is remapped to Realping) test the
  ///     current `ProfileItems` list (the current group + text filter, ordered
  ///     by `Sort`), **not** the whole database and not the selection;
  ///   * every other action tests `SelectedProfiles` (the current selection).
  ///
  /// Mixed/Fast therefore send the current *visible* ids; the other actions
  /// send the selection. An empty scope means "nothing to test" — it is never
  /// widened to the whole DB (PR-16).
  c.SimpleResult startSpeedTest(String action) {
    final kind = speedTestKindForAction(action);
    if (kind == null) return const c.SimpleResult(ok: true);

    // RE-PROF-08: UDP is a runtime capability. When the build/platform cannot
    // perform it, report an explicit reason instead of starting a job that can
    // only fail silently.
    if (kind == 2 && !_bridge.speedTestSupport().udp) {
      state = state.copyWith(
        speedTestRunning: false,
        speedTestStage: 'SpeedtestingFailed',
        speedTestMessage: '当前构建/平台不支持 UDP 测速',
        clearSpeedTestJob: true,
      );
      _log(action, 'udp-unsupported');
      _echo(action);
      return const c.SimpleResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_UNSUPPORTED',
          messageKey: 'error.speedtest_udp_unsupported',
          retryable: false,
        ),
      );
    }

    final config = _resolveSpeedTestConfig();
    _bridge.configureSpeedTest(
      pageSize: config.pageSize,
      mixedConcurrency: config.mixedConcurrency,
      timeoutSecs: config.timeoutSecs,
      speedTestUrl: config.speedTestUrl,
      speedPingTestUrl: config.speedPingTestUrl,
      ipapiUrl: config.ipapiUrl,
      udpTestTarget: config.udpTestTarget,
      delayIntervalSecs: config.delayIntervalSecs,
    );

    final useVisible =
        action == ProfileAction.mixedTest ||
        action == ProfileAction.fastRealping;
    final ids = useVisible
        ? state.visible.map((r) => r.id).toList()
        : state.selected.toList();
    final scopeIds = List<String>.of(ids);

    if (scopeIds.isEmpty) {
      state = state.copyWith(
        speedTestRunning: false,
        speedTestStage: 'SpeedtestingCompleted',
        speedTestMessage: '没有可测试节点',
        clearSpeedTestJob: true,
      );
      _log(action, 'no-nodes');
      _echo(action);
      return const c.SimpleResult(ok: true);
    }

    // Bump the run generation before the bridge call so the new run owns the
    // poller: any older poller self-cancels on its next tick and can no longer
    // settle or summarize this run (R4-22).
    final generation = ++_speedTestGeneration;
    final result = _bridge.startSpeedTest(kind, ids);
    if (!result.ok) {
      final code = result.error?.code ?? 'unknown';
      final detail = result.error?.messageKey;
      final message = (detail == null || detail.isEmpty)
          ? '测速启动失败：$code'
          : '测速启动失败：$code（$detail）';
      state = state.copyWith(
        speedTestRunning: false,
        speedTestStage: 'SpeedtestingFailed',
        speedTestMessage: message,
        speedTestGeneration: generation,
        clearSpeedTestJob: true,
      );
      _log('speedtest-start-failed', code);
      _echo(action);
      return c.SimpleResult(ok: false, error: result.error);
    }

    final started = result.jobId != null;
    if (started) {
      _activeSpeedTestJobs.add(result.jobId!);
    }
    state = state.copyWith(
      speedTestJobId: result.jobId,
      speedTestKind: kind,
      speedTestRunning: started,
      speedTestStage: started ? 'Speedtesting' : 'SpeedtestingCompleted',
      speedTestMessage: started
          ? (useVisible
                ? '正在测试当前列表 ${scopeIds.length} 个节点'
                : '正在测试选中 ${scopeIds.length} 个节点')
          : '没有可测试节点',
      speedTestGeneration: generation,
      clearSpeedTestJob: !started,
    );
    _log(
      action,
      'kind=$kind ids=${ids.length} scope=${scopeIds.length} '
      'job=${result.jobId ?? "-"} gen=$generation',
    );
    _echo(action);
    if (started) {
      // The Rust start already cleared the run's stale results; read them once
      // now so the table shows "no result yet" without waiting a poll tick.
      _refreshLive();
      _startTestPolling(generation, scopeIds);
    }
    return const c.SimpleResult(ok: true);
  }

  void cancelSpeedTest() {
    // Upstream `SpeedtestService.ExitLoop` cancels every in-flight run, so the
    // stop binds to all tracked real job ids, never just the latest one.
    final jobs = _activeSpeedTestJobs.toSet();
    if (jobs.isEmpty) return;
    // Invalidate the current polling generation before cancelling so the old
    // poller cannot settle the run while the runner is still winding down.
    final generation = ++_speedTestGeneration;
    for (final jobId in jobs) {
      _bridge.cancelSpeedTest(jobId);
    }
    _activeSpeedTestJobs.clear();
    state = state.copyWith(
      speedTestRunning: false,
      speedTestStage: 'SpeedtestingStop',
      speedTestMessage: '已停止测速',
      speedTestGeneration: generation,
    );
    _log('speedtest-stop', 'jobs=${jobs.length} gen=$generation');
    _echo(ProfileAction.stopTest);
    // Keep a bounded settle poller until every real job reports zero, doing a
    // final overlay read so results measured before the runner actually exits
    // are never lost; a cancelled run is never summarized as completed.
    _startTestPolling(generation, const <String>[]);
  }

  /// `按测试结果移除无效` (ACT-PROF-021 / PR-11 / RE-PROF-06): really delete the
  /// failed `ProfileItem`s of the current group, not just the test-result rows.
  ///
  /// Upstream `ConfigHandler.RemoveInvalidServerResult` finds every profile in
  /// the current `subid` whose `ProfileExItem.Delay == -1` (complex nodes are
  /// excluded) and removes it. The scope is the **whole current group**
  /// (`ProfileModels(subid, "")`): the text filter must not shrink it, so a
  /// failed node that a search merely hides is still removed.
  ///
  /// Result rows are pruned only after every targeted profile is really gone;
  /// a failed or partial delete keeps the same group's failure evidence, and
  /// another group's failed nodes are never touched. Returns the number of
  /// profiles actually deleted.
  int removeInvalidResults() {
    final failedIds = _bridge
        .speedTestResults()
        .where((r) => r.delay == -1)
        .map((r) => r.indexId)
        .toSet();
    // Only delete stored, non-complex profiles of the current group (upstream
    // `RemoveInvalidServerResult` skips complex nodes and ignores the filter).
    final targets = state.profiles
        .where((p) => failedIds.contains(p.indexId))
        .where((p) => !isComplexProfile(p.configType))
        .where(_profileInGroup)
        .map((p) => p.indexId)
        .toList();
    var removed = 0;
    var fullyRemoved = true;
    if (targets.isNotEmpty) {
      final result = _bridge.deleteProfiles(targets);
      if (result.ok) removed = result.removed.toInt();
      fullyRemoved = result.ok && removed == targets.length;
      if (!result.ok) {
        _log(
          ProfileAction.removeInvalid,
          'delete-failed candidates=${targets.length}',
        );
      }
    }
    // Prune only once every target is gone. The Rust orphan-prune drops rows
    // whose profile no longer exists, so a failed/partial delete keeps the
    // same group's evidence and another group's failed nodes (still stored)
    // are never touched. (A group-scoped prune cannot run after the delete:
    // the removed profiles no longer identify their group.)
    if (targets.isNotEmpty && fullyRemoved) {
      _bridge.removeInvalidResults();
    }
    reload();
    _log(
      ProfileAction.removeInvalid,
      'profiles=$removed candidates=${targets.length} pruned=$fullyRemoved',
    );
    _echo(ProfileAction.removeInvalid);
    return removed;
  }

  /// Whether a stored profile belongs to the current group (`subid`).
  ///
  /// Dedup / remove-invalid scope to the whole group, mirroring upstream
  /// `ProfileItems(subId)` / `ProfileModels(subid, "")`; the live text filter is
  /// deliberately not applied, so a filter-hidden node is still eligible
  /// (RE-PROF-06). When no group is selected all stored profiles are in scope.
  bool _profileInGroup(c.ProfileDto profile) {
    final group = state.groupSubId;
    if (group == null) return true;
    return profile.subid == group;
  }

  /// `移除重复` (ACT-PROF-003 / PR-12): deduplicate the current group by
  /// transport identity (complex nodes always stay) and report the outcome.
  ///
  /// When [keepOlder] is null the frozen `GuiItem.KeepOlderDedupl` setting is
  /// read (`false` reverses the list so the newer entry wins, matching
  /// `ConfigHandler.cs:1166-1168`); the real UI path uses this. The comparison
  /// mirrors `ConfigHandler.CompareProfileItem`; deletion goes through the real
  /// `deleteProfiles` seam so it persists and survives a restart. Unlike the
  /// legacy int wrapper this distinguishes "no duplicates" from a real failure
  /// and reports whether the active node was deleted (R3-PROF-04).
  DedupOutcome removeDuplicateProfilesDetailed({bool? keepOlder}) {
    // Upstream `DedupServerList` scopes to the whole group (`ProfileItems(subId)`),
    // never the live text filter (RE-PROF-06).
    final effectiveKeepOlder = keepOlder ?? readKeepOlderDedupl(_bridge);
    final candidates = state.profiles.where(_profileInGroup).toList();
    final duplicates = deduplicateProfiles(
      candidates,
      keepOlder: effectiveKeepOlder,
    );
    if (duplicates.isEmpty) {
      _log(ProfileAction.removeDuplicate, 'none');
      _echo(ProfileAction.removeDuplicate);
      return const DedupOutcome(ok: true, hadDuplicates: false);
    }
    final activeBefore = state.activeId;
    final activeRemoved =
        activeBefore != null && duplicates.contains(activeBefore);
    final result = _bridge.deleteProfiles(duplicates);
    if (!result.ok) {
      _log(ProfileAction.removeDuplicate, 'delete-failed');
      _echo(ProfileAction.removeDuplicate);
      return DedupOutcome(
        ok: false,
        hadDuplicates: true,
        activeRemoved: activeRemoved,
        errorCode: result.error?.code,
        errorMessageKey: result.error?.messageKey,
      );
    }
    final removed = result.removed.toInt();
    reload();
    _log(
      ProfileAction.removeDuplicate,
      'removed=$removed keepOlder=$effectiveKeepOlder',
    );
    _echo(ProfileAction.removeDuplicate);
    return DedupOutcome(
      ok: true,
      removed: removed,
      hadDuplicates: true,
      activeRemoved: activeRemoved,
    );
  }

  /// Number of profiles removed by dedup. Kept for direct controller callers;
  /// explicit [keepOlder] is honored (the settings-driven path is
  /// [removeDuplicateProfilesDetailed]).
  int removeDuplicateProfiles({bool keepOlder = true}) =>
      removeDuplicateProfilesDetailed(keepOlder: keepOlder).removed;

  Future<void> runBlockingProbe(int ms) async {
    state = state.copyWith(blockingBusy: true);
    _log('blocking-start', 'ms=$ms');
    final elapsed = await _bridge.simulateBlocking(ms);
    state = state.copyWith(blockingBusy: false, lastBlockingMs: elapsed);
    _log('blocking-done', 'elapsed=$elapsed ms');
  }

  /// `自动列宽` (upstream `BtnAutofitColumnWidth_Click` /
  /// `AutofitColumnWidth`, `ProfilesView.xaml.cs:296-314`): size every column to
  /// the wider of its header and current cell content, equivalent to setting
  /// each `DataGridColumn.Width` to `Auto`.
  ///
  /// The measured widths are persisted through the same column store as a
  /// manual resize, so the fit survives a reopen. Only realized (visible) rows
  /// are measured, mirroring the real DataGrid; a large list is bounded so the
  /// command stays responsive.
  void autofitColumns() {
    const headerStyle = TextStyle(fontWeight: FontWeight.bold, fontSize: 12);
    const cellStyle = TextStyle(fontSize: AppTokens.fontSize);
    final rows = state.visible;
    final sampled = rows.length > _autofitSampleRows
        ? rows.sublist(0, _autofitSampleRows)
        : rows;
    final updated = state.columns.map((column) {
      final headerWidth = _measureText('${column.title} \u25B2', headerStyle);
      var contentWidth = 0.0;
      for (final row in sampled) {
        final width = _measureText(column.display(row), cellStyle);
        if (width > contentWidth) contentWidth = width;
      }
      final fitted =
          max(headerWidth, contentWidth) + _cellHorizontalPadding * 2;
      return column.copyWith(width: fitted.clamp(40.0, 600.0).toDouble());
    }).toList();
    state = state.copyWith(columns: updated);
    _persistColumns();
    _log('autofit-columns', 'columns=${updated.length} rows=${sampled.length}');
    _echo('autofit-columns');
  }

  static const int _autofitSampleRows = 500;
  static const double _cellHorizontalPadding = 6;

  double _measureText(String text, TextStyle style) {
    final painter = TextPainter(
      text: TextSpan(text: text, style: style),
      maxLines: 1,
      textDirection: TextDirection.ltr,
    )..layout();
    return painter.width;
  }

  void resizeColumn(String key, double delta) {
    final updated = state.columns
        .map(
          (c) => c.key == key
              ? c.copyWith(width: (c.width + delta).clamp(40.0, 600.0))
              : c,
        )
        .toList();
    state = state.copyWith(columns: updated);
    _persistColumns();
  }

  void toggleColumnVisibility(String key) {
    final visibleCount = state.columns.where((c) => c.visible).length;
    final target = state.columns.firstWhere((c) => c.key == key);
    if (target.visible && visibleCount <= 1) return;
    final updated = state.columns
        .map((c) => c.key == key ? c.copyWith(visible: !c.visible) : c)
        .toList();
    state = state.copyWith(columns: updated);
    _persistColumns();
    _log('column-visibility', '${target.key} -> ${!target.visible}');
  }

  /// Move a column by [delta] positions in the display order.
  void moveColumn(String key, int delta) {
    final columns = List<ProfileColumn>.of(state.columns);
    final index = columns.indexWhere((c) => c.key == key);
    if (index < 0) return;
    final target = (index + delta).clamp(0, columns.length - 1);
    if (target == index) return;
    final column = columns.removeAt(index);
    columns.insert(target, column);
    state = state.copyWith(columns: columns);
    _persistColumns();
    _log('column-move', '$key $index -> $target');
  }

  /// UI-only reorder of the selected rows, preserving relative order.
  void moveSelected({int delta = 0, bool toStart = false, bool toEnd = false}) {
    if (state.selected.isEmpty) {
      _log('move', 'no-selection');
      return;
    }
    final selected = state.all.where((r) => state.selected.contains(r.id));
    final selectedIds = selected.map((r) => r.id).toSet();
    final rest = state.all.where((r) => !selectedIds.contains(r.id)).toList();
    final block = selected.toList();
    late List<ProfileSummary> next;
    if (toStart) {
      next = <ProfileSummary>[...block, ...rest];
    } else if (toEnd) {
      next = <ProfileSummary>[...rest, ...block];
    } else if (delta < 0) {
      next = <ProfileSummary>[...rest];
      final firstIndex = state.all.indexWhere(
        (r) => selectedIds.contains(r.id),
      );
      final insertAt = (firstIndex - 1).clamp(0, next.length);
      next.insertAll(insertAt, block);
    } else {
      next = <ProfileSummary>[...rest];
      final lastIndex = state.all.lastIndexWhere(
        (r) => selectedIds.contains(r.id),
      );
      final insertAt = (lastIndex + 1 - block.length + 1).clamp(0, next.length);
      next.insertAll(insertAt, block);
    }
    // A manual reorder defines the new order; an active column sort would
    // immediately override it, so clear it (upstream `MoveServer` is a manual
    // move independent of `SortServers`).
    state = _recompute(state.copyWith(all: next, sort: const SortSpec()));
    _persistOrder();
  }

  void _persistColumns() {
    _store.saveSection(columnSection, <String, dynamic>{
      'order': state.columns.map((c) => c.key).toList(),
      'visible': <String, bool>{
        for (final c in state.columns) c.key: c.visible,
      },
      'widths': <String, double>{for (final c in state.columns) c.key: c.width},
    });
  }

  void _log(String action, String detail) {
    _seq += 1;
    final events = List<TableEvent>.of(state.events)
      ..add(TableEvent(seq: _seq, action: action, detail: detail));
    if (events.length > 500) {
      events.removeRange(0, events.length - 500);
    }
    state = state.copyWith(events: events);
  }

  void _echo(String action) {
    try {
      final ack = _bridge.echoEvent(_seq, action);
      state = state.copyWith(
        lastAckSeq: ack.seq.toInt(),
        lastAckRustCount: ack.rustProfileCount,
        rustCount: ack.rustProfileCount,
      );
    } catch (_) {
      // Rust echo is best effort; the UI still records the local event.
    }
  }
}
