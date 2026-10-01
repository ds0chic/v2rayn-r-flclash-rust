/// Byte/rate formatting shared by the monitor views and status bar.
///
/// Upstream shows raw byte counters scaled to B/KB/MB/GB/TB (1024-based) and
/// rate values as `x/s`. Unknown values render as `--`, never `0`, so an idle
/// session is distinguishable from "no data".
String formatBytes(Object? bytes) {
  if (bytes == null) return '--';
  final value = _asInt(bytes);
  if (value == null || value < 0) return '--';
  return '${_scale(value)}/s';
}

/// Traffic totals use no `/s` suffix.
String formatTraffic(Object? bytes) {
  if (bytes == null) return '--';
  final value = _asInt(bytes);
  if (value == null || value < 0) return '--';
  return _scale(value);
}

/// Rate formatting (`bytes/second` -> `x/s`).
String formatRate(Object? bps) {
  if (bps == null) return '--';
  final value = _asInt(bps);
  if (value == null || value < 0) return '--';
  return '${_scale(value)}/s';
}

int? _asInt(Object value) {
  if (value is int) return value;
  if (value is BigInt) return value.toInt();
  if (value is num) return value.toInt();
  return null;
}

const _units = <String>['B', 'KB', 'MB', 'GB', 'TB', 'PB'];

String _scale(int value) {
  var amount = value.toDouble();
  var unit = 0;
  while (amount >= 1024 && unit < _units.length - 1) {
    amount /= 1024;
    unit++;
  }
  if (unit == 0) return '$value ${_units[unit]}';
  return '${amount.toStringAsFixed(2)} ${_units[unit]}';
}
