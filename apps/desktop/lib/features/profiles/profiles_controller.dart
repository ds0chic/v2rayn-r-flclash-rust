import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
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
    final rows = _bridge.generate(count);
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
    );
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
    final sorted = applySort(filtered, base.visibleColumns, base.sort);
    return base.copyWith(visible: sorted);
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
