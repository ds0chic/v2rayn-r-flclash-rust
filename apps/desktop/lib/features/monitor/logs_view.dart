import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';

/// Message/log tab (F-MONITOR-001, LAY-MSG-001).
///
/// Shows the real core output forwarded by net-host. Level/keyword filters,
/// auto-refresh (scroll) and collect-pause are honoured; overflow is surfaced
/// (FND-004: the upstream ClearMsg handler is commented out, so the clear
/// control is disabled with an explanation instead of faking a clear).
class LogsView extends ConsumerStatefulWidget {
  const LogsView({super.key});

  @override
  ConsumerState<LogsView> createState() => _LogsViewState();
}

class _LogsViewState extends ConsumerState<LogsView> {
  final ScrollController _scroll = ScrollController();
  final TextEditingController _keyword = TextEditingController();
  bool _initialized = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || _initialized) return;
      _initialized = true;
      ref.read(monitorControllerProvider.notifier).setPageVisible('logs', true);
    });
  }

  @override
  void dispose() {
    _scroll.dispose();
    _keyword.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(monitorControllerProvider);
    final controller = ref.read(monitorControllerProvider.notifier);
    final visible = state.visibleLogs;

    if (!state.scrollPaused) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted || !_scroll.hasClients) return;
        _scroll.jumpTo(_scroll.position.maxScrollExtent);
      });
    }

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
                  width: 200,
                  height: 30,
                  child: TextField(
                    key: const ValueKey('logs-keyword'),
                    controller: _keyword,
                    style: const TextStyle(fontSize: 12),
                    decoration: const InputDecoration(
                      isDense: true,
                      prefixIcon: Icon(Icons.search, size: 14),
                      hintText: '过滤',
                    ),
                    onChanged: controller.setKeyword,
                  ),
                ),
                const SizedBox(width: 8),
                DropdownButton<int>(
                  key: const ValueKey('logs-level'),
                  value: state.minLevel,
                  style: const TextStyle(fontSize: 12),
                  onChanged: (value) => controller.setMinLevel(value ?? 0),
                  items: const <DropdownMenuItem<int>>[
                    DropdownMenuItem(value: 0, child: Text('Trace+')),
                    DropdownMenuItem(value: 1, child: Text('Debug+')),
                    DropdownMenuItem(value: 2, child: Text('Info+')),
                    DropdownMenuItem(value: 3, child: Text('Warn+')),
                    DropdownMenuItem(value: 4, child: Text('Error+')),
                    DropdownMenuItem(value: 5, child: Text('Fatal')),
                  ],
                ),
                const SizedBox(width: 8),
                // AutoRefresh: while on, the view follows new lines.
                Row(
                  children: <Widget>[
                    const Text('自动刷新', style: TextStyle(fontSize: 12)),
                    Switch(
                      key: const ValueKey('logs-autorefresh'),
                      value: !state.scrollPaused,
                      onChanged: (value) => controller.setScrollPaused(!value),
                    ),
                  ],
                ),
                OutlinedButton(
                  key: const ValueKey('logs-pause-collect'),
                  onPressed: () =>
                      controller.setCollectingPaused(!state.collectingPaused),
                  child: Text(
                    state.collectingPaused ? '继续采集' : '暂停采集',
                    style: const TextStyle(fontSize: 12),
                  ),
                ),
                const SizedBox(width: 6),
                // Upstream MsgViewModel.ClearMsg is commented out; a real clear
                // could silently diverge from the reference, so it stays disabled.
                Tooltip(
                  message: '上游 ClearMsg 处理已注释（FND-004），不提供清空',
                  child: TextButton(
                    key: const ValueKey('logs-clear'),
                    onPressed: null,
                    child: const Text('清空', style: TextStyle(fontSize: 12)),
                  ),
                ),
                const SizedBox(width: 16),
                if (state.droppedLines > BigInt.zero ||
                    state.truncatedLines > BigInt.zero)
                  Text(
                    '溢出 ${state.droppedLines} 行 / 截断 ${state.truncatedLines} 行',
                    key: const ValueKey('logs-overflow'),
                    style: TextStyle(
                      fontSize: 11.5,
                      color: Theme.of(context).colorScheme.error,
                    ),
                  ),
              ],
            ),
          ),
        ),
        const Divider(height: 1),
        Expanded(
          child: visible.isEmpty
              ? const Center(
                  child: Text(
                    '信息：暂无内核日志',
                    key: ValueKey('logs-empty'),
                    style: TextStyle(fontSize: 12),
                  ),
                )
              : ListView.builder(
                  key: const ValueKey('logs-list'),
                  controller: _scroll,
                  itemCount: visible.length,
                  itemBuilder: (context, index) =>
                      _LogRow(line: visible[index]),
                ),
        ),
      ],
    );
  }
}

class _LogRow extends StatelessWidget {
  const _LogRow({required this.line});

  final m.LogLineDto line;

  @override
  Widget build(BuildContext context) {
    final color = _levelColor(context, line.level);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 1),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          SizedBox(
            width: 48,
            child: Text(
              _levelLabel(line.level),
              style: TextStyle(fontSize: 11, color: color),
            ),
          ),
          Expanded(
            child: Text(
              line.text,
              style: const TextStyle(fontSize: 12, fontFamily: 'monospace'),
            ),
          ),
          if (line.truncated)
            const Text(
              '[截断]',
              style: TextStyle(fontSize: 11, color: Colors.orange),
            ),
        ],
      ),
    );
  }
}

String _levelLabel(int level) => switch (level) {
  0 => 'TRACE',
  1 => 'DEBUG',
  2 => 'INFO',
  3 => 'WARN',
  4 => 'ERROR',
  5 => 'FATAL',
  _ => '----',
};

Color _levelColor(BuildContext context, int level) => switch (level) {
  0 || 1 => Colors.grey,
  2 => Theme.of(context).colorScheme.primary,
  3 => Colors.orange,
  4 || 5 => Theme.of(context).colorScheme.error,
  _ => Colors.grey,
};
