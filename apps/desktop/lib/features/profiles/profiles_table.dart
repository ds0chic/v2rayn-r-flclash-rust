import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:two_dimensional_scrollables/two_dimensional_scrollables.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/context_menu.dart';
import 'package:v2rayn_desktop/features/profiles/profile_actions.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';
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

class _ProfilesTableState extends ConsumerState<ProfilesTable> {
  static const _headerHeight = AppTokens.tableHeaderHeight;

  /// Row height follows the configured base font size (24/26/28) exposed on
  /// the theme extension, keeping the desktop table compact.
  double get _rowHeight => context.semantics.tableRowHeight;

  final FocusNode _focusNode = FocusNode(debugLabel: 'profiles-table');
  final MenuController _menuController = MenuController();
  late final ScrollController _vertical =
      widget.verticalController ?? ScrollController();
  late final ScrollController _horizontal =
      widget.horizontalController ?? ScrollController();
  bool _ownsVertical = false;
  bool _ownsHorizontal = false;

  @override
  void initState() {
    super.initState();
    _ownsVertical = widget.verticalController == null;
    _ownsHorizontal = widget.horizontalController == null;
  }

  @override
  void dispose() {
    _focusNode.dispose();
    if (_ownsVertical) _vertical.dispose();
    if (_ownsHorizontal) _horizontal.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(profilesControllerProvider);
    final columns = state.visibleColumns;
    final rows = state.visible;

    return MenuAnchor(
      controller: _menuController,
      menuChildren: _buildContextMenu(context, profilesContextMenu),
      child: Focus(
        focusNode: _focusNode,
        autofocus: true,
        onKeyEvent: _onKey,
        child: Listener(
          onPointerDown: (_) => _focusNode.requestFocus(),
          child: Stack(
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
                    _buildColumnSpan(context, index, columns),
                rowBuilder: (index) => _buildRowSpan(index, context),
                cellBuilder: (context, vicinity) => TableViewCell(
                  child: _buildCell(context, vicinity, state, columns, rows),
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
          ),
        ),
      ),
    );
  }

  TableSpan _buildColumnSpan(
    BuildContext context,
    int index,
    List<ProfileColumn> columns,
  ) {
    final width = index == 0
        ? AppTokens.tableHandleWidth
        : columns[index - 1].width;
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
  ) {
    if (vicinity.row == 0) {
      return _headerCell(context, vicinity.column, state, columns);
    }
    final dataIndex = vicinity.row - 1;
    if (dataIndex >= rows.length) return const SizedBox.shrink();
    final row = rows[dataIndex];
    if (vicinity.column == 0) {
      return _handleCell(context, row, dataIndex, state);
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
        case ProfileAction.copy:
          controller.logAction(action!, 'keyboard');
          copySelectedProfiles(ref);
          return KeyEventResult.handled;
        case ProfileAction.activate:
          controller.logAction(action!, 'keyboard');
          toggleActiveSelected(ref);
          return KeyEventResult.handled;
      }
    }
    return ref.read(profilesControllerProvider.notifier).handleKeyEvent(event)
        ? KeyEventResult.handled
        : KeyEventResult.ignored;
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
      toggleActiveSelected(ref);
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
      onSecondaryTapDown: (details) =>
          _showContextMenu(details.globalPosition, row),
      child: Container(
        color: selected ? context.semantics.selectedRow : null,
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
  ) {
    final selected = state.selected.contains(row.id);
    final controller = ref.read(profilesControllerProvider.notifier);
    final content = Container(
      color: selected ? Theme.of(context).colorScheme.primaryContainer : null,
      alignment: Alignment.center,
      child: Draggable<ProfileSummary>(
        data: row,
        dragAnchorStrategy: pointerDragAnchorStrategy,
        onDragStarted: () {
          _menuController.close();
          controller.handleDragStart(row.id);
        },
        feedback: Material(
          elevation: 4,
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
            color: Theme.of(context).colorScheme.secondaryContainer,
            child: Text(row.remarks),
          ),
        ),
        childWhenDragging: Opacity(
          opacity: 0.3,
          child: Text('${index + 1}', style: const TextStyle(fontSize: 11)),
        ),
        child: Text('${index + 1}', style: const TextStyle(fontSize: 11)),
      ),
    );
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
        child: content,
      ),
    );
  }

  void _showContextMenu(Offset globalPosition, ProfileSummary row) {
    ref.read(profilesControllerProvider.notifier).handleRightTap(row.id);
    _menuController.open(position: globalPosition);
  }

  List<Widget> _buildContextMenu(
    BuildContext context,
    List<ContextMenuEntry> entries,
  ) {
    final widgets = <Widget>[];
    for (final entry in entries) {
      if (entry.isSubmenu) {
        widgets.add(
          SubmenuButton(
            key: ValueKey('ctx-${entry.label}'),
            menuChildren: _buildContextMenu(context, entry.submenu),
            child: _menuLabel(entry),
          ),
        );
      } else {
        widgets.add(
          MenuItemButton(
            key: ValueKey('ctx-${entry.label}'),
            onPressed: entry.enabled ? () => _onContextAction(entry) : null,
            child: _menuLabel(entry),
          ),
        );
      }
      if (entry.separatorAfter) {
        widgets.add(const Divider(height: 1));
      }
    }
    return widgets;
  }

  Widget _menuLabel(ContextMenuEntry entry) {
    return Row(
      children: <Widget>[
        Expanded(
          child: Text(entry.label, style: const TextStyle(fontSize: 12)),
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
    );
  }

  void _onContextAction(ContextMenuEntry entry) {
    final profiles = ref.read(profilesControllerProvider.notifier);
    final shell = ref.read(uiShellControllerProvider.notifier);
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
      case ContextActionKind.edit:
        editSelectedProfile(context, ref);
      case ContextActionKind.copy:
        copySelectedProfiles(ref);
      case ContextActionKind.delete:
        deleteSelectedProfiles(context, ref);
      case ContextActionKind.activate:
        toggleActiveSelected(ref);
      case ContextActionKind.remarks:
        renameSelectedProfile(context, ref);
      case ContextActionKind.tcping:
        profiles.emitAction(ProfileAction.tcping);
      case ContextActionKind.realping:
        profiles.emitAction(ProfileAction.realping);
      case ContextActionKind.speedtest:
        profiles.emitAction(ProfileAction.speedtest);
      case ContextActionKind.mixedTest:
        profiles.emitAction(ProfileAction.mixedTest);
      case ContextActionKind.fastRealping:
        profiles.emitAction(ProfileAction.fastRealping);
      case ContextActionKind.udpTest:
        // Disabled in the restricted scope; never faked.
        profiles.emitAction(ProfileAction.udpTest);
      case ContextActionKind.removeInvalid:
        profiles.emitAction(ProfileAction.removeInvalid);
      case ContextActionKind.notImplemented:
        shell.notImplemented(entry.label, entry.actionId);
    }
  }
}
