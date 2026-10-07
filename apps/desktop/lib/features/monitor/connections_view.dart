import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/clash_ui_config.dart';
import 'package:v2rayn_desktop/features/monitor/connections_columns.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_incremental.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/shared/widgets/empty_state.dart';

/// Clash connections tab (F-MONITOR-005, LAY-CLASHCN-001).
///
/// Columns follow the persisted `ConnectionsColumnItem` set (Host / Chain /
/// Network / Type / ProcessPath / Elapsed): order and widths are restored on
/// open (upstream `RestoreUI`) and written back on every reorder/autofit
/// (upstream `StorageUI` parity; synchronous so reopen always sees the last
/// arrangement). Headers drag to reorder; rows have
/// the upstream context menu (close / close all) with the target frozen at
/// menu-open time. Filter, close selected and close all go through the Clash
/// controller.
class ConnectionsView extends ConsumerStatefulWidget {
  const ConnectionsView({super.key});

  @override
  ConsumerState<ConnectionsView> createState() => _ConnectionsViewState();
}

class _ConnectionsViewState extends ConsumerState<ConnectionsView> {
  bool _initialized = false;
  bool _autoRefresh = false;
  Timer? _timer;
  Timer? _filterDebounce;
  final TextEditingController _filter = TextEditingController();
  String _needle = '';
  final Set<String> _selected = <String>{};
  List<ConnectionColumn> _columns = defaultConnectionColumns();

  @override
  void initState() {
    super.initState();
    final config = ref.read(clashUiConfigProvider);
    _autoRefresh = config.connectionsAutoRefresh;
    _columns = resolveVisibleColumns(config.connectionsColumns);
    _restartTimer(
      clashPollPeriod(
        autoRefresh: config.connectionsAutoRefresh,
        intervalSeconds: config.connectionsRefreshInterval,
      ),
    );
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted || _initialized) return;
      _initialized = true;
      final controller = ref.read(monitorControllerProvider.notifier);
      controller.setPageVisible('connections', true);
      await controller.refreshConnections();
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    _filterDebounce?.cancel();
    _filter.dispose();
    super.dispose();
  }

  /// Write the arranged columns back as `ConnectionsColumnItem` rows
  /// (Name/Width/Index) for the next reopen. Every mutation (reorder,
  /// autofit) persists synchronously, so an exit-time write-back like
  /// upstream `StorageUI` is unnecessary: in-memory columns can never
  /// diverge from the persisted rows.
  void _persistColumns() {
    final settings = ref.read(settingsControllerProvider);
    if (!settings.loaded) return;
    ref
        .read(settingsControllerProvider.notifier)
        .saveGroup(
          'ClashUIItem',
          clashUiGroupWith(settings.document, <String, Object>{
            'ConnectionsColumnItem': connectionColumnsToStorage(_columns),
          }),
        );
  }

  void _moveColumn(String name, int to) {
    final from = _columns.indexWhere((c) => c.name == name);
    if (from < 0 || from == to) return;
    setState(() => _columns = moveConnectionColumn(_columns, from, to));
    _persistColumns();
  }

  /// Upstream `btnAutofitColumnWidth`: drop the arranged widths and restore
  /// the `ClashConnectionsView.xaml` defaults, then persist them.
  void _autofitColumns() {
    setState(() => _columns = defaultConnectionColumns());
    _persistColumns();
  }

  /// Upstream `DataGrid.ContextMenu` (`menuConnectionClose` /
  /// `menuConnectionCloseAll`). The row id is frozen when the menu opens and
  /// reused after the async close; the selection is never re-read, so a
  /// concurrent refresh cannot redirect the close. Close is disabled for an
  /// empty id (upstream `canEditRemove`); close-all stays on its own path.
  Future<void> _showRowMenu(Offset position, String rowId) async {
    final targetId = rowId;
    final selection = await showMenu<String>(
      context: context,
      position: RelativeRect.fromLTRB(
        position.dx,
        position.dy,
        position.dx,
        position.dy,
      ),
      items: <PopupMenuEntry<String>>[
        PopupMenuItem<String>(
          key: const ValueKey('connections-menu-close'),
          value: 'close',
          enabled: targetId.isNotEmpty,
          child: const Text('关闭连接'),
        ),
        const PopupMenuItem<String>(
          key: ValueKey('connections-menu-close-all'),
          value: 'close-all',
          child: Text('关闭全部'),
        ),
      ],
    );
    if (!mounted || selection == null) return;
    final controller = ref.read(monitorControllerProvider.notifier);
    if (selection == 'close') {
      if (targetId.isEmpty) return;
      final ok = await controller.closeConnection(targetId);
      if (mounted) {
        setState(() => _selected.clear());
        if (!ok) _report('关闭连接失败');
      }
    } else if (selection == 'close-all') {
      final ok = await controller.closeAllConnections();
      if (mounted) {
        setState(() => _selected.clear());
        if (!ok) _report('关闭全部连接失败');
      }
    }
  }

  /// React to a settings change while the tab is open (save -> live refresh).
  /// The timer always follows this canonical config, never a pending toggle.
  void _applyConfig(ClashUiConfig config) {
    if (!mounted) return;
    setState(() => _autoRefresh = config.connectionsAutoRefresh);
    _restartTimer(
      clashPollPeriod(
        autoRefresh: config.connectionsAutoRefresh,
        intervalSeconds: config.connectionsRefreshInterval,
      ),
    );
  }

  void _restartTimer(Duration? period) {
    _timer?.cancel();
    _timer = null;
    if (period == null || period.inSeconds <= 0) return;
    _timer = Timer.periodic(period, (_) {
      if (mounted) {
        unawaited(
          ref.read(monitorControllerProvider.notifier).refreshConnections(),
        );
      }
    });
  }

  /// Wave B (FLD-CFG-134): persist first, move the toggle only on success.
  /// A failed save keeps the old (persisted) toggle and surfaces a visible
  /// error instead of an unpersisted switch position (SP-12 audit contract).
  void _setAutoRefresh(bool value) {
    final settings = ref.read(settingsControllerProvider);
    if (!settings.loaded) {
      _report('连接选项保存失败，已恢复上次保存的值');
      return;
    }
    final result = ref
        .read(settingsControllerProvider.notifier)
        .saveGroup(
          'ClashUIItem',
          clashUiGroupWith(settings.document, <String, Object>{
            'ConnectionsAutoRefresh': value,
          }),
        );
    if (!result.ok) {
      _report('连接选项保存失败，已恢复上次保存的值');
      return;
    }
    _applyConfig(ref.read(clashUiConfigProvider));
  }

  void _report(String message) {
    final messenger = ScaffoldMessenger.maybeOf(context);
    messenger?.showSnackBar(
      SnackBar(
        key: const ValueKey('connections-action-error'),
        content: Text(message, style: const TextStyle(fontSize: 12)),
      ),
    );
  }

  /// SP-22 debounced needle: keystrokes only reschedule the 150ms window,
  /// the synchronous scan runs once per pause instead of once per key.
  void _onFilterChanged(String value) {
    _filterDebounce?.cancel();
    _filterDebounce = Timer(filterDebounceWindow, () {
      if (!mounted) return;
      setState(() => _needle = value);
    });
  }

  @override
  Widget build(BuildContext context) {
    ref.listen(clashUiConfigProvider, (_, next) => _applyConfig(next));
    final state = ref.watch(monitorControllerProvider);
    final controller = ref.read(monitorControllerProvider.notifier);

    if (!state.clashSupported) {
      return EmptyState(
        message: state.connectionsMessage ?? '当前内核不提供 Clash API',
        semanticIcon: 'connections',
        messageKey: const ValueKey('connections-unsupported'),
      );
    }

    final filtered = filterConnections(state.connections, _needle);
    final rows = filtered.items;
    final closeSelectedDisabled = _selected.isEmpty;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Padding(
          padding: const EdgeInsets.all(6),
          child: SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                SizedBox(
                  width: 220,
                  height: 30,
                  child: TextField(
                    key: const ValueKey('connections-filter'),
                    controller: _filter,
                    style: const TextStyle(fontSize: 12),
                    decoration: const InputDecoration(
                      isDense: true,
                      prefixIcon: Icon(Icons.search, size: 14),
                      hintText: '过滤 Host/Chain/进程',
                    ),
                    onChanged: (value) => _onFilterChanged(value),
                  ),
                ),
                const SizedBox(width: 8),
                OutlinedButton(
                  key: const ValueKey('connections-refresh'),
                  onPressed: controller.refreshConnections,
                  child: const Text('刷新', style: TextStyle(fontSize: 12)),
                ),
                const SizedBox(width: 8),
                Row(
                  children: <Widget>[
                    const Text('自动刷新', style: TextStyle(fontSize: 12)),
                    Switch(
                      key: const ValueKey('connections-auto-refresh'),
                      value: _autoRefresh,
                      onChanged: _setAutoRefresh,
                    ),
                  ],
                ),
                OutlinedButton(
                  key: const ValueKey('connections-close-selected'),
                  onPressed: closeSelectedDisabled
                      ? null
                      : () async {
                          var failures = 0;
                          for (final id in _selected.toList()) {
                            final ok = await controller.closeConnection(id);
                            if (!ok) failures++;
                          }
                          if (mounted) {
                            setState(() => _selected.clear());
                            if (failures > 0) {
                              _report('关闭 $failures 条连接失败');
                            }
                          }
                        },
                  child: const Text('关闭选中', style: TextStyle(fontSize: 12)),
                ),
                const SizedBox(width: 6),
                OutlinedButton(
                  key: const ValueKey('connections-close-all'),
                  onPressed: () async {
                    final ok = await controller.closeAllConnections();
                    if (mounted) {
                      setState(() => _selected.clear());
                      if (!ok) _report('关闭全部连接失败');
                    }
                  },
                  child: const Text('关闭全部', style: TextStyle(fontSize: 12)),
                ),
                const SizedBox(width: 6),
                OutlinedButton(
                  key: const ValueKey('connections-autofit'),
                  onPressed: _autofitColumns,
                  child: const Text('列宽自适应', style: TextStyle(fontSize: 12)),
                ),
                const SizedBox(width: 16),
                Text(
                  filtered.truncated
                      ? '显示 ${rows.length} / 命中 ${filtered.totalMatches}（仅显示前 $maxFilteredConnections 条，细化过滤查看更多）'
                      : '↑${state.connectionsUpload} ↓${state.connectionsDownload} · ${filtered.totalMatches} 条',
                  style: const TextStyle(fontSize: 11.5),
                ),
              ],
            ),
          ),
        ),
        const Divider(height: 1),
        Expanded(
          child: rows.isEmpty
              ? const EmptyState(
                  message: '当前无活动连接',
                  semanticIcon: 'connections',
                  detail: '连接产生后自动出现在此表',
                  messageKey: ValueKey('connections-empty'),
                )
              : SingleChildScrollView(
                  scrollDirection: Axis.horizontal,
                  child: SingleChildScrollView(
                    child: DataTable(
                      key: const ValueKey('connections-list'),
                      columns: <DataColumn>[
                        for (final column in _columns)
                          DataColumn(
                            label: DragTarget<String>(
                              onAcceptWithDetails: (details) => _moveColumn(
                                details.data,
                                _columns.indexWhere(
                                  (c) => c.name == column.name,
                                ),
                              ),
                              builder: (context, _, _) => Draggable<String>(
                                data: column.name,
                                feedback: Material(
                                  child: Text(column.name, style: _head),
                                ),
                                child: SizedBox(
                                  width: column.width.toDouble(),
                                  child: Text(
                                    column.name,
                                    style: _head,
                                    overflow: TextOverflow.ellipsis,
                                  ),
                                ),
                              ),
                            ),
                          ),
                        const DataColumn(label: Text('', style: _head)),
                      ],
                      rows: <DataRow>[
                        for (final c in rows)
                          DataRow(
                            key: ValueKey('connections-row-${c.id}'),
                            selected: _selected.contains(c.id),
                            onSelectChanged: (value) {
                              setState(() {
                                if (value ?? false) {
                                  _selected.add(c.id);
                                } else {
                                  _selected.remove(c.id);
                                }
                              });
                            },
                            cells: <DataCell>[
                              for (final column in _columns)
                                DataCell(
                                  GestureDetector(
                                    onSecondaryTapUp: (details) => _showRowMenu(
                                      details.globalPosition,
                                      c.id,
                                    ),
                                    child: SizedBox(
                                      width: column.width.toDouble(),
                                      child: Text(
                                        _valueFor(c, column.name) ?? '-',
                                        style: const TextStyle(fontSize: 11.5),
                                        overflow: TextOverflow.ellipsis,
                                      ),
                                    ),
                                  ),
                                ),
                              DataCell(
                                IconButton(
                                  key: ValueKey('connections-close-${c.id}'),
                                  iconSize: 14,
                                  tooltip: '关闭连接',
                                  onPressed: () =>
                                      controller.closeConnection(c.id),
                                  icon: const Icon(Icons.close),
                                ),
                              ),
                            ],
                          ),
                      ],
                    ),
                  ),
                ),
        ),
      ],
    );
  }
}

const _head = TextStyle(fontSize: 11.5, fontWeight: FontWeight.w600);

/// Cell text for a persisted column key (`ExName` parity, never localized).
String? _valueFor(m.ClashConnectionDto c, String name) {
  switch (name) {
    case 'Host':
      return c.host;
    case 'Chain':
      return c.chains.join(' -> ');
    case 'Network':
      return c.network;
    case 'Type':
      return c.connectionType;
    case 'ProcessPath':
      return c.processPath;
    case 'Elapsed':
      return _elapsed(c.start);
    default:
      return null;
  }
}

String _elapsed(String? start) {
  if (start == null || start.isEmpty) return '-';
  final parsed = DateTime.tryParse(start);
  if (parsed == null) return '-';
  final seconds = DateTime.now().toUtc().difference(parsed.toUtc()).inSeconds;
  if (seconds < 0) return '-';
  if (seconds < 60) return '${seconds}s';
  if (seconds < 3600) return '${seconds ~/ 60}m${seconds % 60}s';
  return '${seconds ~/ 3600}h${(seconds % 3600) ~/ 60}m';
}
