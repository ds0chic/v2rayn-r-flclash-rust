import 'dart:convert';
import 'dart:io';

/// Draft UI-state persistence used by T01 to prove a save/reopen/restore loop.
/// This is intentionally a temporary JSON format next to the executable and is
/// not the final AppSettings contract.
abstract class UiStateStore {
  Map<String, double> loadColumnWidths();

  void saveColumnWidths(Map<String, double> widths);

  Map<String, dynamic> loadWindowState();

  void saveWindowState(Map<String, dynamic> state);
}

class FileUiStateStore implements UiStateStore {
  FileUiStateStore({String? overridePath})
    : _path = overridePath ?? _defaultPath();

  final String _path;

  static String _defaultPath() {
    final exeDir = File(Platform.resolvedExecutable).parent.path;
    return '$exeDir${Platform.pathSeparator}v2raynr_ui_state.json';
  }

  Map<String, dynamic> _read() {
    try {
      final file = File(_path);
      if (!file.existsSync()) return <String, dynamic>{};
      final decoded = jsonDecode(file.readAsStringSync());
      if (decoded is Map<String, dynamic>) return decoded;
      return <String, dynamic>{};
    } catch (_) {
      return <String, dynamic>{};
    }
  }

  void _write(Map<String, dynamic> data) {
    try {
      File(_path).writeAsStringSync(jsonEncode(data));
    } catch (_) {
      // Best-effort draft persistence; never fail the UI on I/O errors.
    }
  }

  @override
  Map<String, double> loadColumnWidths() {
    final raw = _read()['column_widths'];
    if (raw is Map) {
      return raw.map((k, v) => MapEntry(k.toString(), (v as num).toDouble()));
    }
    return <String, double>{};
  }

  @override
  void saveColumnWidths(Map<String, double> widths) {
    final data = _read();
    data['column_widths'] = widths;
    _write(data);
  }

  @override
  Map<String, dynamic> loadWindowState() {
    final raw = _read()['window'];
    if (raw is Map<String, dynamic>) return raw;
    return <String, dynamic>{};
  }

  @override
  void saveWindowState(Map<String, dynamic> state) {
    final data = _read();
    data['window'] = state;
    _write(data);
  }
}

class MemoryUiStateStore implements UiStateStore {
  Map<String, double> columns = <String, double>{};
  Map<String, dynamic> window = <String, dynamic>{};

  @override
  Map<String, double> loadColumnWidths() => Map<String, double>.of(columns);

  @override
  void saveColumnWidths(Map<String, double> widths) =>
      columns = Map<String, double>.of(widths);

  @override
  Map<String, dynamic> loadWindowState() => Map<String, dynamic>.of(window);

  @override
  void saveWindowState(Map<String, dynamic> state) =>
      window = Map<String, dynamic>.of(state);
}
