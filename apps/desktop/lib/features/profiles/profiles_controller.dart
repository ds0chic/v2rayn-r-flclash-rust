import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
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
    );
  }
}

class ProfilesController extends Notifier<ProfilesState> {
  static const columnSection = 'column_layout';

  BridgePort get _bridge => ref.read(bridgePortProvider);
  UiStateStore get _store => ref.read(uiStateStoreProvider);

  int _seq = 0;

  @override
  ProfilesState build() {
    final count = ref.read(profileRowCountProvider);
    final rows = _bridge.fetchSummaries(count);
    final columns = _applyStoredLayout(defaultProfileColumns());
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

  // -- T06a profile editor / repository actions ---------------------------

  /// A fresh draft for a new node of [configType].
  ProfileDraft newDraft(ConfigType configType) {
    return ProfileDraft()
      ..configType = configType
      ..coreType = ProfileCapabilities.defaultCore(configType)
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
  void reload() {
    final count = ref.read(profileRowCountProvider);
    final rows = _bridge.fetchSummaries(count);
    final filtered = applyFilter(rows, state.filter);
    final sorted = applySort(filtered, state.visibleColumns, state.sort);
    state = state.copyWith(
      all: rows,
      visible: sorted,
      profiles: _bridge.queryAllProfiles(),
      activeId: _bridge.getActiveProfile(),
      clearActive: _bridge.getActiveProfile() == null,
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
    return base.copyWith(visible: sorted);
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
  int genGroupRegion(String subId) {
    final result = _bridge.genGroupRegion(subId);
    if (result.ok) {
      reload();
      _log('gen-group-region', 'sub=$subId count=${result.profiles.length}');
      return result.profiles.length;
    }
    return -1;
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
    _log('sort', '$key ${state.sort.direction.name}');
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

  void handleRightTap(String id) {
    if (!state.selected.contains(id)) {
      state = state.copyWith(selected: selectSingle(id));
    }
    _log(ProfileAction.contextMenu, 'kept=${state.selected.length}');
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

  void handleDragStart(String id) {
    _log(ProfileAction.dragStart, 'id=$id');
  }

  void handleDrop(String sourceId, String targetId) {
    _log(ProfileAction.drop, '$sourceId -> $targetId');
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
        clearSelection();
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
    state = _recompute(state.copyWith(all: next));
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
