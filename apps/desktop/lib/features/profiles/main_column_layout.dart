// G-15 / FLD-CFG-117/118/119: canonical `UiItem.MainColumnItem` <-> table.
//
// Pure contract over the persisted canonical column layout (upstream
// `ColumnItem`: Name / Width / Index, frozen 7d6a967):
// - `ProfilesView.xaml.cs:RestoreUI`: rows ordered by Index; the first row per
//   grid column wins; Width < 0 hides, otherwise Width + next DisplayIndex.
//   `to*` columns follow `GuiItem.EnableStatistics`, `IpInfo` follows
//   `SpeedTestItem.IPAPIUrl` + `UiItem.HideColumnIpInfo`.
// - `ProfilesView.xaml.cs:StorageUI`: every grid column is written back with
//   Width = visible ? ActualWidth : -1, Index = DisplayIndex.
// - FLD-CFG-117/118/119 storage closure: unknown names are preserved verbatim
//   and rewritten (never dropped); empty names and mistyped Width/Index rows
//   are refused; Width 0 means auto (falls back to the table default).
//
// No bridge/FRB access here; the controller owns the settings read/write seam.
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';

/// Width bounds shared with the table (see `resizeColumn`/`autofitColumns`).
const double mainColumnMinWidth = 40;
const double mainColumnMaxWidth = 600;

/// One persisted canonical column row (`UiItem.MainColumnItem[]` entry).
class MainColumnEntry {
  const MainColumnEntry({
    required this.name,
    required this.width,
    required this.index,
  });

  final String name;
  final int width;
  final int index;
}

/// Parse the raw `MainColumnItem` JSON value into canonical rows.
///
/// Unknown names are kept (roundtrip preservation, FLD-CFG-117); rows with an
/// empty name or a mistyped Width/Index are refused (dropped).
List<MainColumnEntry> parseMainColumnItems(Object? raw) {
  if (raw is! List) return const <MainColumnEntry>[];
  final out = <MainColumnEntry>[];
  for (final row in raw) {
    if (row is! Map) continue;
    final name = row['Name'];
    if (name is! String || name.isEmpty) continue;
    final width = row['Width'];
    final index = row['Index'];
    if (width is! num || index is! num) continue;
    out.add(
      MainColumnEntry(name: name, width: width.toInt(), index: index.toInt()),
    );
  }
  return out;
}

/// Visibility forced by sibling settings (upstream `RestoreUI`).
///
/// Columns whose key starts with `to` (Today/Total Up/Down) are only visible
/// when statistics are enabled; `IpInfo` needs a configured IP API url and no
/// explicit hide flag. Applies on top of the Width<0 hidden sentinel.
bool mainColumnForcedHidden(
  String key, {
  required bool showStatistics,
  required bool showIpInfo,
}) {
  if (key.length >= 2 &&
      (key[0] == 't' || key[0] == 'T') &&
      (key[1] == 'o' || key[1] == 'O')) {
    return !showStatistics;
  }
  if (key == 'IpInfo') return !showIpInfo;
  return false;
}

/// Apply canonical rows onto the default columns (upstream `RestoreUI` port).
///
/// Rows are honored in Index order (stable); the first row per known key wins.
/// Width < 0 hides (default width kept); Width 0 falls back to the default
/// width (auto); positive widths are clamped to the table bounds. Unknown
/// names never reach the table. Known keys missing from [entries] keep their
/// default slot after the ordered ones.
List<ProfileColumn> applyMainColumnLayout(
  List<ProfileColumn> defaults,
  List<MainColumnEntry> entries, {
  required bool showStatistics,
  required bool showIpInfo,
}) {
  final ordered = List<MainColumnEntry>.of(entries)
    ..sort((a, b) => a.index.compareTo(b.index));
  final byKey = <String, ProfileColumn>{
    for (final column in defaults) column.key: column,
  };
  final applied = <ProfileColumn>[];
  final seen = <String>{};
  for (final entry in ordered) {
    if (!seen.add(entry.name)) continue;
    final base = byKey.remove(entry.name);
    if (base == null) continue;
    final hidden =
        entry.width < 0 ||
        mainColumnForcedHidden(
          entry.name,
          showStatistics: showStatistics,
          showIpInfo: showIpInfo,
        );
    final width = entry.width <= 0
        ? base.width
        : entry.width.toDouble().clamp(mainColumnMinWidth, mainColumnMaxWidth);
    applied.add(base.copyWith(width: width, visible: !hidden));
  }
  applied.addAll(defaults.where((column) => byKey.containsKey(column.key)));
  return applied;
}

/// Encode display columns back to canonical rows (upstream `StorageUI` port).
///
/// Visible columns store their truncated width (`(int)ActualWidth`), hidden
/// ones the -1 sentinel; Index is the display position. Rows in
/// [preserveUnknownFrom] whose name is not a current display key (unknown
/// columns, FLD-CFG-117) are rewritten verbatim after the known rows so a
/// save never drops them.
List<Map<String, Object>> encodeMainColumnItems(
  List<ProfileColumn> columns, {
  List<MainColumnEntry> preserveUnknownFrom = const <MainColumnEntry>[],
}) {
  final keys = columns.map((column) => column.key).toSet();
  final out = <Map<String, Object>>[
    for (var i = 0; i < columns.length; i++)
      <String, Object>{
        'Name': columns[i].key,
        'Width': columns[i].visible ? columns[i].width.toInt() : -1,
        'Index': i,
      },
  ];
  for (final entry in preserveUnknownFrom) {
    if (entry.name.isEmpty || keys.contains(entry.name)) continue;
    if (out.any((row) => row['Name'] == entry.name)) continue;
    out.add(<String, Object>{
      'Name': entry.name,
      'Width': entry.width,
      'Index': entry.index,
    });
  }
  return out;
}
