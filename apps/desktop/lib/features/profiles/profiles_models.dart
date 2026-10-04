import 'package:v2rayn_desktop/bridge/api/mirrors.dart';

/// Column model for the 14-column profile table (LAY-PROFILES-002).
///
/// `key` is the stable upstream `ExName` used for persistence, sorting and the
/// FRB field; `title` is the localized display label from
/// `ResUI.zh-Hans.resx` and is never used as a storage key.
class ProfileColumn {
  const ProfileColumn({
    required this.key,
    required this.title,
    required this.width,
    required this.numeric,
    required this.display,
    this.visible = true,
    this.flexible = false,
    double? minWidth,
  }) : minWidth = minWidth ?? width;

  final String key;
  final String title;
  final double width;
  final bool numeric;
  final String Function(ProfileSummary row) display;

  /// Column visibility, persisted through `ui_state.json` (LAY-PROFILES-003).
  final bool visible;

  /// Key columns absorb the remaining viewport width before the table starts
  /// scrolling horizontally; non-flexible columns keep their fixed width.
  final bool flexible;

  /// Lower bound used when the viewport cannot fit every column.
  final double minWidth;

  ProfileColumn copyWith({
    String? key,
    String? title,
    double? width,
    bool? numeric,
    String Function(ProfileSummary row)? display,
    bool? visible,
    bool? flexible,
    double? minWidth,
  }) {
    return ProfileColumn(
      key: key ?? this.key,
      title: title ?? this.title,
      width: width ?? this.width,
      numeric: numeric ?? this.numeric,
      display: display ?? this.display,
      visible: visible ?? this.visible,
      flexible: flexible ?? this.flexible,
      minWidth: minWidth ?? this.minWidth,
    );
  }
}

/// Final column widths for [available] logical pixels of table viewport
/// (including the row-handle gutter is the caller's `fixedChrome`).
///
/// Key (flexible) columns grow to absorb spare width and shrink toward their
/// `minWidth`; when even the minimums cannot fit, the returned total exceeds
/// `available` and the table scrolls horizontally instead of clipping.
List<double> fittedColumnWidths(
  List<ProfileColumn> columns,
  double available, {
  double fixedChrome = 0,
}) {
  if (columns.isEmpty) return const <double>[];
  final flexCount = columns.where((c) => c.flexible).length;
  final availableForColumns = available - fixedChrome;
  final baseTotal = columns.fold<double>(0, (sum, c) => sum + c.width);
  if (flexCount == 0) {
    return <double>[for (final c in columns) c.width];
  }
  final minTotal = columns.fold<double>(
    0,
    (sum, c) => sum + (c.flexible ? c.minWidth : c.width),
  );
  if (availableForColumns <= minTotal || baseTotal <= minTotal) {
    return <double>[for (final c in columns) c.flexible ? c.minWidth : c.width];
  }
  if (availableForColumns >= baseTotal) {
    final per = (availableForColumns - baseTotal) / flexCount;
    return <double>[
      for (final c in columns) c.flexible ? c.width + per : c.width,
    ];
  }
  final ratio = (availableForColumns - minTotal) / (baseTotal - minTotal);
  return <double>[
    for (final c in columns)
      c.flexible ? c.minWidth + (c.width - c.minWidth) * ratio : c.width,
  ];
}

/// Default column set and widths from LAY-PROFILES-002.
List<ProfileColumn> defaultProfileColumns() => const <ProfileColumn>[
  ProfileColumn(
    key: 'ConfigType',
    title: '类型',
    width: 80,
    numeric: false,
    display: _configTypeLabel,
  ),
  ProfileColumn(
    key: 'Remarks',
    title: '别名',
    width: 150,
    numeric: false,
    display: _remarksLabel,
    flexible: true,
    minWidth: 120,
  ),
  ProfileColumn(
    key: 'Address',
    title: '地址',
    width: 120,
    numeric: false,
    display: _addressLabel,
    flexible: true,
    minWidth: 100,
  ),
  ProfileColumn(
    key: 'Port',
    title: '端口',
    width: 60,
    numeric: true,
    display: _portLabel,
  ),
  ProfileColumn(
    key: 'Network',
    title: '传输协议',
    width: 100,
    numeric: false,
    display: _networkLabel,
  ),
  ProfileColumn(
    key: 'StreamSecurity',
    title: 'TLS',
    width: 100,
    numeric: false,
    display: _securityLabel,
  ),
  ProfileColumn(
    key: 'SubRemarks',
    title: '订阅分组',
    width: 100,
    numeric: false,
    display: _subRemarksLabel,
    flexible: true,
    minWidth: 90,
  ),
  ProfileColumn(
    key: 'DelayVal',
    title: '延迟 (ms)',
    width: 100,
    numeric: true,
    display: _delayLabel,
  ),
  ProfileColumn(
    key: 'SpeedVal',
    title: '速度 (MB/s)',
    width: 100,
    numeric: true,
    display: _speedLabel,
  ),
  ProfileColumn(
    key: 'TodayUp',
    title: '今日上传',
    width: 100,
    numeric: true,
    display: _todayUpLabel,
  ),
  ProfileColumn(
    key: 'IpInfo',
    title: 'IP 信息',
    width: 100,
    numeric: false,
    display: _ipInfoLabel,
  ),
  ProfileColumn(
    key: 'TodayDown',
    title: '今日下载',
    width: 100,
    numeric: true,
    display: _todayDownLabel,
  ),
  ProfileColumn(
    key: 'TotalUp',
    title: '总上传',
    width: 100,
    numeric: true,
    display: _totalUpLabel,
  ),
  ProfileColumn(
    key: 'TotalDown',
    title: '总下载',
    width: 100,
    numeric: true,
    display: _totalDownLabel,
  ),
];

String _configTypeLabel(ProfileSummary r) => r.configType.name;
String _remarksLabel(ProfileSummary r) => r.remarks;
String _addressLabel(ProfileSummary r) => r.address;
String _portLabel(ProfileSummary r) => r.port.toString();
String _networkLabel(ProfileSummary r) => r.network;
String _securityLabel(ProfileSummary r) => r.streamSecurity;
String _subRemarksLabel(ProfileSummary r) => r.subRemarks;
String _delayLabel(ProfileSummary r) {
  // `-2` is the overlay's "tested and failed" sentinel (see
  // `applySpeedTestOverlay`/`profileDelayTestFailed`); `-1` stays "never tested".
  if (r.delay == -2) return '失败';
  if (r.delay < 0) return '-';
  return '${r.delay} ms';
}

String _speedLabel(ProfileSummary r) => r.speed;
String _todayUpLabel(ProfileSummary r) => formatBytes(r.todayUp);
String _ipInfoLabel(ProfileSummary r) => r.ipInfo;
String _todayDownLabel(ProfileSummary r) => formatBytes(r.todayDown);
String _totalUpLabel(ProfileSummary r) => formatBytes(r.totalUp);
String _totalDownLabel(ProfileSummary r) => formatBytes(r.totalDown);

/// Human-readable byte formatting used by the traffic columns.
String formatBytes(BigInt bytes) {
  const units = <String>['B', 'KB', 'MB', 'GB', 'TB'];
  var value = bytes.toDouble();
  var unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  final decimals = unit == 0 ? 0 : 1;
  return '${value.toStringAsFixed(decimals)} ${units[unit]}';
}

enum SortDirection { none, ascending, descending }

class SortSpec {
  const SortSpec({this.columnKey, this.direction = SortDirection.none});

  final String? columnKey;
  final SortDirection direction;

  /// Two-way toggle for the same column (`SortDirection.none` is only the
  /// "no column sort" initial/manual-drag-cleared state).
  ///
  /// Mirrors upstream `ProfilesViewModel.SortServer` + `_dicHeaderSort`: the
  /// first click on a column is ascending, each repeat click on the same
  /// column flips ascending/descending; a click on another column resets to
  /// ascending. It never cycles back to "no sort".
  SortSpec next(String key) {
    if (columnKey != key || direction == SortDirection.none) {
      return SortSpec(columnKey: key, direction: SortDirection.ascending);
    }
    return direction == SortDirection.ascending
        ? SortSpec(columnKey: key, direction: SortDirection.descending)
        : SortSpec(columnKey: key, direction: SortDirection.ascending);
  }
}

/// Reorder [rows] to follow the persisted `ProfileExItem.Sort` order.
///
/// [persistedOrder] is the id sequence read back from the result store, whose
/// list order already reflects `Sort` (`ProfileExStore::all`). Rows absent from
/// it keep their relative order and are appended last; no row is ever dropped.
/// This is how the node table reads back the order written by
/// `applyProfileOrder` after a reload, a speedtest poll, or a reopen.
List<ProfileSummary> orderByPersistedSort(
  List<ProfileSummary> rows,
  List<String> persistedOrder,
) {
  if (persistedOrder.isEmpty || rows.length < 2) {
    return List<ProfileSummary>.of(rows);
  }
  final rank = <String, int>{};
  for (var i = 0; i < persistedOrder.length; i++) {
    rank.putIfAbsent(persistedOrder[i], () => i);
  }
  final indexed = <(int, int, ProfileSummary)>[
    for (var i = 0; i < rows.length; i++)
      (rank[rows[i].id] ?? (1 << 30), i, rows[i]),
  ];
  indexed.sort((a, b) {
    final byRank = a.$1.compareTo(b.$1);
    return byRank != 0 ? byRank : a.$2.compareTo(b.$2);
  });
  return indexed.map((e) => e.$3).toList();
}

bool rowMatchesQuery(ProfileSummary row, String query) {
  final q = query.trim().toLowerCase();
  if (q.isEmpty) return true;
  // Frozen `AppManager.ProfileModels` filters with
  // `a.remarks like '%q%' or a.address like '%q%'` (AppManager.cs:226), so the
  // table text filter only searches remarks and address.
  return row.remarks.toLowerCase().contains(q) ||
      row.address.toLowerCase().contains(q);
}

List<ProfileSummary> applyFilter(List<ProfileSummary> rows, String query) {
  if (query.trim().isEmpty) return List<ProfileSummary>.of(rows);
  return rows.where((r) => rowMatchesQuery(r, query)).toList();
}

Object? valueForColumn(ProfileColumn column, ProfileSummary row) {
  switch (column.key) {
    case 'ConfigType':
      return row.configType.name;
    case 'Remarks':
      return row.remarks;
    case 'Address':
      return row.address;
    case 'Port':
      return row.port;
    case 'Network':
      return row.network;
    case 'StreamSecurity':
      return row.streamSecurity;
    case 'SubRemarks':
      return row.subRemarks;
    case 'DelayVal':
      return row.delay;
    case 'SpeedVal':
      return parseSpeed(row.speed);
    case 'TodayUp':
      return row.todayUp;
    case 'IpInfo':
      return row.ipInfo;
    case 'TodayDown':
      return row.todayDown;
    case 'TotalUp':
      return row.totalUp;
    case 'TotalDown':
      return row.totalDown;
    default:
      return null;
  }
}

double parseSpeed(String speed) {
  final match = RegExp(r'[0-9]+(\.[0-9]+)?').firstMatch(speed);
  if (match == null) return -1;
  return double.tryParse(match.group(0)!) ?? -1;
}

List<ProfileSummary> applySort(
  List<ProfileSummary> rows,
  List<ProfileColumn> columns,
  SortSpec sort,
) {
  if (sort.columnKey == null || sort.direction == SortDirection.none) {
    return List<ProfileSummary>.of(rows);
  }
  final column = columns.firstWhere(
    (c) => c.key == sort.columnKey,
    orElse: () => columns.first,
  );
  // Stable sort: decorate with the original index so equal rows keep their
  // relative order (the plan requires selection/scroll anchors to survive).
  final indexed = <(int, ProfileSummary)>[
    for (var i = 0; i < rows.length; i++) (i, rows[i]),
  ];
  indexed.sort((a, b) {
    final cmp = _compare(
      valueForColumn(column, a.$2),
      valueForColumn(column, b.$2),
    );
    if (cmp == 0) return a.$1.compareTo(b.$1);
    return sort.direction == SortDirection.ascending ? cmp : -cmp;
  });
  return indexed.map((e) => e.$2).toList();
}

int _compare(Object? a, Object? b) {
  if (a is num && b is num) return a.compareTo(b);
  if (a is BigInt && b is BigInt) return a.compareTo(b);
  if (a is BigInt && b is num) return a.compareTo(BigInt.from(b.toInt()));
  if (a is num && b is BigInt) return BigInt.from(a.toInt()).compareTo(b);
  final sa = a?.toString() ?? '';
  final sb = b?.toString() ?? '';
  return sa.compareTo(sb);
}
