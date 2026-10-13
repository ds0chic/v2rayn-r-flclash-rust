import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;

/// Clipboard text for the message list.
///
/// Matches upstream `MsgView` copy-all: the raw message text of each line, with
/// a newline appended when the line does not already end with one (the upstream
/// text box appends `msg + NewLine`).
String formatLogsForCopy(Iterable<m.LogLineDto> lines) {
  final buffer = StringBuffer();
  for (final line in lines) {
    buffer.write(line.text);
    if (!line.text.endsWith('\n')) buffer.write('\n');
  }
  return buffer.toString();
}

/// Byte/rate formatting shared by the monitor views and status bar.
///
/// Upstream shows raw byte counters scaled to B/KB/MB/GB/TB (1024-based) and
/// rate values as `x/s`. Unknown values render as `--`, never `0`, so an idle
/// session is distinguishable from "no data". Totals use [formatTraffic],
/// rates use [formatRate].
///
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

/// Display magnitudes only: saturate instead of truncating, so wire counters
/// beyond 64 bits (or negative unknowns) never wrap into a wrong unit.
const int _maxDisplay = 0x3FFFFFFFFFFFFFFF;

int? _asInt(Object? value) {
  if (value == null) return null;
  if (value is int) return value < 0 ? -1 : value;
  if (value is BigInt) {
    if (value.isNegative) return -1;
    if (value > BigInt.from(_maxDisplay)) return _maxDisplay;
    return value.toInt();
  }
  if (value is num) {
    final v = value.toInt();
    return v < 0 ? -1 : v;
  }
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
