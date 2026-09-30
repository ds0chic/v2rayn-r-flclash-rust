import 'package:v2rayn_desktop/bridge/api/mirrors.dart';

/// Column model for the 14-column profile table. Titles follow the upstream
/// `ExName` values in `compat/layouts.yaml` LAY-PROFILES-002.
class ProfileColumn {
  const ProfileColumn({
    required this.key,
    required this.title,
    required this.width,
    required this.numeric,
    required this.display,
  });

  final String key;
  final String title;
  final double width;
  final bool numeric;
  final String Function(ProfileSummary row) display;
}

/// Default column set and widths from LAY-PROFILES-002.
List<ProfileColumn> defaultProfileColumns() => const <ProfileColumn>[
  ProfileColumn(
    key: 'ConfigType',
    title: 'ConfigType',
    width: 80,
    numeric: false,
    display: _configTypeLabel,
  ),
  ProfileColumn(
    key: 'Remarks',
    title: 'Remarks',
    width: 150,
    numeric: false,
    display: _remarksLabel,
  ),
  ProfileColumn(
    key: 'Address',
    title: 'Address',
    width: 120,
    numeric: false,
    display: _addressLabel,
  ),
  ProfileColumn(
    key: 'Port',
    title: 'Port',
    width: 60,
    numeric: true,
    display: _portLabel,
  ),
  ProfileColumn(
    key: 'Network',
    title: 'Network',
    width: 100,
    numeric: false,
    display: _networkLabel,
  ),
  ProfileColumn(
    key: 'StreamSecurity',
    title: 'StreamSecurity',
    width: 100,
    numeric: false,
    display: _securityLabel,
  ),
  ProfileColumn(
    key: 'SubRemarks',
    title: 'SubRemarks',
    width: 100,
    numeric: false,
    display: _subRemarksLabel,
  ),
  ProfileColumn(
    key: 'DelayVal',
    title: 'DelayVal',
    width: 100,
    numeric: true,
    display: _delayLabel,
  ),
  ProfileColumn(
    key: 'SpeedVal',
    title: 'SpeedVal',
    width: 100,
    numeric: true,
    display: _speedLabel,
  ),
  ProfileColumn(
    key: 'TodayUp',
    title: 'TodayUp',
    width: 100,
    numeric: true,
    display: _todayUpLabel,
  ),
  ProfileColumn(
    key: 'IpInfo',
    title: 'IpInfo',
    width: 100,
    numeric: false,
    display: _ipInfoLabel,
  ),
  ProfileColumn(
    key: 'TodayDown',
    title: 'TodayDown',
    width: 100,
    numeric: true,
    display: _todayDownLabel,
  ),
  ProfileColumn(
    key: 'TotalUp',
    title: 'TotalUp',
    width: 100,
    numeric: true,
    display: _totalUpLabel,
  ),
  ProfileColumn(
    key: 'TotalDown',
    title: 'TotalDown',
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
String _delayLabel(ProfileSummary r) => r.delay < 0 ? '-' : '${r.delay} ms';
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

  SortSpec next(String key) {
    if (columnKey != key || direction == SortDirection.none) {
      return SortSpec(columnKey: key, direction: SortDirection.ascending);
    }
    switch (direction) {
      case SortDirection.ascending:
        return SortSpec(columnKey: key, direction: SortDirection.descending);
      case SortDirection.descending:
        return const SortSpec();
      case SortDirection.none:
        return SortSpec(columnKey: key, direction: SortDirection.ascending);
    }
  }
}

bool rowMatchesQuery(ProfileSummary row, String query) {
  final q = query.trim().toLowerCase();
  if (q.isEmpty) return true;
  return row.remarks.toLowerCase().contains(q) ||
      row.address.toLowerCase().contains(q) ||
      row.subRemarks.toLowerCase().contains(q) ||
      row.ipInfo.toLowerCase().contains(q) ||
      row.configType.name.toLowerCase().contains(q) ||
      row.network.toLowerCase().contains(q) ||
      row.streamSecurity.toLowerCase().contains(q) ||
      row.port.toString().contains(q);
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
  final sorted = List<ProfileSummary>.of(rows);
  sorted.sort((a, b) {
    final cmp = _compare(valueForColumn(column, a), valueForColumn(column, b));
    return sort.direction == SortDirection.ascending ? cmp : -cmp;
  });
  return sorted;
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
