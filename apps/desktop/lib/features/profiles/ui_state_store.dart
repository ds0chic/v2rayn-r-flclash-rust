import 'dart:convert';
import 'dart:io';

/// Draft UI-state persistence used to prove a save/reopen/restore loop.
///
/// The whole document lives in a single JSON file named `ui_state.json` next to
/// the executable. It is intentionally a temporary format: `docs/decisions/
/// T05-ui-state.md` records the schema and the migration plan to the T02
/// AppEngine store. Until then this is the single source of truth for layout,
/// column and theme preferences so the UI never fabricates runtime state.
abstract class UiStateStore {
  Map<String, double> loadColumnWidths();

  void saveColumnWidths(Map<String, double> widths);

  Map<String, dynamic> loadWindowState();

  void saveWindowState(Map<String, dynamic> state);

  /// Generic persisted section, used for layout/column/theme/status state.
  Map<String, dynamic>? loadSection(String key);

  void saveSection(String key, Map<String, dynamic> value);

  Map<String, dynamic> loadDocument();

  void saveDocument(Map<String, dynamic> data);
}

class FileUiStateStore implements UiStateStore {
  FileUiStateStore({String? overridePath})
    : _path = overridePath ?? _defaultPath();

  final String _path;

  static String _defaultPath() {
    final exeDir = File(Platform.resolvedExecutable).parent.path;
    return '$exeDir${Platform.pathSeparator}ui_state.json';
  }

  @override
  Map<String, dynamic> loadDocument() => _read();

  @override
  void saveDocument(Map<String, dynamic> data) => _write(data);

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

  @override
  Map<String, dynamic>? loadSection(String key) {
    final raw = _read()[key];
    if (raw is Map<String, dynamic>) return raw;
    return null;
  }

  @override
  void saveSection(String key, Map<String, dynamic> value) {
    final data = _read();
    data[key] = value;
    _write(data);
  }
}

class MemoryUiStateStore implements UiStateStore {
  Map<String, double> columns = <String, double>{};
  Map<String, dynamic> window = <String, dynamic>{};
  final Map<String, Map<String, dynamic>> sections =
      <String, Map<String, dynamic>>{};

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

  @override
  Map<String, dynamic>? loadSection(String key) {
    final value = sections[key];
    return value == null ? null : Map<String, dynamic>.of(value);
  }

  @override
  void saveSection(String key, Map<String, dynamic> value) =>
      sections[key] = Map<String, dynamic>.of(value);

  @override
  Map<String, dynamic> loadDocument() => <String, dynamic>{
    if (columns.isNotEmpty) 'column_widths': columns,
    if (window.isNotEmpty) 'window': window,
    for (final entry in sections.entries) entry.key: entry.value,
  };

  @override
  void saveDocument(Map<String, dynamic> data) {
    sections
      ..clear()
      ..addAll(<String, Map<String, dynamic>>{
        for (final entry in data.entries)
          if (entry.value is Map)
            entry.key: Map<String, dynamic>.from(entry.value as Map),
      });
  }
}
