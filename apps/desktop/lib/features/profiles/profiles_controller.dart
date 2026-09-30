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
  final List<ProfileColumn> columns;
  final int? lastAckSeq;
  final int? lastAckRustCount;
  final bool blockingBusy;
  final int? lastBlockingMs;

  int get selectedCount => selected.length;
  int get totalCount => all.length;
  TableEvent? get lastEvent => events.isEmpty ? null : events.last;

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
  BridgePort get _bridge => ref.read(bridgePortProvider);
  UiStateStore get _store => ref.read(uiStateStoreProvider);

  int _seq = 0;

  @override
  ProfilesState build() {
    final count = ref.read(profileRowCountProvider);
    final rows = _bridge.generate(count);
    final columns = _applyStoredWidths(defaultProfileColumns());
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

  List<ProfileColumn> _applyStoredWidths(List<ProfileColumn> columns) {
    final stored = _store.loadColumnWidths();
    if (stored.isEmpty) return columns;
    return columns
        .map(
          (c) => stored.containsKey(c.key)
              ? ProfileColumn(
                  key: c.key,
                  title: c.title,
                  width: stored[c.key]!,
                  numeric: c.numeric,
                  display: c.display,
                )
              : c,
        )
        .toList();
  }

  ProfilesState _recompute(ProfilesState base) {
    final filtered = applyFilter(base.all, base.filter);
    final sorted = applySort(filtered, base.columns, base.sort);
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
    final updated = state.columns.map((c) {
      if (c.key != key) return c;
      final width = (c.width + delta).clamp(40.0, 600.0);
      return ProfileColumn(
        key: c.key,
        title: c.title,
        width: width,
        numeric: c.numeric,
        display: c.display,
      );
    }).toList();
    state = state.copyWith(columns: updated);
    _store.saveColumnWidths(<String, double>{
      for (final c in updated) c.key: c.width,
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
