import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/log_ui_config.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_format.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_incremental.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';
import 'package:v2rayn_desktop/shared/widgets/empty_state.dart';

/// Message/log tab (F-MONITOR-001, LAY-MSG-001).
///
/// Shows the real core output forwarded by net-host. Collection (Rust ring),
/// presentation (auto-refresh + level/keyword filter), scrolling and clipboard
/// copy are kept separate, matching upstream `MsgViewModel`/`MsgView`. Hiding
/// the tab freezes only the page-local view; the Rust ring keeps collecting.
class LogsView extends ConsumerStatefulWidget {
  const LogsView({super.key});

  @override
  ConsumerState<LogsView> createState() => _LogsViewState();
}

class _LogsViewState extends ConsumerState<LogsView> {
  final ScrollController _scroll = ScrollController();
  final TextEditingController _keyword = TextEditingController();
  Timer? _keywordDebounce;
  late final MonitorController _controller;
  bool _initialized = false;

  @override
  void initState() {
    super.initState();
    // Captured once: `ref` is unsafe in dispose(), the notifier is not.
    _controller = ref.read(monitorControllerProvider.notifier);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || _initialized) return;
      _initialized = true;
      _controller.setPageVisible('logs', true);
      // Wave B G-12: seed the filter/refresh presentation state from the
      // canonical `MsgUIItem` group on every open (save -> reopen keeps the
      // initial filter; null AutoRefresh falls back to live tailing).
      _applyConfig(ref.read(logUiConfigProvider));
    });
  }

  @override
  void dispose() {
    _controller.setPageVisible('logs', false);
    _keywordDebounce?.cancel();
    _scroll.dispose();
    _keyword.dispose();
    super.dispose();
  }

  /// SP-22: keyword scans the retained tail synchronously, so keystrokes are
  /// debounced into one scan per pause instead of one full filter per key.
  /// Wave B G-12: the debounced value is committed through the canonical
  /// `MsgUIItem` group (persist-first), so save -> reopen keeps the filter.
  void _onKeywordChanged(String value) {
    _keywordDebounce?.cancel();
    _keywordDebounce = Timer(filterDebounceWindow, () {
      if (!mounted) return;
      _commitKeyword(value);
    });
  }

  /// Apply the canonical config to the page-local view. Pause flags, the
  /// retained tail and the live stream are untouched (controller side), so a
  /// paused view stays paused across a filter change and a live view keeps
  /// tailing.
  void _applyConfig(LogUiConfig config) {
    if (!mounted) return;
    if (_keyword.text != config.keyword) {
      _keyword.value = TextEditingValue(
        text: config.keyword,
        selection: TextSelection.collapsed(offset: config.keyword.length),
      );
    }
    _controller.seedLogView(
      keyword: config.keyword,
      autoRefresh: config.autoRefresh,
    );
  }

  /// Persist [changes] into `MsgUIItem`; true only when the write landed.
  bool _persistMsgUi(Map<String, Object?> changes) {
    final settings = ref.read(settingsControllerProvider);
    if (!settings.loaded) return false;
    final result = ref
        .read(settingsControllerProvider.notifier)
        .saveGroup('MsgUIItem', msgUiGroupWith(settings.document, changes));
    return result.ok;
  }

  void _showNotice(String message, String key) {
    final messenger = ScaffoldMessenger.maybeOf(context);
    messenger?.showSnackBar(
      SnackBar(
        key: ValueKey(key),
        content: Text(message, style: const TextStyle(fontSize: 12)),
      ),
    );
  }

  void _revertKeyword(String previous) {
    _keyword.value = TextEditingValue(
      text: previous,
      selection: TextSelection.collapsed(offset: previous.length),
    );
  }

  /// Commit a debounced keyword: invalid regex is rejected with the previous
  /// filter kept (FLD-CFG-065); a failed save rolls back the text and keeps
  /// the persisted filter (FLD-CFG-066 failure path).
  void _commitKeyword(String value) {
    if (!mounted) return;
    final previous = ref.read(monitorControllerProvider).keyword;
    if (value == previous) return;
    if (!isValidLogFilter(value)) {
      _revertKeyword(previous);
      _showNotice('过滤表达式非法，已保留上次过滤', 'logs-filter-error');
      return;
    }
    if (!_persistMsgUi(<String, Object?>{'MainMsgFilter': value})) {
      _revertKeyword(previous);
      _showNotice('日志选项保存失败，已恢复上次保存的值', 'logs-save-error');
      return;
    }
    _applyConfig(ref.read(logUiConfigProvider));
  }

  /// Wave B G-12: persist-first auto-refresh toggle. A failed save keeps the
  /// old (persisted) switch and shows a visible error; the view never runs on
  /// an unpersisted value.
  void _setAutoRefresh(bool value) {
    if (value == ref.read(monitorControllerProvider).autoRefresh) return;
    if (!_persistMsgUi(<String, Object?>{'AutoRefresh': value})) {
      _showNotice('日志选项保存失败，已恢复上次保存的值', 'logs-save-error');
      return;
    }
    _applyConfig(ref.read(logUiConfigProvider));
  }

  @override
  Widget build(BuildContext context) {
    // React to a canonical change while the tab is open (save -> live
    // refresh). Seeding only touches filter/refresh, never pause flags.
    ref.listen(logUiConfigProvider, (_, next) => _applyConfig(next));
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
                    onChanged: _onKeywordChanged,
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
                // Presentation refresh: when off the view freezes but the Rust
                // ring keeps collecting; resuming resyncs the tail.
                Row(
                  children: <Widget>[
                    const Text('自动刷新', style: TextStyle(fontSize: 12)),
                    Switch(
                      key: const ValueKey('logs-autorefresh'),
                      value: state.autoRefresh,
                      onChanged: _setAutoRefresh,
                    ),
                  ],
                ),
                // Scroll-to-end, independent of refresh/collection.
                Row(
                  children: <Widget>[
                    const Text('自动滚动', style: TextStyle(fontSize: 12)),
                    Switch(
                      key: const ValueKey('logs-autoscroll'),
                      value: !state.scrollPaused,
                      onChanged: (value) => controller.setScrollPaused(!value),
                    ),
                  ],
                ),
                // Collection pause: real lines stop entering the Rust ring.
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
                OutlinedButton.icon(
                  key: const ValueKey('logs-copy-all'),
                  onPressed: visible.isEmpty
                      ? null
                      : () => Clipboard.setData(
                          ClipboardData(text: formatLogsForCopy(visible)),
                        ),
                  icon: const Icon(Icons.content_copy, size: 14),
                  label: const Text('复制全部', style: TextStyle(fontSize: 12)),
                ),
                const SizedBox(width: 6),
                // Upstream MsgView.ClearMsg clears the box and inserts a marker.
                OutlinedButton(
                  key: const ValueKey('logs-clear'),
                  onPressed: controller.clearLogs,
                  child: const Text('清空', style: TextStyle(fontSize: 12)),
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
              ? const EmptyState(
                  message: '暂无内核日志',
                  semanticIcon: 'logs',
                  detail: '内核运行时将在此显示真实输出',
                  messageKey: ValueKey('logs-empty'),
                )
              : SelectionArea(
                  key: const ValueKey('logs-selection'),
                  child: ListView.builder(
                    key: const ValueKey('logs-list'),
                    controller: _scroll,
                    itemCount: visible.length,
                    itemBuilder: (context, index) =>
                        _LogRow(line: visible[index]),
                  ),
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
              style: const TextStyle(
                fontSize: AppTokens.monoFontSize,
                fontFamily: AppTokens.monoFontFamily,
              ),
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

Color _levelColor(BuildContext context, int level) {
  final scheme = Theme.of(context).colorScheme;
  final semantics = context.semantics;
  return switch (level) {
    0 || 1 => scheme.onSurfaceVariant,
    2 => semantics.info,
    3 => semantics.warning,
    4 || 5 => scheme.error,
    _ => scheme.onSurfaceVariant,
  };
}
