import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
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

final bridgePortProvider = Provider<BridgePort>((ref) => const FrbBridgePort());

final uiStateStoreProvider = Provider<UiStateStore>(
  (ref) => FileUiStateStore(),
);

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
    required this.sort,
    required this.selected,
    required this.events,
    required this.doubleClick2Activate,
    required this.rustCount,
    required this.columns,
    this.profiles = const <c.ProfileDto>[],
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
  });

  final List<ProfileSummary> all;
  final List<ProfileSummary> visible;
  final String filter;
  final SortSpec sort;
  final Set<String> selected;
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

  int get selectedCount => selected.length;
  int get totalCount => all.length;
  TableEvent? get lastEvent => events.isEmpty ? null : events.last;

  List<ProfileColumn> get visibleColumns =>
      columns.where((c) => c.visible).toList();

  ProfilesState copyWith({
    List<ProfileSummary>? all,
    List<ProfileSummary>? visible,
    String? filter,
    SortSpec? sort,
    Set<String>? selected,
    List<TableEvent>? events,
    bool? doubleClick2Activate,
    int? rustCount,
    List<ProfileColumn>? columns,
    List<c.ProfileDto>? profiles,
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
    bool clearSpeedTestJob = false,
  }) {
    return ProfilesState(
      all: all ?? this.all,
      visible: visible ?? this.visible,
      filter: filter ?? this.filter,
      sort: sort ?? this.sort,
      selected: selected ?? this.selected,
      events: events ?? this.events,
      doubleClick2Activate: doubleClick2Activate ?? this.doubleClick2Activate,
      rustCount: rustCount ?? this.rustCount,
      columns: columns ?? this.columns,
      profiles: profiles ?? this.profiles,
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
    );
  }
}

class ProfilesController extends Notifier<ProfilesState> {
  static const columnSection = 'column_layout';

  BridgePort get _bridge => ref.read(bridgePortProvider);
  UiStateStore get _store => ref.read(uiStateStoreProvider);

  int _seq = 0;
  Timer? _testPoller;

  /// Node ids covered by the last started speedtest job; used to summarize the
  /// run when it settles (progress rows alone are not a completion verdict).
  List<String> _speedTestTargets = const <String>[];

  @override
  ProfilesState build() {
    final count = ref.read(profileRowCountProvider);
    final rows = _bridge.fetchSummaries(count);
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
      profiles: _bridge.queryAllProfiles(),
      activeId: _bridge.getActiveProfile(),
    );
  }

  /// While a job runs, poll the Rust `ProfileExItem` store and refresh the
  /// table. The Rust side still coalesces results into 50-100 ms batches; the
  /// UI simply reads the closure at a bounded cadence and never fabricates a
  /// percentage.
  void _startTestPolling() {
    _testPoller?.cancel();
    _testPoller = Timer.periodic(const Duration(milliseconds: 150), (timer) {
      reload();
      if (_bridge.speedTestActiveJobs() == 0) {
        timer.cancel();
        _testPoller = null;
        final cancelled = state.speedTestStage == 'SpeedtestingStop';
        state = state.copyWith(
          speedTestRunning: false,
          speedTestStage: cancelled
              ? 'SpeedtestingStop'
              : 'SpeedtestingCompleted',
          speedTestMessage: cancelled
              ? '已停止测速'
              : _summarizeSpeedTest(_speedTestTargets),
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
    final rows = _bridge.fetchSummaries(count);
    final active = _bridge.getActiveProfile();
    state = _recompute(
      state.copyWith(
        all: rows,
        profiles: _bridge.queryAllProfiles(),
        activeId: active,
        clearActive: active == null,
      ),
    );
    _log('reload', 'profiles=${state.profiles.length} rows=${rows.length}');
  }

  /// Persist a draft through the real bridge (optimistic revision).
  c.SaveProfileResult saveDraft(c.ProfileDto draft) {
    final result = _bridge.saveProfile(draft, _bridge.profileRevision());
    if (result.ok) {
      reload();
      _log('save-profile', 'id=${result.profile?.indexId ?? draft.indexId}');
    } else {
      _log('save-profile-failed', result.error?.code ?? 'unknown');
    }
    return result;
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
    return base.copyWith(
      visible: sorted,
      selected: base.selected.where(visibleIds.contains).toSet(),
    );
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
  void setGroupSubId(String? subId) {
    state = _recompute(
      state.copyWith(groupSubId: subId, clearGroup: subId == null),
    );
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

  /// Persisted resolution of a group/chain node (subscription matches first,
  /// then the explicit `ChildItems` order), for the group editor preview.
  ///
  /// Best effort like the Rust echo path: a backend failure yields an empty
  /// preview instead of breaking the editor.
  List<c.ProfileDto> groupChildPreview(String indexId) {
    if (indexId.isEmpty) return const [];
    try {
      return _bridge.groupChildren(indexId).items;
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

  void setFilter(String value) {
    state = _recompute(state.copyWith(filter: value));
  }

  void submitFilter() {
    state = _recompute(state.copyWith(filter: state.filter));
    _log('refresh', 'filter="${state.filter}" -> ${state.visible.length}');
  }

  void sortBy(String key) {
    state = _recompute(state.copyWith(sort: state.sort.next(key)));
    _persistOrder();
    _log('sort', '$key ${state.sort.direction.name}');
  }

  /// Persist the current visible row order into the upstream
  /// `ProfileExItem.Sort` field (PR-15 / FIX-10B).
  ///
  /// Mirrors `ConfigHandler.SortServers`/`MoveServer`: the whole visible list
  /// is written as `(position + 1) * 10` through the real
  /// `speedtestApplyProfileOrder` bridge, so the order survives a restart. An
  /// empty list or a single row carries no ordering information and is never
  /// written (no fake persistence, no error).
  void _persistOrder() {
    final ids = state.visible.map((r) => r.id).toList();
    if (ids.length < 2) return;
    _bridge.applyProfileOrder(ids);
  }

  /// `按测试结果排序` (ACT-PROF-020): order the visible rows by the measured
  /// delay ascending, unknown/failed delays (-1) last. Operates on the real
  /// `ProfileSummary.delay` from the speedtest result overlay; no synthetic
  /// ordering is produced.
  void sortByResult() {
    final rows = List<ProfileSummary>.of(state.visible);
    rows.sort((a, b) {
      final ad = a.delay < 0 ? 1 << 30 : a.delay;
      final bd = b.delay < 0 ? 1 << 30 : b.delay;
      return ad.compareTo(bd);
    });
    state = state.copyWith(visible: rows);
    _persistOrder();
    _log('sort-result', 'rows=${rows.length}');
    _echo('sort-result');
  }

  void selectRow(String id, {bool ctrl = false, bool shift = false}) {
    Set<String> next;
    if (shift) {
      next = extendSelection(state.visible, state.selected, id);
    } else if (ctrl) {
      next = toggleSelection(state.selected, id);
    } else {
      next = selectSingle(id);
    }
    state = state.copyWith(selected: next);
    _log('select', 'id=$id selected=${next.length}');
  }

  /// Drag-select: replace the selection with the inclusive visible range
  /// between the drag anchor and the row under the pointer (WPF DataGrid
  /// press-and-drag parity).
  void selectRange(String anchorId, String currentId) {
    final next = rangeSelection(state.visible, anchorId, currentId);
    if (setEquals(next, state.selected)) return;
    state = state.copyWith(selected: next);
    _log('select', 'range=$anchorId..$currentId selected=${next.length}');
  }

  void selectAll() {
    final ids = state.visible.map((r) => r.id).toSet();
    state = state.copyWith(selected: ids);
    _log(ProfileAction.selectAll, 'selected=${ids.length}');
    _echo(ProfileAction.selectAll);
  }

  void clearSelection() {
    state = state.copyWith(selected: const <String>{});
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
    var index = state.selected.isEmpty
        ? (delta > 0 ? -1 : rows.length)
        : rows.indexWhere((r) => state.selected.contains(r.id));
    if (index < 0 && state.selected.isNotEmpty) {
      index = delta > 0 ? -1 : rows.length;
    }
    final target = (index + delta).clamp(0, rows.length - 1);
    final id = rows[target].id;
    state = state.copyWith(selected: selectSingle(id));
    _log(
      delta < 0 ? ProfileAction.navigateUp : ProfileAction.navigateDown,
      'id=$id',
    );
  }

  void handleRightTap(String id) {
    if (!state.selected.contains(id)) {
      state = state.copyWith(selected: selectSingle(id));
    }
    _log(ProfileAction.contextMenu, 'kept=${state.selected.length}');
  }

  /// Re-bind the selection to the target ids captured when the context menu
  /// opened, so an open menu always commands the rows it was opened on even if
  /// the selection drifted meanwhile.
  ///
  /// Returns false without touching the selection when the snapshot is empty or
  /// any target id no longer exists (refresh/delete/filter); the caller then
  /// closes the stale menu instead of acting on a hidden/other row.
  bool restoreContextTargets(List<String> ids) {
    if (ids.isEmpty) return false;
    final known = state.all.map((r) => r.id).toSet();
    if (!ids.every(known.contains)) return false;
    final target = ids.toSet();
    if (!setEquals(state.selected, target)) {
      state = state.copyWith(selected: target);
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
      case ProfileAction.escape:
        // ACT-PROF-038: Esc stops a running test first, then clears the
        // selection (upstream `LstProfiles_PreviewKeyDown(Escape)`).
        if (state.speedTestRunning) {
          cancelSpeedTest();
        }
        clearSelection();
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
        clearSpeedTestJob: true,
      );
      _log('speedtest-start-failed', code);
      _echo(action);
      return c.SimpleResult(ok: false, error: result.error);
    }

    final started = result.jobId != null;
    _speedTestTargets = started ? scopeIds : const <String>[];
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
      clearSpeedTestJob: !started,
    );
    _log(
      action,
      'kind=$kind ids=${ids.length} scope=${scopeIds.length} '
      'job=${result.jobId ?? "-"}',
    );
    _echo(action);
    if (started) {
      _startTestPolling();
    }
    return const c.SimpleResult(ok: true);
  }

  void cancelSpeedTest() {
    final jobId = state.speedTestJobId;
    if (jobId == null) return;
    _bridge.cancelSpeedTest(jobId);
    _testPoller?.cancel();
    _testPoller = null;
    state = state.copyWith(
      speedTestRunning: false,
      speedTestStage: 'SpeedtestingStop',
      speedTestMessage: '已停止测速',
    );
    _log('speedtest-stop', 'job=$jobId');
    _echo(ProfileAction.stopTest);
  }

  /// `按测试结果移除无效` (ACT-PROF-021 / PR-11): really delete the failed
  /// `ProfileItem`s of the current group, not just the test-result rows.
  ///
  /// Upstream `ConfigHandler.RemoveInvalidServerResult` finds every profile in
  /// the current `subid` whose `ProfileExItem.Delay == -1` (complex nodes are
  /// excluded) and removes the profile. Here the failed ids come from the live
  /// result overlay ([speedTestResults]); only nodes present in the current
  /// group are eligible, so a hidden group's results never delete a visible
  /// node. The result rows are cleared afterwards (existing
  /// `speedtest_remove_invalid`).
  ///
  /// Returns the number of profiles actually deleted.
  int removeInvalidResults() {
    final failedIds = _bridge
        .speedTestResults()
        .where((r) => r.delay == -1)
        .map((r) => r.indexId)
        .toSet();
    // Only delete stored, non-complex profiles that belong to the current view
    // (upstream `RemoveInvalidServerResult` skips complex nodes).
    final targets = state.profiles
        .where((p) => failedIds.contains(p.indexId))
        .where((p) => !isComplexProfile(p.configType))
        .where((p) => _profileInCurrentGroup(p))
        .map((p) => p.indexId)
        .toList();
    var removed = 0;
    if (targets.isNotEmpty) {
      final result = _bridge.deleteProfiles(targets);
      if (result.ok) removed = result.removed.toInt();
    }
    _bridge.removeInvalidResults();
    reload();
    _log(
      ProfileAction.removeInvalid,
      'profiles=$removed candidates=${targets.length}',
    );
    _echo(ProfileAction.removeInvalid);
    return removed;
  }

  /// Whether a stored profile belongs to the current group/filter view.
  ///
  /// Mirrors the visible-row projection so batch actions never touch a hidden
  /// group's node when the table shows a different group (PR-18).
  bool _profileInCurrentGroup(c.ProfileDto profile) {
    final group = state.groupSubId;
    if (group != null && profile.subid != group) return false;
    if (state.filter.trim().isEmpty) return true;
    return rowMatchesQuery(dtoToSummary(profile), state.filter);
  }

  /// `移除重复` (ACT-PROF-003 / PR-12): deduplicate the current group by
  /// transport identity, keeping the older node when `KeepOlderDedupl` is set
  /// (complex nodes always stay). Returns the number of profiles removed.
  ///
  /// The comparison mirrors the frozen `ConfigHandler.CompareProfileItem` and
  /// the existing `subscriptions::deduplicate` pure function; the deletion goes
  /// through the real `deleteProfiles` seam, so it persists and survives a
  /// restart.
  int removeDuplicateProfiles({bool keepOlder = true}) {
    final candidates = state.profiles.where(_profileInCurrentGroup).toList();
    final duplicates = deduplicateProfiles(candidates, keepOlder: keepOlder);
    if (duplicates.isEmpty) {
      _log(ProfileAction.removeDuplicate, 'none');
      _echo(ProfileAction.removeDuplicate);
      return 0;
    }
    final result = _bridge.deleteProfiles(duplicates);
    final removed = result.ok ? result.removed.toInt() : 0;
    reload();
    _log(ProfileAction.removeDuplicate, 'removed=$removed');
    _echo(ProfileAction.removeDuplicate);
    return removed;
  }

  Future<void> runBlockingProbe(int ms) async {
    state = state.copyWith(blockingBusy: true);
    _log('blocking-start', 'ms=$ms');
    final elapsed = await _bridge.simulateBlocking(ms);
    state = state.copyWith(blockingBusy: false, lastBlockingMs: elapsed);
    _log('blocking-done', 'elapsed=$elapsed ms');
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
