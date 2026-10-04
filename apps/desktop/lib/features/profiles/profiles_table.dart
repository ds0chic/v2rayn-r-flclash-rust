import 'dart:async';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:two_dimensional_scrollables/two_dimensional_scrollables.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/command_context.dart';
import 'package:v2rayn_desktop/features/profiles/context_menu.dart';
import 'package:v2rayn_desktop/features/profiles/profile_actions.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
import 'package:v2rayn_desktop/features/subs/subs_actions.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';
import 'package:v2rayn_desktop/shared/widgets/app_dialog.dart';
import 'package:v2rayn_desktop/shared/widgets/context_menu_session.dart';
import 'package:v2rayn_desktop/shared/widgets/empty_state.dart';

import 'profiles_controller.dart';

/// Real two-dimensional virtualized profile table. Only the visible cells are
/// built, so 10k+ rows render without materializing 14*k widgets.
class ProfilesTable extends ConsumerStatefulWidget {
  const ProfilesTable({
    super.key,
    this.verticalController,
    this.horizontalController,
  });

  final ScrollController? verticalController;
  final ScrollController? horizontalController;

  @override
  ConsumerState<ProfilesTable> createState() => _ProfilesTableState();
}

class _ProfilesTableState extends ConsumerState<ProfilesTable>
    with WidgetsBindingObserver {
  static const _headerHeight = AppTokens.tableHeaderHeight;

  /// Upstream `App.xaml` `MenuItemHeight=32`; every root and submenu row uses
  /// this exact height so the icon/text/shortcut/arrow columns line up.
  static const double _menuItemHeight = 32;

  /// Initial horizontal padding inside a menu row (upstream 10-12).
  static const double _menuItemPadding = 12;

  /// Forces every `MenuItemButton`/`SubmenuButton` box to the upstream 32-px
  /// row height; Material's default `visualDensity` otherwise yields 40.
  static const ButtonStyle _menuRowStyle = ButtonStyle(
    minimumSize: WidgetStatePropertyAll(Size(0, _menuItemHeight)),
    maximumSize: WidgetStatePropertyAll(Size.infinite),
    fixedSize: WidgetStatePropertyAll(Size.fromHeight(_menuItemHeight)),
    padding: WidgetStatePropertyAll(EdgeInsets.zero),
    tapTargetSize: MaterialTapTargetSize.shrinkWrap,
    alignment: Alignment.centerLeft,
  );

  /// Row height follows the configured base font size (24/26/28) exposed on
  /// the theme extension, keeping the desktop table compact.
  double get _rowHeight => context.semantics.tableRowHeight;

  final FocusNode _focusNode = FocusNode(debugLabel: 'profiles-table');
  final MenuController _menuController = MenuController();

  /// The anchor's render box, used to convert the global pointer position to
  /// the MenuAnchor-local coordinates its `open(position:)` requires.
  final GlobalKey _anchorBoxKey = GlobalKey(debugLabel: 'profiles-menu-anchor');

  /// Immutable command target captured when the menu opens. Cleared on close,
  /// but the command closure already holds its own reference, so closing the
  /// menu never destroys the business target (FIX-01 / PR-19).
  CommandContext? _commandContext;

  /// Keyboard highlight index within the root menu while it is open (-1 = no
  /// highlight yet). Arrow keys move it, Enter activates the highlighted row.
  int _menuFocusIndex = -1;

  /// Root entries actually rendered for the open menu (with the live
  /// move-to-group submenu injected). Used by keyboard navigation because the
  /// constant [profilesContextMenu] does not carry the runtime group list.
  List<ContextMenuEntry> _activeRootEntries = const <ContextMenuEntry>[];

  /// Focus nodes for each rendered root row, so ↑/↓ can move the visible
  /// highlight and Enter can activate the same row the pointer would.
  final List<FocusNode> _menuRowFocusNodes = <FocusNode>[];

  late final ScrollController _vertical =
      widget.verticalController ?? ScrollController();
  late final ScrollController _horizontal =
      widget.horizontalController ?? ScrollController();
  bool _ownsVertical = false;
  bool _ownsHorizontal = false;

  /// Drag-select state: the row pressed when a primary-button drag started
  /// (upstream WPF DataGrid press-and-drag range selection). Cleared on
  /// pointer up/cancel.
  String? _dragAnchorId;
  bool _dragSelecting = false;

  /// Edge auto-scroll while drag-selecting beyond the viewport (-1 up, +1
  /// down). The timer scrolls one third of a row per frame and extends the
  /// range to the row entering the viewport, matching the WPF DataGrid.
  Timer? _dragScrollTimer;
  int _dragScrollDirection = 0;

  @override
  void initState() {
    super.initState();
    _ownsVertical = widget.verticalController == null;
    _ownsHorizontal = widget.horizontalController == null;
    WidgetsBinding.instance.addObserver(this);
  }

  @override
  void dispose() {
    _stopDragAutoScroll();
    WidgetsBinding.instance.removeObserver(this);
    _focusNode.dispose();
    for (final node in _menuRowFocusNodes) {
      node.dispose();
    }
    if (_ownsVertical) _vertical.dispose();
    if (_ownsHorizontal) _horizontal.dispose();
    super.dispose();
  }

  /// Window lifecycle (UX-CTX-03): a lost/inactive window must not leave the
  /// context menu hanging over stale content, and reactivating the window must
  /// never restore it. Windows minimization reports `inactive`/`hidden`.
  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (_menuController.isOpen &&
        (state == AppLifecycleState.inactive ||
            state == AppLifecycleState.hidden ||
            state == AppLifecycleState.paused ||
            state == AppLifecycleState.detached)) {
      _menuController.close();
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(profilesControllerProvider);
    final columns = state.visibleColumns;
    final rows = state.visible;
    // `UiItem.EnableDragDropSort`: only register row drag-reorder when the
    // persisted setting is true (upstream `ProfilesView.xaml:27-34`).
    final dragSortEnabled = ref.watch(profilesEnableDragDropSortProvider);

    return MenuAnchor(
      controller: _menuController,
      menuChildren: _buildContextMenu(
        context,
        _activeRootEntries.isEmpty ? profilesContextMenu : _activeRootEntries,
        _commandContext,
      ),
      // Constrain the panel to the window so a long label never pushes it past
      // the visible edge; the actual width is measured from the rendered
      // labels by the shared menu style. `visualDensity` cannot set the row
      // height, so each row carries its own 32-px box.
      style: MenuStyle(
        maximumSize: WidgetStatePropertyAll(
          Size(
            (MediaQuery.sizeOf(context).width - 16).clamp(160, 560),
            MediaQuery.sizeOf(context).height - 16,
          ),
        ),
      ),
      onClose: _onMenuClosed,
      child: Focus(
        focusNode: _focusNode,
        autofocus: true,
        onKeyEvent: _onKey,
        child: Listener(
          key: _anchorBoxKey,
          behavior: HitTestBehavior.opaque,
          onPointerDown: _onPointerDown,
          onPointerMove: _onPointerMove,
          onPointerUp: _onPointerUp,
          onPointerCancel: _onPointerUp,
          child: LayoutBuilder(
            builder: (context, constraints) {
              final widths = fittedColumnWidths(
                columns,
                constraints.maxWidth,
                fixedChrome: AppTokens.tableHandleWidth,
              );
              return Stack(
                children: <Widget>[
                  TableView.builder(
                    verticalDetails: ScrollableDetails.vertical(
                      controller: _vertical,
                    ),
                    horizontalDetails: ScrollableDetails.horizontal(
                      controller: _horizontal,
                    ),
                    pinnedRowCount: 1,
                    pinnedColumnCount: 1,
                    columnCount: columns.length + 1,
                    rowCount: rows.length + 1,
                    columnBuilder: (index) =>
                        _buildColumnSpan(context, index, widths),
                    rowBuilder: (index) => _buildRowSpan(index, context),
                    cellBuilder: (context, vicinity) => TableViewCell(
                      child: _buildCell(
                        context,
                        vicinity,
                        state,
                        columns,
                        rows,
                        dragSortEnabled,
                      ),
                    ),
                  ),
                  if (rows.isEmpty)
                    Positioned(
                      top: _headerHeight,
                      left: 0,
                      right: 0,
                      bottom: 0,
                      child: const IgnorePointer(
                        child: EmptyState(
                          message: '暂无节点',
                          semanticIcon: 'empty',
                          detail: '可从剪贴板导入或添加节点',
                          messageKey: ValueKey('profiles-empty'),
                        ),
                      ),
                    ),
                ],
              );
            },
          ),
        ),
      ),
    );
  }

  TableSpan _buildColumnSpan(
    BuildContext context,
    int index,
    List<double> widths,
  ) {
    final width = index == 0 ? AppTokens.tableHandleWidth : widths[index - 1];
    final grid = context.semantics.gridLine;
    return TableSpan(
      extent: FixedTableSpanExtent(width),
      foregroundDecoration: TableSpanDecoration(
        border: TableSpanBorder(trailing: BorderSide(color: grid, width: 1)),
      ),
    );
  }

  TableSpan _buildRowSpan(int index, BuildContext context) {
    final isHeader = index == 0;
    final semantics = context.semantics;
    // Optional zebra striping; data rows only (row 0 is the header).
    final zebra = semantics.zebraEnabled && !isHeader && (index - 1).isOdd;
    return TableSpan(
      extent: FixedTableSpanExtent(isHeader ? _headerHeight : _rowHeight),
      backgroundDecoration: TableSpanDecoration(
        color: isHeader
            ? Theme.of(context).colorScheme.surfaceContainerHighest
            : (zebra ? semantics.zebraStripe : null),
        border: TableSpanBorder(
          trailing: BorderSide(color: semantics.gridLine, width: 1),
        ),
      ),
    );
  }

  Widget _buildCell(
    BuildContext context,
    TableVicinity vicinity,
    ProfilesState state,
    List<ProfileColumn> columns,
    List<ProfileSummary> rows,
    bool dragSortEnabled,
  ) {
    if (vicinity.row == 0) {
      return _headerCell(context, vicinity.column, state, columns);
    }
    final dataIndex = vicinity.row - 1;
    if (dataIndex >= rows.length) return const SizedBox.shrink();
    final row = rows[dataIndex];
    if (vicinity.column == 0) {
      return _handleCell(context, row, dataIndex, state, dragSortEnabled);
    }
    final column = columns[vicinity.column - 1];
    return _dataCell(context, row, column, state);
  }

  Widget _headerCell(
    BuildContext context,
    int columnIndex,
    ProfilesState state,
    List<ProfileColumn> columns,
  ) {
    if (columnIndex == 0) {
      return Container(
        key: const ValueKey('header-handle'),
        alignment: Alignment.center,
        child: const Text('#', style: TextStyle(fontWeight: FontWeight.w600)),
      );
    }
    final column = columns[columnIndex - 1];
    final sorted =
        state.sort.columnKey == column.key &&
        state.sort.direction != SortDirection.none;
    final indicator = !sorted
        ? ''
        : (state.sort.direction == SortDirection.ascending
              ? ' \u25B2'
              : ' \u25BC');
    return Stack(
      children: <Widget>[
        Positioned.fill(
          child: GestureDetector(
            key: ValueKey('header-${column.title}'),
            behavior: HitTestBehavior.opaque,
            onTap: () => _sortWithAnchor(column.key, state),
            child: Align(
              alignment: Alignment.centerLeft,
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 6),
                child: Text(
                  '${column.title}$indicator',
                  style: const TextStyle(
                    fontWeight: FontWeight.w600,
                    fontSize: 12,
                  ),
                  overflow: TextOverflow.clip,
                  softWrap: false,
                ),
              ),
            ),
          ),
        ),
        Positioned(
          top: 0,
          bottom: 0,
          right: 0,
          width: 8,
          child: MouseRegion(
            cursor: SystemMouseCursors.resizeColumn,
            child: GestureDetector(
              key: ValueKey('resize-${column.title}'),
              behavior: HitTestBehavior.opaque,
              onHorizontalDragUpdate: (details) => ref
                  .read(profilesControllerProvider.notifier)
                  .resizeColumn(column.key, details.delta.dx),
            ),
          ),
        ),
      ],
    );
  }

  /// Stable sort that keeps the row currently at the top scroll position in
  /// place, so the viewport anchor survives the reorder (plan §08).
  /// Keyboard scope: editor-level actions open dialogs; selection/navigation
  /// fall through to the controller. Text fields keep their own scope (HKR-002).
  KeyEventResult _onKey(FocusNode node, KeyEvent event) {
    if (_menuController.isOpen) {
      // The menu owns the keyboard while it is open. Esc closes the whole chain
      // (upstream submenu Esc-return semantics are not real-machine verified
      // yet; registered as an open point) and restores table focus without
      // leaking into the table shortcuts. Arrow/Enter move the menu highlight
      // instead of reaching the table, so the table never activates a row.
      if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
        return KeyEventResult.handled;
      }
      switch (event.logicalKey) {
        case LogicalKeyboardKey.escape:
          _closeMenuChain();
          return KeyEventResult.handled;
        case LogicalKeyboardKey.arrowDown:
          _moveMenuHighlight(1);
          return KeyEventResult.handled;
        case LogicalKeyboardKey.arrowUp:
          _moveMenuHighlight(-1);
          return KeyEventResult.handled;
        case LogicalKeyboardKey.enter:
        case LogicalKeyboardKey.numpadEnter:
          _activateMenuHighlight();
          return KeyEventResult.handled;
      }
      return KeyEventResult.handled;
    }
    if (event is KeyDownEvent || event is KeyRepeatEvent) {
      final keyboard = HardwareKeyboard.instance;
      final action = actionForKey(
        key: event.logicalKey,
        ctrl: keyboard.isControlPressed,
        shift: keyboard.isShiftPressed,
        alt: keyboard.isAltPressed,
      );
      final controller = ref.read(profilesControllerProvider.notifier);
      switch (action) {
        case ProfileAction.edit:
          controller.logAction(action!, 'keyboard');
          editSelectedProfile(context, ref);
          return KeyEventResult.handled;
        case ProfileAction.delete:
          controller.logAction(action!, 'keyboard');
          deleteSelectedProfiles(context, ref);
          return KeyEventResult.handled;
        case ProfileAction.exportShareUrl:
          // Ctrl+C: export the selected nodes' share URIs to the clipboard
          // (upstream `Export2ShareUrlAsync(false)`), never the clone command.
          controller.logAction(action!, 'keyboard');
          exportSelectedShareUrls(ref);
          return KeyEventResult.handled;
        case ProfileAction.share:
          // Ctrl+F: open the main row's share QR window (upstream
          // `ShareServerAsync`), not the default log/echo.
          controller.logAction(action!, 'keyboard');
          shareProfileQr(context, ref);
          return KeyEventResult.handled;
        case ProfileAction.activate:
          controller.logAction(action!, 'keyboard');
          setActiveSelected(ref);
          return KeyEventResult.handled;
      }
    }
    return ref.read(profilesControllerProvider.notifier).handleKeyEvent(event)
        ? KeyEventResult.handled
        : KeyEventResult.ignored;
  }

  /// Close the menu chain from the keyboard, restoring table focus and
  /// clearing the highlight. Used by Esc and by the window lifecycle observer.
  void _closeMenuChain() {
    _menuController.close();
    _menuFocusIndex = -1;
    _focusNode.requestFocus();
  }

  /// Move the visible menu highlight by [delta] rows within the open root menu.
  void _moveMenuHighlight(int delta) {
    final count = _activeRootEntries.length;
    if (count == 0) return;
    var next = _menuFocusIndex + delta;
    if (_menuFocusIndex < 0) next = delta > 0 ? 0 : count - 1;
    if (next < 0) next = count - 1;
    if (next >= count) next = 0;
    // Keep focus on the table so subsequent keys reach [_onKey]; the highlight
    // is purely visual state. Submenus take focus only on activation, below.
    setState(() => _menuFocusIndex = next);
  }

  /// Activate the highlighted root row. A plain entry runs its command; a
  /// submenu opens (the Material menu handles the cascade via focus/hover). A
  /// disabled row does nothing, matching the mouse path.
  void _activateMenuHighlight() {
    if (_menuFocusIndex < 0 || _menuFocusIndex >= _activeRootEntries.length) {
      _moveMenuHighlight(1);
      return;
    }
    final entry = _activeRootEntries[_menuFocusIndex];
    final enabled =
        entry.enabled &&
        (!_requiresTarget(entry.kind) ||
            (_commandContext?.hasTargets ?? false));
    if (!enabled) return;
    if (entry.isSubmenu) {
      if (_menuFocusIndex < _menuRowFocusNodes.length) {
        _menuRowFocusNodes[_menuFocusIndex].requestFocus();
      }
      return;
    }
    _onContextAction(entry, _commandContext);
  }

  void _onDoubleTap(ProfileSummary row) {
    final controller = ref.read(profilesControllerProvider.notifier);
    final state = ref.read(profilesControllerProvider);
    controller.selectRow(row.id);
    final action = state.doubleClick2Activate
        ? ProfileAction.activate
        : ProfileAction.edit;
    controller.logAction(action, row.id);
    if (state.doubleClick2Activate) {
      setActiveSelected(ref);
    } else {
      editSelectedProfile(context, ref);
    }
  }

  void _sortWithAnchor(String key, ProfilesState state) {
    final rows = state.visible;
    double offset = 0;
    if (_vertical.hasClients) offset = _vertical.offset;
    final topIndex = rows.isEmpty
        ? 0
        : (offset / _rowHeight).floor().clamp(0, rows.length - 1);
    final anchorId = rows.isEmpty ? null : rows[topIndex].id;

    ref.read(profilesControllerProvider.notifier).sortBy(key);

    if (anchorId == null || !_vertical.hasClients) return;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final next = ref.read(profilesControllerProvider).visible;
      final newIndex = next.indexWhere((r) => r.id == anchorId);
      if (newIndex < 0) return;
      final position = _vertical.position;
      final target = (newIndex * _rowHeight).clamp(
        0.0,
        position.maxScrollExtent,
      );
      _vertical.jumpTo(target);
    });
  }

  Widget _dataCell(
    BuildContext context,
    ProfileSummary row,
    ProfileColumn column,
    ProfilesState state,
  ) {
    final selected = state.selected.contains(row.id);
    // Active node marker, independent from the multi-select highlight: an
    // active row keeps its own fill even when a different row is selected.
    final isActive = state.activeId == row.id;
    final controller = ref.read(profilesControllerProvider.notifier);
    final value = column.display(row);
    final label = Text(
      value,
      overflow: TextOverflow.ellipsis,
      maxLines: 1,
      style: const TextStyle(fontSize: AppTokens.fontSize),
    );
    return GestureDetector(
      key: ValueKey('cell-${row.id}-${column.key}'),
      behavior: HitTestBehavior.opaque,
      onTap: () => controller.selectRow(
        row.id,
        ctrl: HardwareKeyboard.instance.isControlPressed,
        shift: HardwareKeyboard.instance.isShiftPressed,
      ),
      onDoubleTap: () => _onDoubleTap(row),
      child: Container(
        color: selected
            ? context.semantics.selectedRow
            : (isActive ? activeRowFill(context) : null),
        alignment: column.numeric
            ? Alignment.centerRight
            : Alignment.centerLeft,
        padding: const EdgeInsets.symmetric(horizontal: 6),
        // Only long cells carry a tooltip so hover stays quiet in dense tables.
        child: value.length > 12
            ? Tooltip(
                message: value,
                waitDuration: const Duration(milliseconds: 600),
                child: label,
              )
            : label,
      ),
    );
  }

  Widget _handleCell(
    BuildContext context,
    ProfileSummary row,
    int index,
    ProfilesState state,
    bool dragSortEnabled,
  ) {
    final selected = state.selected.contains(row.id);
    final isActive = state.activeId == row.id;
    final controller = ref.read(profilesControllerProvider.notifier);
    final ordinal = Text('${index + 1}', style: const TextStyle(fontSize: 11));

    final numberBox = Container(
      color: selected ? Theme.of(context).colorScheme.primaryContainer : null,
      alignment: Alignment.center,
      child: dragSortEnabled
          ? Draggable<ProfileSummary>(
              data: row,
              dragAnchorStrategy: pointerDragAnchorStrategy,
              onDragStarted: () {
                _menuController.close();
                controller.handleDragStart(row.id);
              },
              feedback: Material(
                elevation: 4,
                child: Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 4,
                  ),
                  color: Theme.of(context).colorScheme.secondaryContainer,
                  child: Text(row.remarks),
                ),
              ),
              childWhenDragging: Opacity(opacity: 0.3, child: ordinal),
              child: ordinal,
            )
          : ordinal,
    );

    // A solid leading marker independent from the selectable row fill, so the
    // active node stays visible even when the same row is also selected.
    final cell = Row(
      children: <Widget>[
        SizedBox(
          width: 3,
          child: ColoredBox(
            key: ValueKey('active-marker-${row.id}'),
            color: isActive
                ? activeRowMarkerColor(context)
                : Colors.transparent,
          ),
        ),
        Expanded(child: numberBox),
      ],
    );

    if (!dragSortEnabled) {
      return Container(key: ValueKey('handle-${row.id}'), child: cell);
    }

    return DragTarget<ProfileSummary>(
      key: ValueKey('drop-${row.id}'),
      onAcceptWithDetails: (details) =>
          controller.handleDrop(details.data.id, row.id),
      builder: (context, candidate, rejected) => Container(
        key: ValueKey('handle-${row.id}'),
        foregroundDecoration: candidate.isEmpty
            ? null
            : BoxDecoration(
                border: Border.all(
                  color: Theme.of(context).colorScheme.primary,
                  width: 2,
                ),
              ),
        child: cell,
      ),
    );
  }

  /// Pointer-down at the table level. The table sits inside the menu's
  /// [TapRegion] group, so Flutter's default outside-tap never fires for it;
  /// this explicitly closes the whole menu chain, then lets the same pointer
  /// down continue as a normal interaction (select another row, open the menu
  /// on a new target, focus the table, ...).
  void _onPointerDown(PointerDownEvent event) {
    if ((event.buttons & kSecondaryButton) != 0) {
      _openContextMenu(event);
      return;
    }
    if ((event.buttons & kPrimaryButton) != 0) {
      // Start a possible drag-select (WPF DataGrid press-and-drag). The handle
      // column keeps its own reorder drag, so it is excluded.
      if (event.localPosition.dx >= AppTokens.tableHandleWidth) {
        final state = ref.read(profilesControllerProvider);
        _dragAnchorId = _rowIdAt(event.localPosition, state.visible);
        _dragSelecting = false;
      } else {
        _dragAnchorId = null;
      }
    }
    if (_menuController.isOpen) {
      // Close the whole menu chain for a table click, but defer the rebuild to
      // the next frame so the in-flight tap gesture still completes and selects
      // the clicked row (close + normal handling).
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && _menuController.isOpen) {
          _menuController.close();
        }
      });
    }
    _focusNode.requestFocus();
  }

  /// Row under a table-local pointer position, or null for header/empty area.
  String? _rowIdAt(Offset local, List<ProfileSummary> rows) {
    if (local.dy < _headerHeight) return null;
    final offset = _vertical.hasClients ? _vertical.offset : 0.0;
    final contentY = local.dy - _headerHeight + offset;
    if (contentY < 0) return null;
    final index = contentY ~/ _rowHeight;
    if (index < 0 || index >= rows.length) return null;
    return rows[index].id;
  }

  void _onPointerMove(PointerMoveEvent event) {
    final anchorId = _dragAnchorId;
    if (anchorId == null || (event.buttons & kPrimaryButton) == 0) return;
    if (_vertical.hasClients) {
      final position = _vertical.position;
      final contentTop = _headerHeight;
      final contentBottom = contentTop + position.viewportDimension;
      if (event.localPosition.dy < contentTop) {
        _startDragAutoScroll(-1, anchorId);
        return;
      }
      if (event.localPosition.dy > contentBottom) {
        _startDragAutoScroll(1, anchorId);
        return;
      }
    }
    _stopDragAutoScroll();
    final state = ref.read(profilesControllerProvider);
    final currentId = _rowIdAt(event.localPosition, state.visible);
    if (currentId == null) return;
    if (!_dragSelecting && currentId == anchorId) return;
    _dragSelecting = true;
    ref
        .read(profilesControllerProvider.notifier)
        .selectRange(anchorId, currentId);
  }

  /// Auto-scroll while the drag-select pointer is held beyond an edge. Extends
  /// the range to the first/last row entering the viewport on every tick.
  void _startDragAutoScroll(int direction, String anchorId) {
    if (_dragScrollTimer != null && _dragScrollDirection == direction) return;
    _stopDragAutoScroll();
    _dragScrollDirection = direction;
    _dragSelecting = true;
    _extendRangeToEdge(anchorId, direction);
    _dragScrollTimer = Timer.periodic(const Duration(milliseconds: 16), (_) {
      if (!mounted || _dragAnchorId == null) {
        _stopDragAutoScroll();
        return;
      }
      if (!_vertical.hasClients) return;
      final position = _vertical.position;
      final delta = _dragScrollDirection * (_rowHeight / 3);
      final target = (position.pixels + delta).clamp(
        0.0,
        position.maxScrollExtent,
      );
      if (target != position.pixels) {
        _vertical.jumpTo(target);
      }
      _extendRangeToEdge(anchorId, _dragScrollDirection, atOffset: target);
    });
  }

  void _extendRangeToEdge(String anchorId, int direction, {double? atOffset}) {
    final state = ref.read(profilesControllerProvider);
    final rows = state.visible;
    if (rows.isEmpty || !_vertical.hasClients) return;
    final position = _vertical.position;
    final offset = atOffset ?? position.pixels;
    final firstIndex = (offset / _rowHeight).floor().clamp(0, rows.length - 1);
    final lastIndex = ((offset + position.viewportDimension) / _rowHeight)
        .floor()
        .clamp(0, rows.length - 1);
    final edgeId = direction < 0 ? rows[firstIndex].id : rows[lastIndex].id;
    ref.read(profilesControllerProvider.notifier).selectRange(anchorId, edgeId);
  }

  void _stopDragAutoScroll() {
    _dragScrollTimer?.cancel();
    _dragScrollTimer = null;
    _dragScrollDirection = 0;
  }

  void _onPointerUp(PointerEvent event) {
    _stopDragAutoScroll();
    _dragAnchorId = null;
    _dragSelecting = false;
  }

  /// Open the single context menu for the row (or the current selection when
  /// the trigger lands on a header or empty area) under the pointer.
  void _openContextMenu(PointerDownEvent event) {
    final controller = ref.read(profilesControllerProvider.notifier);
    final state = ref.read(profilesControllerProvider);

    // Convert the global pointer position into the MenuAnchor's local space.
    // MenuController.open(position:) expects anchor-local coordinates; passing
    // the global position double-counts the table origin and shifts the menu.
    final box = _anchorBoxKey.currentContext?.findRenderObject() as RenderBox?;
    final local = box?.globalToLocal(event.position) ?? event.localPosition;

    final rows = state.visible;
    final offset = _vertical.hasClients ? _vertical.offset : 0.0;
    String? rowId;
    ContextMenuRegion region;
    if (local.dy < _headerHeight) {
      region = ContextMenuRegion.columnHeader;
    } else {
      final contentY = local.dy - _headerHeight + offset;
      final index = contentY < 0 ? -1 : contentY ~/ _rowHeight;
      if (index >= 0 && index < rows.length) {
        rowId = rows[index].id;
        region = ContextMenuRegion.data;
      } else {
        region = ContextMenuRegion.empty;
      }
    }

    // The upstream DataGrid.ContextMenu covers the whole grid; a right-click on
    // a row makes that row the target while preserving an existing
    // multi-selection, whereas header/empty keep the current selection.
    if (rowId != null) {
      controller.handleRightTap(rowId);
    }
    final snapshot = ref.read(profilesControllerProvider);
    final command = CommandContext(
      targetIds: snapshot.selected.toList(),
      primaryId:
          rowId ??
          (snapshot.selected.length == 1 ? snapshot.selected.first : null),
      groupSubId: snapshot.groupSubId,
      menuOpenPosition: local,
      viewContext: region,
      focusRestore: _focusNode,
    );
    setState(() {
      _commandContext = command;
      _menuFocusIndex = -1;
      _activeRootEntries = _resolveRootEntries(snapshot);
      _syncMenuRowFocusNodes(_activeRootEntries.length);
    });
    _menuController.open(position: local);
    // Keep the table as the focus owner so Esc/arrows reach [_onKey]; the menu's
    // own focus scope is still reachable by hover/click and by the per-row
    // focus nodes used for keyboard navigation.
    _focusNode.requestFocus();
  }

  void _onMenuClosed() {
    if (!mounted) return;
    // Clearing the on-screen session must not destroy the already-captured
    // command: `_onContextAction` receives its own [CommandContext] reference
    // and runs after this fires.
    setState(() {
      _commandContext = null;
      _menuFocusIndex = -1;
      _activeRootEntries = const <ContextMenuEntry>[];
    });
  }

  /// Inject the live subscription-group list into the `移至订阅分组` submenu.
  ///
  /// The submenu lists every stored subscription group plus a `无分组` entry
  /// (empty `subid`), mirroring the upstream `cmbMoveToGroup` ComboBox. When
  /// the backend cannot persist a move, the entries are rendered disabled with
  /// an honest tooltip instead of faking a change.
  List<ContextMenuEntry> _resolveRootEntries(ProfilesState state) {
    final subs = ref.read(profilesControllerProvider.notifier).subItems();
    final moveEntries = <ContextMenuEntry>[
      if (subs.isEmpty)
        const ContextMenuEntry(
          label: '暂无订阅分组',
          actionId: 'ACT-PROF-013',
          kind: ContextActionKind.notImplemented,
          enabled: false,
          helpTooltip: '没有可移动到的订阅分组',
        ),
      for (final sub in subs)
        ContextMenuEntry(
          label:
              '${sub.remarks.isEmpty ? sub.id : sub.remarks} (${_countFor(state, sub.id)})',
          actionId: 'ACT-PROF-013',
          kind: ContextActionKind.moveToGroup,
          enabled: true,
          targetSubId: sub.id,
        ),
      const ContextMenuEntry(
        label: '无分组',
        actionId: 'ACT-PROF-013',
        kind: ContextActionKind.moveToGroup,
        enabled: true,
        targetSubId: '',
      ),
    ];
    return <ContextMenuEntry>[
      for (final entry in profilesContextMenu)
        if (entry.kind == ContextActionKind.moveToGroup)
          ContextMenuEntry(
            label: entry.label,
            actionId: entry.actionId,
            kind: entry.kind,
            submenu: moveEntries,
          )
        else
          _withRuntimeCapabilities(entry),
    ];
  }

  /// Re-enable entries whose availability is a runtime capability rather than a
  /// missing feature. UDP test follows `SpeedTestSupport.udp`; the two full
  /// client-config exports are always actionable (the backend reports a
  /// structured error when it cannot generate). The entries stay visible so the
  /// upstream menu layout is preserved (RE-PROF-08).
  ContextMenuEntry _withRuntimeCapabilities(ContextMenuEntry entry) {
    final mappedSubmenu = entry.submenu
        .map(_withRuntimeCapabilities)
        .toList(growable: false);
    ContextMenuEntry rebuild({bool? enabled, String? helpTooltip}) =>
        ContextMenuEntry(
          label: entry.label,
          actionId: entry.actionId,
          kind: entry.kind,
          shortcut: entry.shortcut,
          submenu: mappedSubmenu,
          separatorAfter: entry.separatorAfter,
          enabled: enabled ?? entry.enabled,
          helpTooltip: helpTooltip,
          targetSubId: entry.targetSubId,
        );

    switch (entry.kind) {
      case ContextActionKind.udpTest:
        final udp = ref.read(bridgePortProvider).speedTestSupport().udp;
        return rebuild(
          enabled: udp,
          helpTooltip: udp ? null : '当前构建/平台不支持 UDP 测速',
        );
      case ContextActionKind.exportClientConfig:
      case ContextActionKind.exportClientConfigClipboard:
        return rebuild(enabled: true);
      default:
        return mappedSubmenu.isEmpty ? entry : rebuild();
    }
  }

  int _countFor(ProfilesState state, String subId) {
    return state.profiles.where((p) => p.subid == subId).length;
  }

  void _syncMenuRowFocusNodes(int count) {
    while (_menuRowFocusNodes.length < count) {
      _menuRowFocusNodes.add(
        FocusNode(debugLabel: 'ctx-row-${_menuRowFocusNodes.length}'),
      );
    }
  }

  /// Whether an entry resolves its object from the captured node primary
  /// (`CommandContext.primaryId`). These are the entry points that must refuse
  /// a hidden/retargeted row (R3-PROF-01/02); everything else uses batch ids,
  /// the group, or the whole view.
  static bool _usesPrimaryTarget(ContextActionKind kind) {
    switch (kind) {
      case ContextActionKind.edit:
      case ContextActionKind.share:
      case ContextActionKind.activate:
      case ContextActionKind.exportClientConfig:
      case ContextActionKind.exportClientConfigClipboard:
        return true;
      default:
        return false;
    }
  }

  /// Whether an entry needs a live command target. Entries that operate on the
  /// whole view (select all, remove invalid) stay enabled without a selection;
  /// the rest are disabled like the upstream menu.
  static bool _requiresTarget(ContextActionKind kind) {
    switch (kind) {
      case ContextActionKind.selectAll:
      case ContextActionKind.removeInvalid:
      case ContextActionKind.removeDuplicate:
      case ContextActionKind.sortResult:
      case ContextActionKind.genGroupAll:
      case ContextActionKind.genGroupRegion:
      // Full client-config export falls back to the active node when nothing
      // is selected (RE-PROF-08), so it must stay clickable without a target.
      case ContextActionKind.exportClientConfig:
      case ContextActionKind.exportClientConfigClipboard:
      case ContextActionKind.notImplemented:
        return false;
      default:
        return true;
    }
  }

  /// The root menu children, built once per open. `_activeRootEntries` carries
  /// the runtime move-to-group submenu so keyboard navigation and rendering
  /// stay in sync.
  List<Widget> _buildContextMenu(
    BuildContext context,
    List<ContextMenuEntry> entries,
    CommandContext? command,
  ) {
    return _buildMenuLevel(context, entries, command);
  }

  List<Widget> _buildMenuLevel(
    BuildContext context,
    List<ContextMenuEntry> entries,
    CommandContext? command,
  ) {
    final hasTargets = command?.hasTargets ?? false;
    final widgets = <Widget>[];
    for (var i = 0; i < entries.length; i++) {
      final entry = entries[i];
      final enabled =
          entry.enabled && (!_requiresTarget(entry.kind) || hasTargets);
      final highlighted = !entry.isSubmenu && i == _menuFocusIndex;
      if (entry.isSubmenu) {
        widgets.add(
          SubmenuButton(
            key: ValueKey('ctx-${entry.label}'),
            style: _menuRowStyle,
            menuChildren: _buildMenuLevel(context, entry.submenu, command),
            child: _menuLabel(entry, highlighted: false),
          ),
        );
      } else {
        // Bind the immutable command captured when the menu opened. The table's
        // MenuItemButton closes the menu (firing onClose) before running
        // onPressed, so reading the live `_commandContext` field there would
        // already be null; the captured [command] reference survives the close.
        final button = MenuItemButton(
          key: ValueKey('ctx-${entry.label}'),
          style: _menuRowStyle,
          onPressed: enabled ? () => _onContextAction(entry, command) : null,
          child: _menuLabel(entry, highlighted: highlighted),
        );
        if (entry.helpTooltip != null && entry.helpTooltip!.isNotEmpty) {
          widgets.add(Tooltip(message: entry.helpTooltip!, child: button));
        } else {
          widgets.add(button);
        }
      }
      if (entry.separatorAfter) {
        widgets.add(Divider(height: 1, key: ValueKey('ctx-sep-$i')));
      }
    }
    return widgets;
  }

  /// A menu row with a fixed 32-px height (upstream `MenuItemHeight`), a text
  /// column, an optional right-aligned shortcut column and the submenu arrow
  /// column reserved by [SubmenuButton]. Widths are measured from the actual
  /// labels; long labels/tooltips never force the panel past the window edge
  /// because [MenuStyle] constrains it (see the anchor `style`).
  Widget _menuLabel(ContextMenuEntry entry, {required bool highlighted}) {
    return Container(
      height: _menuItemHeight,
      alignment: Alignment.centerLeft,
      padding: const EdgeInsets.symmetric(horizontal: _menuItemPadding),
      decoration: highlighted
          ? BoxDecoration(
              color: Theme.of(context).colorScheme.primary
                  .withValues(alpha: 0.10),
            )
          : null,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Flexible(
            child: Text(
              entry.label,
              style: const TextStyle(fontSize: 12),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          ),
          if (entry.shortcut != null)
            Padding(
              padding: const EdgeInsets.only(left: 24),
              child: Text(
                entry.shortcut!,
                style: const TextStyle(fontSize: 11, color: Colors.grey),
              ),
            ),
        ],
      ),
    );
  }

  void _onContextAction(ContextMenuEntry entry, CommandContext? command) {
    final profiles = ref.read(profilesControllerProvider.notifier);
    final shell = ref.read(uiShellControllerProvider.notifier);

    // Commands run against the target captured when the menu opened. If the
    // group/view changed or the target row vanished (refresh/delete/filter),
    // close the menu and keep an explainable selection instead of falling back
    // to the first visible row.
    if (_requiresTarget(entry.kind)) {
      final current = ref.read(profilesControllerProvider);
      final restored =
          command != null &&
          command.groupSubId == current.groupSubId &&
          profiles.restoreContextTargets(command.targetIds);
      if (!restored) {
        _closeMenuChain();
        shell.setMessage('操作目标已失效，请重新选择节点');
        return;
      }
    }
    // A captured single-object target (edit/share/activate/export) must still
    // be visible; a filter that hides it refuses the command instead of
    // silently retargeting to the live selection (R3-PROF-02). Commands that
    // do not consume a node primary (group generation, batch, results) are not
    // gated by this.
    final targetId = command?.primaryId;
    if (_usesPrimaryTarget(entry.kind) &&
        targetId != null &&
        !profiles.isVisibleTarget(targetId)) {
      _closeMenuChain();
      shell.setMessage('操作目标已失效，请重新选择节点');
      return;
    }
    _menuController.close();
    _menuFocusIndex = -1;

    switch (entry.kind) {
      case ContextActionKind.selectAll:
        profiles.emitAction(ProfileAction.selectAll);
      case ContextActionKind.moveTop:
        profiles.emitAction(ProfileAction.moveTop);
      case ContextActionKind.moveUp:
        profiles.emitAction(ProfileAction.moveUp);
      case ContextActionKind.moveDown:
        profiles.emitAction(ProfileAction.moveDown);
      case ContextActionKind.moveBottom:
        profiles.emitAction(ProfileAction.moveBottom);
      case ContextActionKind.moveToGroup:
        _moveToGroup(command, entry);
      case ContextActionKind.edit:
        editSelectedProfile(context, ref, targetId: targetId);
      case ContextActionKind.copy:
        copySelectedProfiles(ref);
      case ContextActionKind.delete:
        deleteSelectedProfiles(context, ref);
      case ContextActionKind.removeDuplicate:
        unawaited(_removeDuplicate(entry));
      case ContextActionKind.activate:
        setActiveSelected(ref, targetId: targetId);
      case ContextActionKind.share:
        shareProfileQr(context, ref, targetId: targetId);
      case ContextActionKind.exportClientConfig:
        unawaited(
          exportSelectedClientConfig(
            context,
            ref,
            toClipboard: false,
            targetId: targetId,
          ),
        );
      case ContextActionKind.exportClientConfigClipboard:
        unawaited(
          exportSelectedClientConfig(
            context,
            ref,
            toClipboard: true,
            targetId: targetId,
          ),
        );
      case ContextActionKind.exportShare:
        exportProfiles(context, ref, kind: 'share');
      case ContextActionKind.exportShareBase64:
        exportProfiles(context, ref, kind: 'base64');
      case ContextActionKind.exportInner:
        exportProfiles(context, ref, kind: 'inner');
      case ContextActionKind.tcping:
        profiles.emitAction(ProfileAction.tcping);
      case ContextActionKind.realping:
        profiles.emitAction(ProfileAction.realping);
      case ContextActionKind.speedtest:
        profiles.emitAction(ProfileAction.speedtest);
      case ContextActionKind.udpTest:
        // Disabled in the restricted scope; never faked.
        profiles.emitAction(ProfileAction.udpTest);
      case ContextActionKind.sortResult:
        profiles.sortByResult();
      case ContextActionKind.removeInvalid:
        profiles.emitAction(ProfileAction.removeInvalid);
      case ContextActionKind.genGroupAll:
        _genGroup(command, region: false);
      case ContextActionKind.genGroupRegion:
        _genGroup(command, region: true);
      case ContextActionKind.notImplemented:
        shell.notImplemented(entry.label, entry.actionId);
    }
  }

  /// `移至订阅分组` (ACT-PROF-013): move the captured target ids into the chosen
  /// subscription group.
  ///
  /// The destination comes from the entry's stable [ContextMenuEntry.targetSubId]
  /// (never parsed from the display label), and the ids come from the immutable
  /// [CommandContext] captured when the menu opened (never the live selection).
  void _moveToGroup(CommandContext? command, ContextMenuEntry entry) {
    final profiles = ref.read(profilesControllerProvider.notifier);
    final shell = ref.read(uiShellControllerProvider.notifier);
    if (command == null || command.targetIds.isEmpty) {
      shell.setMessage('请先选择要移动的节点');
      return;
    }
    final subId = entry.targetSubId ?? '';
    final target = subId.isEmpty
        ? null
        : profiles.subItems().where((s) => s.id == subId).firstOrNull;
    final label = target?.remarks.isEmpty ?? true
        ? (target?.id ?? '无分组')
        : target!.remarks;
    final result = profiles.moveProfilesToGroup(
      command.targetIds,
      subId,
      subRemarks: target?.remarks ?? '',
    );
    if (result) {
      shell.setMessage(target == null ? '已移至无分组' : '已移至分组“$label”');
    } else {
      shell.setMessage('移动失败：目标节点不存在或保存被拒绝');
    }
  }

  /// `移除重复` (ACT-PROF-003 / PR-12): deduplicate the current group after an
  /// explicit confirmation, mirroring upstream `RemoveDuplicateServer`
  /// (`ShowYesNoInteraction` → `DedupServerList` → refresh).
  Future<void> _removeDuplicate(ContextMenuEntry entry) async {
    final profiles = ref.read(profilesControllerProvider.notifier);
    final shell = ref.read(uiShellControllerProvider.notifier);
    final confirmed = await showAppConfirmDialog(
      context,
      title: '移除重复',
      message: '确认移除当前分组中的重复节点?',
      confirmLabel: '移除',
      destructive: true,
      dialogKey: const ValueKey('dedup-confirm'),
      confirmKey: const ValueKey('dedup-confirm-ok'),
      cancelKey: const ValueKey('dedup-cancel'),
    );
    if (confirmed != true) {
      shell.setMessage('已取消移除重复');
      return;
    }
    // Read `GuiItem.KeepOlderDedupl` inside the detailed call and distinguish a
    // real delete failure from "no duplicates" (R3-PROF-04).
    final outcome = profiles.removeDuplicateProfilesDetailed();
    if (!outcome.ok) {
      final code = outcome.errorCode ?? 'unknown';
      final detail = outcome.errorMessageKey;
      shell.setMessage(
        detail == null || detail.isEmpty
            ? '移除重复失败：$code'
            : '移除重复失败：$code（$detail）',
      );
      return;
    }
    if (!outcome.hadDuplicates) {
      shell.setMessage('没有重复节点');
      return;
    }
    // If the active duplicate was deleted, re-pick a live node before applying
    // (same fallback as node deletion, RE-PROF-02/RE-PROF-06).
    if (outcome.activeRemoved) {
      await reconcileActiveAfterRemoval(ref);
    }
    shell.setMessage('已移除 ${outcome.removed} 个重复节点');
  }

  /// `一键生成策略组` (ACT-PROF-007/008) for the subscription group selected in
  /// the table at menu-open time.
  ///
  /// Upstream `GenGroupAllServer`/`GenGroupRegionServer` generate for
  /// `SelectedSub`; they do not need a selected node. The captured
  /// `command.groupSubId` is the stable target, so a later group switch or a
  /// leftover hidden selection cannot retarget the command (PR-21). With no
  /// concrete group selected the command fails with an honest message instead
  /// of guessing from a row.
  void _genGroup(CommandContext? command, {required bool region}) {
    final profiles = ref.read(profilesControllerProvider.notifier);
    final shell = ref.read(uiShellControllerProvider.notifier);
    final subId = command?.groupSubId;
    if (subId == null || subId.isEmpty) {
      shell.setMessage('请先在顶部分组中选择一个订阅分组');
      return;
    }
    if (region) {
      final result = profiles.genGroupRegion(subId);
      if (result.ok && result.profiles.isNotEmpty) {
        profiles.selectGenerated(
          result.profiles.map((p) => p.indexId).toList(),
        );
        shell.setMessage('已生成 ${result.profiles.length} 个地区策略组');
      } else {
        shell.setMessage('生成地区策略组失败：该分组无匹配地区节点或被拒绝');
      }
    } else {
      final result = profiles.genGroupAll(subId);
      final created = result.profile;
      if (result.ok && created != null) {
        profiles.selectGenerated(<String>[created.indexId]);
        shell.setMessage('已生成全部配置项策略组');
      } else {
        shell.setMessage('生成策略组失败：订阅分组不存在或保存被拒绝');
      }
    }
  }
}

/// Background fill for the active node row, independent from the multi-select
/// highlight (`Semantics.selectedRow`).
///
/// Upstream `ProfilesView.xaml:271-277` paints every cell of an active row
/// `MaterialDesign.Brush.Primary.Light`; the selection keeps its own container
/// color, so the active marker stays distinguishable in both themes.
@visibleForTesting
Color activeRowFill(BuildContext context) {
  final scheme = Theme.of(context).colorScheme;
  final light = Theme.of(context).brightness == Brightness.light;
  return scheme.primary.withValues(alpha: light ? 0.16 : 0.30);
}

/// Solid leading marker drawn on the active row, visible even when that row is
/// also selected.
@visibleForTesting
Color activeRowMarkerColor(BuildContext context) =>
    Theme.of(context).colorScheme.primary;
