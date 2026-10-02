import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';
import 'package:v2rayn_desktop/shared/widgets/empty_state.dart';

/// Clash proxies tab (F-MONITOR-004, LAY-CLASHPROXY-001).
///
/// Renders the controller's proxy groups and nodes. Delay probes and group
/// selection go through the bridge; a non-mihomo/sing-box core reports
/// "当前内核不提供 Clash API" instead of an empty fake list.
class ProxiesView extends ConsumerStatefulWidget {
  const ProxiesView({super.key});

  @override
  ConsumerState<ProxiesView> createState() => _ProxiesViewState();
}

class _ProxiesViewState extends ConsumerState<ProxiesView> {
  bool _initialized = false;
  bool _autoRefresh = false;
  bool _sortDescending = false;
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted || _initialized) return;
      _initialized = true;
      final controller = ref.read(monitorControllerProvider.notifier);
      controller.setPageVisible('proxies', true);
      await controller.refreshProxies();
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  void _setAutoRefresh(bool value) {
    setState(() => _autoRefresh = value);
    _timer?.cancel();
    if (value) {
      _timer = Timer.periodic(const Duration(seconds: 2), (_) async {
        if (mounted) {
          await ref.read(monitorControllerProvider.notifier).refreshProxies();
        }
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(monitorControllerProvider);
    final controller = ref.read(monitorControllerProvider.notifier);

    if (!state.clashSupported) {
      return EmptyState(
        message: state.proxiesMessage ?? '当前内核不提供 Clash API',
        semanticIcon: 'proxies',
        messageKey: const ValueKey('proxies-unsupported'),
      );
    }

    final groups = state.proxies.where((p) => p.isGroup).toList()
      ..sort(
        (a, b) => _sortDescending
            ? b.name.compareTo(a.name)
            : a.name.compareTo(b.name),
      );
    final nodes = state.proxies.where((p) => !p.isGroup).toList()
      ..sort(
        (a, b) => _sortDescending
            ? b.name.compareTo(a.name)
            : a.name.compareTo(b.name),
      );

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
                OutlinedButton(
                  key: const ValueKey('proxies-refresh'),
                  onPressed: controller.refreshProxies,
                  child: const Text('刷新', style: TextStyle(fontSize: 12)),
                ),
                const SizedBox(width: 8),
                Row(
                  children: <Widget>[
                    const Text('自动刷新', style: TextStyle(fontSize: 12)),
                    Switch(
                      key: const ValueKey('proxies-auto-refresh'),
                      value: _autoRefresh,
                      onChanged: _setAutoRefresh,
                    ),
                  ],
                ),
                TextButton.icon(
                  key: const ValueKey('proxies-sort'),
                  onPressed: () =>
                      setState(() => _sortDescending = !_sortDescending),
                  icon: Icon(
                    _sortDescending ? Icons.sort_by_alpha : Icons.sort,
                    size: 14,
                  ),
                  label: Text(
                    _sortDescending ? '名称 Z-A' : '名称 A-Z',
                    style: const TextStyle(fontSize: 12),
                  ),
                ),
                const SizedBox(width: 16),
                Text(
                  '组 ${groups.length} / 节点 ${nodes.length}',
                  style: const TextStyle(fontSize: 11.5),
                ),
              ],
            ),
          ),
        ),
        const Divider(height: 1),
        Expanded(
          child: state.proxies.isEmpty
              ? const EmptyState(
                  message: '暂无代理数据',
                  semanticIcon: 'proxies',
                  detail: '内核提供 Clash API 后自动加载',
                  messageKey: ValueKey('proxies-empty'),
                )
              : ListView(
                  key: const ValueKey('proxies-list'),
                  padding: const EdgeInsets.all(6),
                  children: <Widget>[
                    for (final group in groups)
                      _GroupCard(
                        group: group,
                        delays: state.proxyDelays,
                        onTestGroup: () => controller.testGroup(group.name),
                        onTestNode: controller.testProxy,
                        onSelect: (name) =>
                            controller.selectProxy(group.name, name),
                      ),
                    if (nodes.isNotEmpty) ...<Widget>[
                      const Padding(
                        padding: EdgeInsets.only(top: 8, bottom: 4),
                        child: Text('节点', style: TextStyle(fontSize: 12)),
                      ),
                      for (final node in nodes)
                        _NodeRow(
                          node: node,
                          delay: state.proxyDelays[node.name] ?? node.delay,
                          onTest: () => controller.testProxy(node.name),
                        ),
                    ],
                  ],
                ),
        ),
      ],
    );
  }
}

class _GroupCard extends StatelessWidget {
  const _GroupCard({
    required this.group,
    required this.delays,
    required this.onTestGroup,
    required this.onTestNode,
    required this.onSelect,
  });

  final m.ClashProxyDto group;
  final Map<String, int> delays;
  final VoidCallback onTestGroup;
  final Future<int> Function(String name) onTestNode;
  final Future<bool> Function(String name) onSelect;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Card(
      key: ValueKey('proxies-group-${group.name}'),
      margin: const EdgeInsets.symmetric(vertical: 4),
      child: Padding(
        padding: const EdgeInsets.all(8),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              children: <Widget>[
                Expanded(
                  child: Text(
                    '${group.name}  [${group.proxyType}]',
                    style: const TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ),
                Text(
                  '当前: ${group.now ?? '--'}',
                  style: TextStyle(fontSize: 12, color: scheme.primary),
                ),
                const SizedBox(width: 8),
                OutlinedButton(
                  key: ValueKey('proxies-group-delay-${group.name}'),
                  onPressed: onTestGroup,
                  child: const Text('测试延迟', style: TextStyle(fontSize: 11)),
                ),
              ],
            ),
            for (final child in group.all)
              ListTile(
                key: ValueKey('proxies-select-${group.name}-$child'),
                dense: true,
                visualDensity: VisualDensity.compact,
                selected: group.now == child,
                onTap: () => onSelect(child),
                leading: Icon(
                  group.now == child
                      ? Icons.radio_button_checked
                      : Icons.radio_button_unchecked,
                  size: 16,
                ),
                title: Text(child, style: const TextStyle(fontSize: 12)),
                trailing: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    Text(
                      formatDelayMs(delays[child] ?? -1),
                      key: ValueKey('proxies-child-delay-$child'),
                      style: const TextStyle(fontSize: 11),
                    ),
                    IconButton(
                      key: ValueKey('proxies-child-test-$child'),
                      iconSize: 14,
                      tooltip: '测试节点延迟',
                      onPressed: () => onTestNode(child),
                      icon: const Icon(Icons.speed),
                    ),
                  ],
                ),
              ),
          ],
        ),
      ),
    );
  }
}

class _NodeRow extends StatelessWidget {
  const _NodeRow({
    required this.node,
    required this.delay,
    required this.onTest,
  });

  final m.ClashProxyDto node;
  final int delay;
  final VoidCallback onTest;

  @override
  Widget build(BuildContext context) {
    return ListTile(
      key: ValueKey('proxies-node-${node.name}'),
      dense: true,
      title: Text(
        '${node.name}  [${node.proxyType}]',
        style: const TextStyle(fontSize: 12),
      ),
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Text(formatDelayMs(delay), style: const TextStyle(fontSize: 11)),
          IconButton(
            key: ValueKey('proxies-node-delay-${node.name}'),
            iconSize: 14,
            tooltip: '测试节点延迟',
            onPressed: onTest,
            icon: const Icon(Icons.speed),
          ),
        ],
      ),
    );
  }
}

/// `-1` means timeout/unreachable (upstream convention).
String formatDelayMs(int delay) => delay < 0 ? '--' : '${delay}ms';
