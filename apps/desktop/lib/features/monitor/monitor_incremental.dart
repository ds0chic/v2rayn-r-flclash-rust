import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;

/// SP-22 增量可合并覆盖层：日志/测速/连接更新按 ID 增量合并，筛选有界。
///
/// 背景（CP-15）：测速同时浏览日志/连接并筛选节点时，界面曾被“每事件一次
/// 全量 overlay + 同步全表扫描”阻塞。本文件只放纯函数：调用方（controller/
/// views）负责 150ms 合并窗口与防抖，这里保证单次合并有界、不 mutated 输入。
///
/// 增量语义：已存在行原位更新、新增行追加、消失行在快照合并中丢弃；
/// 测速延迟按节点 ID 覆盖；日志只保留有界尾部，最终结果（尾部）不被挤掉。

/// 日志合并窗口：高频批次在此窗口内合并为一次行刷新；计数器/暂停标记
/// 不经过此窗口，立即生效（生命周期与最终结果不被高频日志挤掉）。
const Duration logCoalesceWindow = Duration(milliseconds: 150);

/// 筛选输入防抖：避免每键一次同步全表扫描。
const Duration filterDebounceWindow = Duration(milliseconds: 150);

/// 连接筛选/渲染的单次上限：10k 连接下界面仍响应，超量截断并明示。
const int maxFilteredConnections = 300;

/// 日志尾部合并：追加 [incoming] 并只保留最后 [max] 行，不 mutated [existing]。
List<m.LogLineDto> mergeLogTail(
  List<m.LogLineDto> existing,
  List<m.LogLineDto> incoming,
  int max,
) {
  if (incoming.isEmpty) return List<m.LogLineDto>.of(existing);
  final total = existing.length + incoming.length;
  if (total <= max) return <m.LogLineDto>[...existing, ...incoming];
  final merged = <m.LogLineDto>[...existing, ...incoming];
  return merged.sublist(total - max);
}

/// 测速延迟按 ID 覆盖合并，返回新 map，不 mutated [existing]。
Map<String, int> mergeDelayMap(
  Map<String, int> existing,
  List<m.DelayResultDto> items,
) {
  if (items.isEmpty) return Map<String, int>.of(existing);
  final merged = Map<String, int>.of(existing);
  for (final item in items) {
    merged[item.name] = item.delay;
  }
  return merged;
}

/// 连接快照按 ID 合并：存活行保持原顺序并原位更新，新行追加，
/// 快照中消失的行丢弃（关闭/断开即时可见，不残留）。
List<m.ClashConnectionDto> mergeConnectionsById(
  List<m.ClashConnectionDto> existing,
  List<m.ClashConnectionDto> incoming,
) {
  final next = <String, m.ClashConnectionDto>{
    for (final c in incoming) c.id: c,
  };
  final merged = <m.ClashConnectionDto>[];
  for (final c in existing) {
    final updated = next.remove(c.id);
    if (updated != null) merged.add(updated);
  }
  merged.addAll(next.values);
  return merged;
}

/// 有界连接筛选结果。
class ConnectionFilterResult {
  const ConnectionFilterResult({
    required this.items,
    required this.totalMatches,
    required this.truncated,
  });

  /// 本次实际返回的行（至多 [maxFilteredConnections]）。
  final List<m.ClashConnectionDto> items;

  /// 全部命中数（不限上限）。
  final int totalMatches;

  /// 是否因上限被截断。
  final bool truncated;
}

/// 有界筛选：大小写不敏感子串匹配，命中超过 [limit] 即停并标记截断，
/// 避免 10k 行下每次按键全量扫描 + 全量建表。
ConnectionFilterResult filterConnections(
  List<m.ClashConnectionDto> items,
  String needle, {
  int limit = maxFilteredConnections,
}) {
  final query = needle.trim().toLowerCase();
  if (query.isEmpty) {
    final capped = items.length > limit ? items.sublist(0, limit) : items;
    return ConnectionFilterResult(
      items: List<m.ClashConnectionDto>.of(capped),
      totalMatches: items.length,
      truncated: items.length > limit,
    );
  }
  final matched = <m.ClashConnectionDto>[];
  var total = 0;
  for (final c in items) {
    final haystack = <String?>[
      c.host,
      c.connectionType,
      c.network,
      c.processPath,
      c.rule,
      c.chains.join(' '),
    ].whereType<String>().join(' ').toLowerCase();
    if (haystack.contains(query)) {
      total++;
      if (matched.length < limit) matched.add(c);
    }
  }
  return ConnectionFilterResult(
    items: matched,
    totalMatches: total,
    truncated: total > matched.length,
  );
}

/// 排序后的代理分组/节点（新列表，输入不被 mutated）。
class SortedProxies {
  const SortedProxies({required this.groups, required this.nodes});

  final List<m.ClashProxyDto> groups;
  final List<m.ClashProxyDto> nodes;
}

/// 上游 `RefreshProxyDetails` 排序：0 按延迟升序（超时沉底）、1 按名称，
/// 其它保持核心顺序。复制后排序，调用方状态列表不被重排。
SortedProxies sortProxiesForDisplay(
  List<m.ClashProxyDto> groups,
  List<m.ClashProxyDto> nodes,
  Map<String, int> delays,
  int sorting,
) {
  int rank(m.ClashProxyDto proxy) {
    final delay = delays[proxy.name] ?? proxy.delay;
    return delay < 0 ? 1 << 30 : delay;
  }

  final sortedGroups = List<m.ClashProxyDto>.of(groups);
  final sortedNodes = List<m.ClashProxyDto>.of(nodes);
  if (sorting == 0) {
    sortedNodes.sort((a, b) => rank(a).compareTo(rank(b)));
    sortedGroups.sort((a, b) => a.name.compareTo(b.name));
  } else if (sorting == 1) {
    sortedGroups.sort((a, b) => a.name.compareTo(b.name));
    sortedNodes.sort((a, b) => a.name.compareTo(b.name));
  }
  return SortedProxies(groups: sortedGroups, nodes: sortedNodes);
}
