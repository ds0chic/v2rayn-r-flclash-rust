// Wave B / FLD-CFG-156/157/158: canonical `UiItem.WindowSizeItem` <-> window.
//
// Pure contract over the persisted window geometry rows (upstream
// `WindowSizeItem`: TypeName / Width / Height, frozen 7d6a967;
// `ConfigHandler.GetWindowSizeItem` / `SaveWindowSizeItem`, mirrored by
// `application::settings::{get,save}_window_size`):
// - 156 `locateWindowSize`: exact (case-sensitive) TypeName match; degenerate
//   rows (width <= 0 or height <= 0) are treated as absent; rows with an
//   unknown TypeName are retained in storage but no live window asks for them.
// - 157/158 `resolveWindowSize` / `upsertWindowSize`: the stored width/height
//   is applied on open and re-stored on close/resize; a missing or degenerate
//   entry falls back to the default size (never zero-size).
// - `readWindowSizes` / `uiGroupWithWindowSize`: the settings-document seam.
//   There is no dedicated get/save FRB binding, so rows ride inside the `UiItem`
//   group via `saveSettingsGroup` and come back on the next `load`.
//
// No bridge/FRB/window_manager access here; the settings controller owns the
// save seam and the window host applies the resolved size on open and
// re-stores it on close/resize. Multi-window independence is structural:
// every row is keyed by its own TypeName.
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

/// Fallback size when no valid row exists for a TypeName.
///
/// Mirrors `AppWindowMetrics.defaultSize` (1200x800, LAY-MAIN-002) without
/// importing Flutter so this contract stays dependency-free.
const int fallbackWindowWidth = 1200;

/// See [fallbackWindowWidth].
const int fallbackWindowHeight = 800;

/// Parse the raw canonical `UiItem.WindowSizeItem` value into rows.
///
/// Rows with an empty/missing TypeName or a non-map shape are refused;
/// everything else is kept verbatim (including unknown TypeNames and
/// degenerate sizes) so a save never drops them. Degenerate rows simply do
/// not participate in [locateWindowSize].
List<WindowGeometry> parseWindowSizeItems(Object? raw) {
  if (raw is! List) return const <WindowGeometry>[];
  final out = <WindowGeometry>[];
  for (final row in raw) {
    if (row is! Map) continue;
    final name = row['TypeName'];
    if (name is! String || name.isEmpty) continue;
    out.add(
      WindowGeometry.fromJson(
        name,
        row is Map<String, dynamic> ? row : Map<String, dynamic>.from(row),
      ),
    );
  }
  return out;
}

/// Locate the stored geometry row for [typeName] (FLD-CFG-156).
///
/// Exact match only (upstream `==`, case-sensitive); the first row wins and a
/// degenerate row (non-positive width/height, mirroring
/// `get_window_size`) is treated as absent, returning null.
WindowGeometry? locateWindowSize(List<WindowGeometry> rows, String typeName) {
  for (final row in rows) {
    if (row.typeName == typeName) {
      return row.isValid ? row : null;
    }
  }
  return null;
}

/// Resolve the size to apply when opening [typeName] (FLD-CFG-157/158).
///
/// The stored width/height when a valid row exists, otherwise the fallback.
/// The result is never zero-size.
WindowGeometry resolveWindowSize(List<WindowGeometry> rows, String typeName) =>
    locateWindowSize(rows, typeName) ??
    WindowGeometry(
      typeName: typeName,
      width: fallbackWindowWidth,
      height: fallbackWindowHeight,
    );

/// Re-store the size for [typeName] on close/resize (FLD-CFG-157/158).
///
/// Same-TypeName upsert in place (mirroring `save_window_size`, which leaves
/// `MainGirdHeight*`/orientation untouched); any other TypeName is appended.
/// Unknown rows are preserved untouched, so windows never cross-talk.
List<WindowGeometry> upsertWindowSize(
  List<WindowGeometry> rows,
  String typeName,
  int width,
  int height,
) {
  final next = List<WindowGeometry>.of(rows);
  for (var i = 0; i < next.length; i++) {
    if (next[i].typeName == typeName) {
      next[i] = next[i].copyWith(width: width, height: height);
      return next;
    }
  }
  next.add(WindowGeometry(typeName: typeName, width: width, height: height));
  return next;
}

/// Encode rows back to canonical `WindowSizeItem` JSON.
List<Map<String, Object?>> encodeWindowSizeItems(List<WindowGeometry> rows) =>
    rows.map((row) => Map<String, Object?>.of(row.toJson())).toList();

/// Read the canonical rows from a loaded settings document.
List<WindowGeometry> readWindowSizes(Map<String, dynamic> document) {
  final ui = document['UiItem'];
  if (ui is! Map) return const <WindowGeometry>[];
  return parseWindowSizeItems(ui['WindowSizeItem']);
}

/// Build the new `UiItem` group value persisting [typeName] at [width]x[height].
///
/// Copies every other `UiItem` key untouched; pass the result to
/// `saveSettingsGroup('UiItem', ...)`. Unknown rows survive the roundtrip.
Map<String, dynamic> uiGroupWithWindowSize(
  Map<String, dynamic> document,
  String typeName,
  int width,
  int height,
) {
  final rawUi = document['UiItem'];
  final ui = rawUi is Map<String, dynamic>
      ? Map<String, dynamic>.of(rawUi)
      : rawUi is Map
      ? Map<String, dynamic>.from(rawUi)
      : <String, dynamic>{};
  final rows = parseWindowSizeItems(ui['WindowSizeItem']);
  ui['WindowSizeItem'] = encodeWindowSizeItems(
    upsertWindowSize(rows, typeName, width, height),
  );
  return ui;
}
