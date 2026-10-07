import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;

/// SP-20 准备范围：连接表列布局持久化合同与右键关闭目标冻结。
///
/// 原版对照（v2rayN 7.25.4 / 7d6a967，只读 `work/`）：
/// - 默认列与宽度见 `v2rayN/Views/ClashConnectionsView.xaml`：
///   Host=300 / Chain=500 / Network=80 / Type=160 / ProcessPath=100 /
///   Elapsed=100（`ExName` 即稳定列键，禁止用本地化表头做键）。
/// - `ClashConnectionsView.xaml.cs`：`RestoreUI` 按
///   `ClashUIItem.ConnectionsColumnItem.OrderBy(Index)` 恢复，仅 `Width > 0`
///   覆盖；`StorageUI` 在退出时按 `Name`/`ActualWidth`/`DisplayIndex` 回写。
/// - `ServiceLib/ViewModels/ClashConnectionsViewModel.cs`：右键关闭冻结
///   `SelectedSource.Id`（空 Id 不可执行，对应 `canEditRemove`），关闭全部走
///   独立空 id 路径，关闭后重查连接。
///
/// 本文件只做可独立部分：纯列布局归一化/回写形状、关闭目标冻结与过期丢弃、
/// 合成连接夹具。真实连接管理（endpoint+generation 绑定、FRB/运行时接线）
/// 依赖 SP-17（A08 在途），不在本次实现；不做任何 socket/HTTP 调用。

/// 一列的持久化形状（对应上游 `ColumnItem`：Name / Width / Index）。
class ConnectionColumn {
  const ConnectionColumn({
    required this.name,
    required this.width,
    required this.index,
  });

  final String name;
  final int width;
  final int index;
}

/// 上游 `ClashConnectionsView.xaml` 默认列（宽/序）。
List<ConnectionColumn> defaultConnectionColumns() => const <ConnectionColumn>[
  ConnectionColumn(name: 'Host', width: 300, index: 0),
  ConnectionColumn(name: 'Chain', width: 500, index: 1),
  ConnectionColumn(name: 'Network', width: 80, index: 2),
  ConnectionColumn(name: 'Type', width: 160, index: 3),
  ConnectionColumn(name: 'ProcessPath', width: 100, index: 4),
  ConnectionColumn(name: 'Elapsed', width: 100, index: 5),
];

/// One persisted canonical column row (`ClashUIItem.ConnectionsColumnItem[]`
/// entry, upstream `ColumnItem`: Name / Width / Index).
class ConnectionColumnEntry {
  const ConnectionColumnEntry({
    required this.name,
    required this.width,
    required this.index,
  });

  final String name;
  final int width;
  final int index;
}

/// Parse the raw `ConnectionsColumnItem` JSON value into canonical rows.
///
/// Unknown names are kept (roundtrip preservation, FLD-CFG-136); rows with an
/// empty name or a mistyped Width/Index are refused (dropped). Mirrors the
/// profiles G-15 `parseMainColumnItems` contract.
List<ConnectionColumnEntry> parseConnectionColumnItems(Object? raw) {
  if (raw is! List) return const <ConnectionColumnEntry>[];
  final out = <ConnectionColumnEntry>[];
  for (final row in raw) {
    if (row is! Map) continue;
    final name = row['Name'];
    if (name is! String || name.isEmpty) continue;
    final width = row['Width'];
    final index = row['Index'];
    if (width is! num || index is! num) continue;
    out.add(
      ConnectionColumnEntry(
        name: name,
        width: width.toInt(),
        index: index.toInt(),
      ),
    );
  }
  return out;
}

int _asIndex(Object? value, int fallback) {
  if (value is num) return value.toInt();
  return int.tryParse('$value') ?? fallback;
}

int _asWidth(Object? value, int fallback) {
  final parsed = _asIndex(value, fallback);
  return parsed > 0 ? parsed : fallback;
}

/// 将持久化的 `ConnectionsColumnItem` 行归一化为可见列顺序。
///
/// 语义镜像原版 `RestoreUI`：按 `Index` 升序，仅已知列键参与，未知列丢弃，
/// `Width <= 0`/非法回退到上游默认宽，缺失的默认列按上游顺序补齐；返回列
/// 按 0..n-1 重编 `index`，可直接作为下一次 `StorageUI` 回写的顺序。
List<ConnectionColumn> resolveVisibleColumns(
  List<Map<String, dynamic>> persisted,
) {
  final defaults = defaultConnectionColumns();
  final defaultByName = {for (final c in defaults) c.name: c};
  final ranked = <({String name, int width, int rank, int seen})>[];
  var seen = 0;
  for (final row in persisted) {
    final name = row['Name']?.toString() ?? '';
    final fallback = defaultByName[name];
    if (fallback == null) continue;
    ranked.add((
      name: name,
      width: _asWidth(row['Width'], fallback.width),
      rank: _asIndex(row['Index'], fallback.index),
      seen: seen++,
    ));
  }
  // 同名重复行只保留 Index 最小的一行（原版按列匹配首个命中）。
  final deduped = <String, ({int width, int rank, int seen})>{};
  for (final entry in ranked) {
    final current = deduped[entry.name];
    if (current == null ||
        entry.rank < current.rank ||
        (entry.rank == current.rank && entry.seen < current.seen)) {
      deduped[entry.name] = (
        width: entry.width,
        rank: entry.rank,
        seen: entry.seen,
      );
    }
  }
  final ordered = deduped.entries.toList()
    ..sort((a, b) {
      final byRank = a.value.rank.compareTo(b.value.rank);
      if (byRank != 0) return byRank;
      return a.value.seen.compareTo(b.value.seen);
    });
  final placed = ordered.map((e) => e.key).toList();
  for (final fallback in defaults) {
    if (!deduped.containsKey(fallback.name)) placed.add(fallback.name);
  }
  // 保持已排序行的相对顺序，未持久化的默认列按上游顺序补齐：已放置的持久
  // 行在前（Index 顺序），补齐列按默认顺序追加。
  final result = <ConnectionColumn>[];
  for (var i = 0; i < placed.length; i++) {
    final name = placed[i];
    final kept = deduped[name];
    result.add(
      ConnectionColumn(
        name: name,
        width: kept?.width ?? defaultByName[name]!.width,
        index: i,
      ),
    );
  }
  return result;
}

/// 将可见列转回 `ConnectionsColumnItem` 存储行（Name/Width/Index），供调用方
/// 写入设置文档后独立重开读回；形状与上游 `StorageUI` 回写一致。
List<Map<String, dynamic>> connectionColumnsToStorage(
  List<ConnectionColumn> columns,
) => encodeConnectionColumns(columns);

/// Encode display columns back to canonical rows, preserving unknown rows
/// (FLD-CFG-136, profiles G-15 mirror).
///
/// Visible columns store their width, hidden ones are not part of this table
/// (widths are always positive here); Index is the display position. Rows in
/// [preserveUnknownFrom] whose name is not a current display key (unknown
/// columns) are rewritten verbatim after the known rows so a save never
/// drops them.
List<Map<String, dynamic>> encodeConnectionColumns(
  List<ConnectionColumn> columns, {
  List<ConnectionColumnEntry> preserveUnknownFrom =
      const <ConnectionColumnEntry>[],
}) {
  final keys = columns.map((column) => column.name).toSet();
  final out = <Map<String, dynamic>>[
    for (final c in columns)
      <String, dynamic>{'Name': c.name, 'Width': c.width, 'Index': c.index},
  ];
  for (final entry in preserveUnknownFrom) {
    if (entry.name.isEmpty || keys.contains(entry.name)) continue;
    if (out.any((row) => row['Name'] == entry.name)) continue;
    out.add(<String, dynamic>{
      'Name': entry.name,
      'Width': entry.width,
      'Index': entry.index,
    });
  }
  return out;
}

/// 将 [from] 处的列移到 [to] 处并按 0..n-1 重编 `index`（表头拖拽排列用）。
///
/// 越界目标钳制到首尾，不抛异常；宽度随列一起移动。
List<ConnectionColumn> moveConnectionColumn(
  List<ConnectionColumn> columns,
  int from,
  int to,
) {
  if (columns.isEmpty) return const <ConnectionColumn>[];
  final list = columns.toList();
  final entry = list.removeAt(from.clamp(0, list.length - 1));
  list.insert(to.clamp(0, list.length), entry);
  return <ConnectionColumn>[
    for (var i = 0; i < list.length; i++)
      ConnectionColumn(name: list[i].name, width: list[i].width, index: i),
  ];
}

/// 右键关闭冻结的目标：请求发出时冻结 `id` 与会话 `generation`。
class ConnectionCloseTarget {
  const ConnectionCloseTarget({required this.id, required this.generation});

  final String id;
  final int generation;
}

/// 冻结单条关闭目标。空/缺失 id 直接返回 null（镜像原版 `canEditRemove`；
/// 关闭全部另走 [isCloseAllRequest]，禁止把空 id 当单条发送）。
ConnectionCloseTarget? freezeConnectionClose(String? id, int generation) {
  if (id == null || id.isEmpty) return null;
  return ConnectionCloseTarget(id: id, generation: generation);
}

/// 迟到响应的过期检查：仅同 generation 的回包可作用于当前会话。
bool closeResponseIsCurrent(int requestGeneration, int currentGeneration) =>
    requestGeneration == currentGeneration;

/// 关闭全部是独立路径（原版 `ClashConnectionClose(all: true)`）。
bool isCloseAllRequest({required bool closeAll, required String? id}) =>
    closeAll && (id == null || id.isEmpty);

/// 合成连接夹具：纯内存构造，不做任何 socket/HTTP 调用，不触达宿主网络。
///
/// host 统一使用 `*.example.invalid` 合成域，避免与任何真实目标混淆。
List<m.ClashConnectionDto> syntheticConnections(int count, {int seed = 0}) {
  final rows = <m.ClashConnectionDto>[];
  for (var i = 0; i < count; i++) {
    final n = seed * count + i;
    rows.add(
      m.ClashConnectionDto(
        id: 'syn-conn-$n',
        host: 'host-$n.example.invalid:443',
        network: i.isEven ? 'tcp' : 'udp',
        connectionType: 'Shadowsocks',
        chains: <String>['RULE', 'proxy-$n'],
        rule: 'RULE',
        processPath: 'C:\\synthetic\\app-$n.exe',
        source: '10.255.0.${n % 250 + 1}:${10000 + (n % 50000)}',
        destination: '192.0.2.${n % 250 + 1}:443',
        upload: BigInt.from(n),
        download: BigInt.from(n * 2),
        start: null,
      ),
    );
  }
  return rows;
}
