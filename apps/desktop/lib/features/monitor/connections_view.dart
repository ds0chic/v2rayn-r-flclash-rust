import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';
import 'package:v2rayn_desktop/shared/widgets/empty_state.dart';

/// Clash connections tab (F-MONITOR-005, LAY-CLASHCN-001).
///
/// Columns follow the persisted `ConnectionsColumnItem` default set:
/// Host / Chain / Network / Type / ProcessPath / Elapsed. Filter, close
/// selected and close all go through the Clash controller.
class ConnectionsView extends ConsumerStatefulWidget {
  const ConnectionsView({super.key});

  @override
  ConsumerState<ConnectionsView> createState() => _ConnectionsViewState();
}

class _ConnectionsViewState extends ConsumerState<ConnectionsView> {
  bool _initialized = false;
  bool _autoRefresh = false;
  Timer? _timer;
  final TextEditingController _filter = TextEditingController();
  String _needle = '';
  final Set<String> _selected = <String>{};

  @override
  void initState() {
    super.initState();
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
    _filter.dispose();
    super.dispose();
  }

  void _setAutoRefresh(bool value) {
    setState(() => _autoRefresh = value);
    _timer?.cancel();
    if (value) {
      _timer = Timer.periodic(const Duration(seconds: 2), (_) async {
        if (mounted) {
          await ref
              .read(monitorControllerProvider.notifier)
              .refreshConnections();
        }
      });
    }
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

  List<m.ClashConnectionDto> _filtered(List<m.ClashConnectionDto> items) {
    final needle = _needle.trim().toLowerCase();
    if (needle.isEmpty) return items;
    return items.where((c) {
      final haystack = <String?>[
        c.host,
        c.connectionType,
        c.network,
        c.processPath,
        c.rule,
        c.chains.join(' '),
      ].whereType<String>().join(' ').toLowerCase();
      return haystack.contains(needle);
    }).toList();
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(monitorControllerProvider);
    final controller = ref.read(monitorControllerProvider.notifier);

    if (!state.clashSupported) {
      return EmptyState(
        message: state.connectionsMessage ?? '当前内核不提供 Clash API',
        semanticIcon: 'connections',
        messageKey: const ValueKey('connections-unsupported'),
      );
    }

    final rows = _filtered(state.connections);
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
                    onChanged: (value) => setState(() => _needle = value),
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
                const SizedBox(width: 16),
                Text(
                  '↑${state.connectionsUpload} ↓${state.connectionsDownload}',
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
                      columns: const <DataColumn>[
                        DataColumn(label: Text('Host', style: _head)),
                        DataColumn(label: Text('Chain', style: _head)),
                        DataColumn(label: Text('Network', style: _head)),
                        DataColumn(label: Text('Type', style: _head)),
                        DataColumn(label: Text('ProcessPath', style: _head)),
                        DataColumn(label: Text('Elapsed', style: _head)),
                        DataColumn(label: Text('', style: _head)),
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
                              DataCell(_text(c.host)),
                              DataCell(_text(c.chains.join(' -> '))),
                              DataCell(_text(c.network)),
                              DataCell(_text(c.connectionType)),
                              DataCell(_text(c.processPath)),
                              DataCell(_text(_elapsed(c.start))),
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

Widget _text(String? value) =>
    Text(value ?? '-', style: const TextStyle(fontSize: 11.5));

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
