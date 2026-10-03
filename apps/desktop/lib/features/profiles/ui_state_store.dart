import 'dart:convert';
import 'dart:io';

/// UI-state persistence used to prove a save/reopen/restore loop.
///
/// The whole document lives in a single JSON file named `ui_state.json` next to
/// the executable. It is intentionally a temporary format: `docs/decisions/
/// T05-ui-state.md` records the schema and the migration plan to the T02
/// AppEngine store. Until then this is the single source of truth for layout,
/// column and theme preferences so the UI never fabricates runtime state.
///
/// Geometry and column widths share one consistent, migratable source:
/// - every window is keyed by its upstream `GetType().Name` (`WindowSizeItem`
///   `TypeName`), mirroring `ConfigHandler.GetWindowSizeItem`/`SaveWindowSizeItem`;
/// - every table (node list, Clash connections) uses the same [ColumnLayout]
///   shape (`order`/`visible`/`widths`), mirroring upstream `ColumnItem`
///   (`Name`/`Width`/`Index`) for `UiItem.MainColumnItem` and
///   `ClashUIItem.ConnectionsColumnItem`.
abstract class UiStateStore {
  /// Canonical section holding [WindowGeometry] rows keyed by `TypeName`.
  static const String windowGeometrySection = 'window_geometry';

  /// Bookkeeping section (schema version + provenance).
  static const String metaSection = 'meta';

  /// Schema version written by [migrateLegacyUiState].
  static const int schemaVersion = 2;

  static const String _profilesColumnSection = 'column_layout';

  /// Live section for the Clash connections table (LAY-CLASHCN-002).
  static const String clashConnectionsColumnSection =
      'clash_connections_column_layout';

  /// Section name holding the persisted [ColumnLayout] for [table].
  static String columnSectionFor(String table) => switch (table) {
    ColumnTable.profiles => _profilesColumnSection,
    ColumnTable.clashConnections => clashConnectionsColumnSection,
    _ => '${table}_column_layout',
  };

  Map<String, double> loadColumnWidths();

  void saveColumnWidths(Map<String, double> widths);

  Map<String, dynamic> loadWindowState();

  void saveWindowState(Map<String, dynamic> state);

  /// Generic persisted section, used for layout/column/theme/status state.
  Map<String, dynamic>? loadSection(String key);

  void saveSection(String key, Map<String, dynamic> value);

  Map<String, dynamic> loadDocument();

  void saveDocument(Map<String, dynamic> data);

  /// Persisted column order/visibility/widths for one table.
  ColumnLayout loadColumnLayout(String table) {
    final section = loadSection(columnSectionFor(table));
    if (section != null) return ColumnLayout.fromJson(section);
    if (table == ColumnTable.profiles) {
      final legacy = loadColumnWidths();
      if (legacy.isNotEmpty) return ColumnLayout(widths: legacy);
    }
    return const ColumnLayout();
  }

  void saveColumnLayout(String table, ColumnLayout layout) =>
      saveSection(columnSectionFor(table), layout.toJson());

  /// Every persisted window geometry row, keyed by upstream `TypeName`.
  Map<String, WindowGeometry> loadWindowGeometries() {
    final raw = loadSection(windowGeometrySection);
    if (raw != null && raw.isNotEmpty) {
      final result = <String, WindowGeometry>{};
      for (final entry in raw.entries) {
        final value = entry.value;
        if (value is Map) {
          result[entry.key] = WindowGeometry.fromJson(
            entry.key,
            Map<String, dynamic>.from(value),
          );
        }
      }
      if (result.isNotEmpty) return result;
    }
    // Legacy single-window blob written before TypeName keys existed.
    final migrated = windowGeometryFromLegacy(loadWindowState());
    return migrated == null
        ? const <String, WindowGeometry>{}
        : <String, WindowGeometry>{migrated.typeName: migrated};
  }

  WindowGeometry? loadWindowGeometry(String typeName) =>
      loadWindowGeometries()[typeName];

  void saveWindowGeometry(WindowGeometry geometry) {
    final raw = loadSection(windowGeometrySection) ?? <String, dynamic>{};
    final next = Map<String, dynamic>.of(raw);
    next[geometry.typeName] = geometry.toJson();
    saveSection(windowGeometrySection, next);
  }

  /// Idempotently fold the pre-TypeName layout blobs into the canonical
  /// sections. Legacy keys are left in place so no data is lost; this is safe to
  /// call on every startup and after a backup restore.
  void migrateLegacyUiState() {
    if (loadSection(columnSectionFor(ColumnTable.profiles)) == null) {
      final legacy = loadColumnWidths();
      if (legacy.isNotEmpty) {
        saveColumnLayout(ColumnTable.profiles, ColumnLayout(widths: legacy));
      }
    }
    if (loadSection(windowGeometrySection) == null) {
      final migrated = windowGeometryFromLegacy(loadWindowState());
      if (migrated != null) saveWindowGeometry(migrated);
    }
    saveSection(metaSection, <String, dynamic>{
      'schema_version': schemaVersion,
    });
  }
}

/// Table ids for [UiStateStore.loadColumnLayout].
class ColumnTable {
  const ColumnTable._();

  static const String profiles = 'profiles';
  static const String clashConnections = 'clash_connections';
}

/// Upstream `GetType().Name` keys persisted in `UiItem.WindowSizeItem`.
///
/// Mirrors the `WindowBase<...>` subclasses' `GetType().Name`; update/backup are
/// embedded MainWindow views, not separate sized windows.
class WindowTypeNames {
  const WindowTypeNames._();

  static const String main = 'MainWindow';
  static const String optionSetting = 'OptionSettingWindow';
  static const String subSetting = 'SubSettingWindow';
  static const String routingSetting = 'RoutingSettingWindow';
  static const String routingRuleSetting = 'RoutingRuleSettingWindow';
  static const String routingRuleDetails = 'RoutingRuleDetailsWindow';
  static const String dnsSetting = 'DNSSettingWindow';
  static const String globalHotkeySetting = 'GlobalHotkeySettingWindow';
  static const String fullConfigTemplate = 'FullConfigTemplateWindow';
  static const String addServer = 'AddServerWindow';
  static const String addServer2 = 'AddServer2Window';
  static const String addGroupServer = 'AddGroupServerWindow';
  static const String profilesSelect = 'ProfilesSelectWindow';
  static const String subEdit = 'SubEditWindow';

  static const List<String> all = <String>[
    main,
    optionSetting,
    subSetting,
    routingSetting,
    routingRuleSetting,
    routingRuleDetails,
    dnsSetting,
    globalHotkeySetting,
    fullConfigTemplate,
    addServer,
    addServer2,
    addGroupServer,
    profilesSelect,
    subEdit,
  ];
}

/// Persisted column order/visibility/widths for one table.
///
/// The shape is shared by the node table (`UiItem.MainColumnItem`) and the
/// Clash connections table (`ClashUIItem.ConnectionsColumnItem`), keyed by the
/// stable column key (upstream `Name`, never the localized label).
class ColumnLayout {
  const ColumnLayout({
    this.order = const <String>[],
    this.visible = const <String, bool>{},
    this.widths = const <String, double>{},
  });

  factory ColumnLayout.fromJson(Map<String, dynamic> raw) => ColumnLayout(
    order:
        (raw['order'] as List?)?.map((dynamic e) => e.toString()).toList() ??
        const <String>[],
    visible:
        (raw['visible'] as Map?)?.map(
          (k, v) => MapEntry(k.toString(), v == true),
        ) ??
        const <String, bool>{},
    widths:
        (raw['widths'] as Map?)?.map(
          (k, v) => MapEntry(
            k.toString(),
            v is num ? v.toDouble() : double.tryParse('$v') ?? 0,
          ),
        ) ??
        const <String, double>{},
  );

  final List<String> order;
  final Map<String, bool> visible;
  final Map<String, double> widths;

  bool get isEmpty => order.isEmpty && visible.isEmpty && widths.isEmpty;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'order': order,
    'visible': visible,
    'widths': widths,
  };
}

/// One window's persisted geometry, mirroring upstream `WindowSizeItem`
/// (`TypeName`/`Width`/`Height`) extended with `UiItem.MainGirdHeight1/2` and
/// `MainGirdOrientation` (`EGirdOrientation`: 0 horizontal / 1 vertical / 2 tab).
class WindowGeometry {
  const WindowGeometry({
    required this.typeName,
    required this.width,
    required this.height,
    this.mainGridHeight1 = 0,
    this.mainGridHeight2 = 0,
    this.orientation = 1,
  });

  factory WindowGeometry.fromJson(String typeName, Map<String, dynamic> raw) =>
      WindowGeometry(
        typeName: typeName,
        width: _readInt(raw, 'Width', 'width'),
        height: _readInt(raw, 'Height', 'height'),
        mainGridHeight1: _readInt(raw, 'MainGirdHeight1', 'main_grid_height1'),
        mainGridHeight2: _readInt(raw, 'MainGirdHeight2', 'main_grid_height2'),
        orientation: _readInt(raw, 'Orientation', 'orientation', fallback: 1),
      );

  final String typeName;
  final int width;
  final int height;
  final int mainGridHeight1;
  final int mainGridHeight2;
  final int orientation;

  /// Upstream only restores a row when both dimensions are positive.
  bool get isValid => width > 0 && height > 0;

  WindowGeometry copyWith({
    String? typeName,
    int? width,
    int? height,
    int? mainGridHeight1,
    int? mainGridHeight2,
    int? orientation,
  }) => WindowGeometry(
    typeName: typeName ?? this.typeName,
    width: width ?? this.width,
    height: height ?? this.height,
    mainGridHeight1: mainGridHeight1 ?? this.mainGridHeight1,
    mainGridHeight2: mainGridHeight2 ?? this.mainGridHeight2,
    orientation: orientation ?? this.orientation,
  );

  Map<String, dynamic> toJson() => <String, dynamic>{
    'TypeName': typeName,
    'Width': width,
    'Height': height,
    if (mainGridHeight1 != 0) 'MainGirdHeight1': mainGridHeight1,
    if (mainGridHeight2 != 0) 'MainGirdHeight2': mainGridHeight2,
    if (orientation != 1) 'Orientation': orientation,
  };

  static int _readInt(
    Map<String, dynamic> raw,
    String pascal,
    String snake, {
    int? fallback,
  }) {
    final value = raw[pascal] ?? raw[snake];
    if (value is num) return value.toInt();
    final parsed = int.tryParse('$value');
    return parsed ?? fallback ?? 0;
  }
}

/// Parse the pre-TypeName flat `window` blob into a [WindowGeometry].
///
/// Accepts both a flat `{"width":..,"height":..}` map (assumed `MainWindow`) and
/// a `TypeName`-keyed map of maps.
WindowGeometry? windowGeometryFromLegacy(Map<String, dynamic> legacy) {
  if (legacy.isEmpty) return null;
  final maps = legacy.values.whereType<Map>().toList();
  if (maps.length == legacy.length) {
    for (final entry in legacy.entries) {
      final geometry = WindowGeometry.fromJson(
        entry.key,
        Map<String, dynamic>.from(entry.value as Map),
      );
      if (geometry.isValid) return geometry;
    }
    return null;
  }
  final geometry = WindowGeometry.fromJson(WindowTypeNames.main, legacy);
  return geometry.isValid ? geometry : null;
}

class FileUiStateStore extends UiStateStore {
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

class MemoryUiStateStore extends UiStateStore {
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
