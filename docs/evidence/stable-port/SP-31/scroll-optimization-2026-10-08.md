# SP-31 scroll row-build cost reductions (2026-10-08, local-only)

Implements optimizations (1)-(3) from
`scroll-hotspot-analysis-2026-10-07.md`, one at a time, in
`apps/desktop/lib/features/profiles/profiles_table.dart` only.
No builds, no cargo, no release/armed runs (integrator measures per the
heavy-load plan in the analysis doc). No behavior, selection/context-menu,
or column-persistence change. No commit.

## Change 1 — per-row `TableSpan` cache

`_ProfilesTableState._rowSpanCache: Map<String, TableSpan>`, keyed by
`index | rowKey | columnRevision | overlayRevision | visualRevision`:

- `rowKey`: `'header'` for row 0, else the row id
  (`oob-$index` for an out-of-range index, same guard as before).
- `columnRevision`: `Object.hashAll` over
  `(key, width, visible, flexible, minWidth)` per visible column —
  O(columns) ≈ 14 hashes, once per build.
- `overlayRevision`: O(1) `Object.hash(speedTestGeneration, lastAckSeq,
  all.length, visible.length)`. Span visuals never consume overlay content,
  so this is structural/generation invalidation only; a stale hit is
  impossible because resolved visuals are also in the key. Deliberately O(1):
  hashing all 50k rows per frame would defeat the optimization.
- `visualRevision`: `Object.hash` of the once-per-build resolved visuals
  (row height, header fill, grid-line color, zebra flag/stripe), so a theme
  or font-size change misses the cache and rebuilds spans.
- `index` is in the key so a reorder (same id, new position) recomputes the
  parity-dependent zebra fill instead of reusing a stale span.

Why it reduces per-frame work: previously `_buildRowSpan` ran
`context.semantics` (Theme.of + extension lookup) plus `Theme.of` and built
fresh `TableSpan`/`TableSpanDecoration`/`BorderSide` objects for every
visible row on every build (once per scroll frame). Now the per-build
`build()` resolves Theme/semantics once, and a scroll frame with unchanged
columns/overlay/visuals is one string key + one map lookup per visible row.
Cache is bounded (cleared past 1024 entries ≈ 10x a viewport) and cleared in
`dispose()`.

## Change 2 — `RepaintBoundary` per cell; `const` audit

- `cellBuilder` now returns
  `TableViewCell(child: RepaintBoundary(child: _buildCell(...)))`.
  Build still flows through every frame; only paint is isolated, so a
  partial update (selection change, 150 ms delay-overlay tick on one row)
  does not repaint sibling cells. No key/finder/hit-test change
  (verified: no `ancestor`/`descendant` finder on table cells in the target
  tests; `getRect`/tap/drag semantics unchanged).
- `const` audit of the scroll hot path: padding, text styles, tooltip
  `Duration`, `SizedBox.shrink`, and the header-handle `ValueKey` were
  already `const`. Two attempted further `const`s were evaluated and
  deliberately NOT kept:
  - `SizedBox.shrink()` in place of the header-handle `Container` was
    written, then reverted: `Container` with no child sizes to max
    constraints while `SizedBox.shrink()` sizes to zero — not provably
    identical under loose constraints, and it dropped the `header-handle`
    key type some tests may rely on.
  - `const Container(...)` fails analyze (`const_with_non_const`:
    `Container` has no const constructor). Reverted to the original line.

Known tradeoff for the integrator's measurement: one boundary per visible
cell adds render objects/layers (~columns × visible rows). Raster is
currently flat (~2 ms), so if the armed scroll run shows raster regression
without build gain, this change is the first candidate to revert — it is a
one-hunk revert (`cellBuilder` only).

## Change 3 — per-build `_CellBuildScope` for `cellBuilder`

New private `_CellBuildScope` (bottom of `profiles_table.dart`), built once
in `build()` and consumed by `_buildCell`/`_headerCell`/`_dataCell`/
`_handleCell` instead of per-cell recomputation:

- Hoisted (were per cell / per handle-row per build): one
  `ref.read(profilesControllerProvider.notifier)` (was once per visible
  handle row), `semantics.selectedRow`, `activeRowFill`,
  `activeRowMarkerColor`, `primaryContainer` (selected badge),
  `secondaryContainer` (drag feedback, built eagerly by `Draggable` every
  frame), `primary` (drop-target border). That is ~2-4 inherited-widget
  lookups × visible cells per frame eliminated; remaining per-cell work is
  the unavoidable content itself (`column.display(row)`, tooltip gating,
  `Set.contains` membership).
- `onSortHeader` closure (`(key) => _sortWithAnchor(key, state)`) allocated
  once per build and passed down; header tap/resize behavior identical
  (same state, same controller, same anchor logic).
- The `DragTarget` hover builder keeps event-time semantics but now reads
  the hoisted `scope.dropTargetBorder` instead of its own `Theme.of`.

Behavior-identity argument: every hoisted value is resolved from the same
build context/state the per-cell code read (no `Theme` scope sits between
`build()`'s context and the cells), so colors/sort/selection/controller
are value-identical; only lookup repetition is gone. Theme-change rebuild
behavior is preserved (build subscribes once; scope is rebuilt with it).
Column persistence untouched (no controller/store change).

## Checks (all local, targeted only)

Flutter: `C:\Users\Colby\toolchains\flutter\bin\flutter.bat`, workdir
`apps/desktop`. Note: two task-listed paths were wrong; the real files are
`test/table_actions_test.dart` and
`test/ux_space01_column_persistence_test.dart` (top-level `test/`, not
`test/repair/`).

Per change: `dart format` (write, then check-mode `0 changed`),
`flutter analyze` (`No issues found!`), and the 8 files run individually.
A `Color.value` first draft of change 1 introduced 3
`deprecated_member_use` infos; replaced with `toARGB32()` — analyze clean
since.

| file | ch1 | ch2 | ch3 |
|---|---|---|---|
| `test/repair/wave_b_g15_columns_test.dart` (11) | pass | pass | pass |
| `test/re_prof_14_autofit_test.dart` (1) | pass | pass | pass |
| `test/re_prof_14_drag_enabled_test.dart` (1) | pass¹ | pass | pass |
| `test/re_prof_14_drag_disabled_test.dart` (1) | pass | pass | pass |
| `test/t05_profiles_ui_test.dart` (1) | pass | pass | pass² |
| `test/profiles_filter_test.dart` (1) | pass | pass | pass² |
| `test/table_actions_test.dart` (6) | pass | pass | pass |
| `test/ux_space01_column_persistence_test.dart` (1) | pass | pass | pass |

¹ ch1 batch run hit one `did not complete` tester crash on
`re_prof_14_drag_enabled_test.dart`; solo retry passed with no code change.
² ch3 batch run hit `did not complete` on `t05` and `profiles_filter`;
isolated retries passed with no code change (t05 needed two attempts).
Pattern matches a flaky tester under back-to-back runs, not a code
regression: both files passed in the ch1/ch2 rounds and again after retry
with identical code.

Final state: `dart format --output=none --set-exit-if-changed
lib/features/profiles/profiles_table.dart` → `0 changed`;
`flutter analyze` → `No issues found!`. No cargo, no release build, no
whole-suite run, per instructions. `git status` shows only the one intended
file modified; nothing committed.

## Expected frame-cost effect (for the integrator's armed run)

- Scroll frames with stable columns/overlay/theme: per-row span build →
  map hit; per-cell inherited lookups → ~zero. Remaining per-frame build is
  widget construction for visible cells only (unchanged count), minus the
  eliminated lookups and span allocations.
- Overlay-tick frames (150 ms poll): unchanged rows hit the span cache;
  `RepaintBoundary` additionally isolates their repaint.
- Nothing here changes dataset-size scaling of `visible`/sort/filter
  (already O(visible) via virtualization); the win is the constant factor
  per visible row/cell.

## Left / notes

- If the armed measurement regresses raster, revert change 2 first (one hunk).
- Candidate (4) from the analysis (`ValueNotifier`-driven span cache keyed
  on visible-range change) was not needed and not attempted.
- `dart format --output=none` is check-only (does not write); the write pass
  is `dart format <file>`, followed by the check pass expecting `0 changed`.

## Armed measurement after optimization (2026-10-08, commit 0ce933b)

Armed evidence build `38f6e2c1` (git_dirty=false, smoke_armed=true), same host.

| scenario | baseline 82d374e | optimized 0ce933b | note |
|---|---|---|---|
| startup main->first frame | 159.2ms | 174.4ms | host variance |
| scroll 10k build p50/p95 | 14.8 / 22.9ms | 14.0 / 22.7ms | p50 -0.8ms |
| scroll 10k dropped | 288 (48.9%) | 254 (43.6%) | -5.3pp |
| scroll 10k raster p95 | 2.1ms | 2.0ms | slightly better |
| scroll 50k build p50/p95 | 23.3 / 33.5ms | 23.5 / 38.0ms | p95 +4.5ms (noise / cache-miss overhead, ambiguous) |
| scroll 50k dropped | 583 (99.7%) | 588 (99.8%) | flat |

Conclusion: first optimization pass is a **marginal win at 10k and neutral at
50k**; the 16.7ms/99% budget is still missed. The row-span cache likely misses
on frames where `visualRevision`/`overlayRevision` change; candidate (4) from
the hotspot analysis (notifier-driven cache / build-scope split) or a deeper
cell-build reduction is required next. Kept the changes (raster not worse,
10k dropped improved); a second iteration needs another armed window.
Raw: `gui/gui_scroll_10000_opt0ce.json`, `gui/gui_scroll_50000_opt0ce.json`,
`gui/gui_startup.json`.

## Second pass — row-subtree (cell list) cache (2026-10-08, local-only)

`TableView.builder` has no row widget (only `rowBuilder` → `TableSpan` plus
per-cell `cellBuilder`), so the "row subtree" is cached as the row's fully
wrapped cell list: `_ProfilesTableState._rowCellsCache:
Map<String, List<TableViewCell>>` (each entry holds the
`TableViewCell(RepaintBoundary(...))` widgets for one data row, indexed by
column). New `_buildCachedCell` fronts `_buildCell`: on a hit it returns the
stored widget for the requested column with no `column.display()` call and no
widget/closure allocation; on a miss it builds all `columns.length + 1` cells
for that row once and stores the list. Span cache kept as-is (separate key).
Bounded at 512 rows (entries are heavier than spans: ~15 widgets each;
cleared past the limit) and cleared in `dispose()`.

Key: `index | rowKey(id) | columnRevision | overlayRevision |
cellVisualRevision | selected | isActive | row.hashCode`, where
`cellVisualRevision = hash(span visualRevision, selectedRowFill, activeFill,
activeMarker, numberBadgeFill, dragFeedbackFill, dropTargetBorder,
dragSortEnabled)`. `ProfileSummary` has value equality covering every
displayed field (`mirrors.dart:87-127`) and all 14 `display` functions are
pure in the row (`profiles_models.dart:211-231`), so `row.hashCode` is a valid
content version: same content hits, any edit/delay/speed/traffic change
misses. Selection/active are per-row bools (O(1) `Set.contains` each), so a
single-row selection change rebuilds only that row. Sort spec is not in the
data-row key because data cells never read it.

Reuse-safety audit (all data-row subtrees are immutable widget configs):

- Each cached widget mounts at exactly one (row, column) slot per frame, never
  duplicated within a frame, with stable keys (`cell-<id>-<col>`,
  `handle-<id>`, `drop-<id>`, `active-marker-<id>`); `Draggable`/`DragTarget`/
  `GestureDetector` element state therefore updates in place and hover/drag
  state is preserved, not reset.
- Frozen closures capture only the row snapshot (versioned), the immutable
  scope colors (versioned), and the long-lived controller (cache is per-`State`,
  cleared on dispose; drop callbacks key on stable row ids — a cached
  `Draggable(data: row)` carries an equal-content snapshot whose `id` is
  identical, so `handleDrop` behavior is unchanged).
- The `DragTarget` hover builder still receives its live `candidate` list at
  event time; only the border color is frozen (and versioned).
- One part is deliberately NOT cached: the header row (row 0). Its sort tap
  closes over the per-build `onSortHeader`, which captures that build's
  `ProfilesState` snapshot for the sort scroll anchor — a cached header would
  sort against a stale anchor. Headers and out-of-range rows build fresh every
  frame. Column persistence untouched (no controller/store change; resize or
  visibility change misses via `columnRevision`).

Checks (same targeted set, run individually; `dart format` write then
check-mode `0 changed`; `flutter analyze` `No issues found!`):

| file | result |
|---|---|
| `test/repair/wave_b_g15_columns_test.dart` (11) | pass |
| `test/re_prof_14_autofit_test.dart` (1) | pass after 1 retry¹ |
| `test/re_prof_14_drag_enabled_test.dart` (1) | pass after 1 retry¹ |
| `test/re_prof_14_drag_disabled_test.dart` (1) | pass |
| `test/t05_profiles_ui_test.dart` (1) | pass after 1 retry¹ |
| `test/profiles_filter_test.dart` (1) | pass |
| `test/table_actions_test.dart` (6) | pass |
| `test/ux_space01_column_persistence_test.dart` (1) | pass |

¹ Same known flaky tester `did not complete` under back-to-back runs as the
first pass (autofit/drag_enabled/t05); solo retries passed with no code
change. No cargo, no release build, no whole-suite run, nothing committed.

Expected frame-cost effect (for the integrator's armed run): scroll frames
with stable columns/overlay/theme/selection should now skip per-cell
`display()` + `Text`/`Container`/`GestureDetector`/`Tooltip` construction and
closure allocation for unchanged data rows (the dominant remaining build cost
after pass 1); per visible row the work becomes one string key + one map
lookup per cell, plus one `Set.contains` × 2 for the key. New-row frames
(scroll into fresh rows) still build those rows once, then re-hit.
Overlay-tick frames that bump `speedTestGeneration`/`lastAckSeq` miss
everything once and rebuild (same as the span cache). If the armed run shows
no build win, the likely cause is cache misses from `overlayRevision` churn,
not the mechanism — consider keying overlay per-row or splitting the build
scope per candidate (4) next. Revert is two hunks (`cellBuilder` call site +
`_buildCachedCell`/cache field).

## Pass 2 measurement + revert (2026-10-08, commit 87bdaa7 reverted)

Armed build with the per-row cell-list cache; same host, same harness.

| scenario | baseline 82d374e | pass 1 (0ce933b) | pass 2 (87bdaa7) |
|---|---|---|---|
| scroll 10k build p50 | 14.8ms | 14.0ms | 15.0ms |
| scroll 10k dropped | 48.9% | 43.6% | 51.1% |
| scroll 50k build p50 | 23.3ms | 23.5ms | 25.5ms |
| raster p95 (10k) | 2.1ms | 2.0ms | 2.06ms |

Pass 2 is neutral-to-worse: the cache key (row + revisions + selection) is
recomputed per cell and likely misses on overlay-revision churn, so the extra
key/map work is pure overhead. **Reverted to pass 1** (the small 10k win,
raster unchanged). Conclusion: widget-construction caching is NOT the primary
bottleneck; further work needs an actual CPU profile of a scroll frame (e.g.
profile-mode DevTools timeline or `--profile` capture) before more changes,
not more speculative caching. Budget 16.7ms/99% remains unmet and is recorded
as an open SP-31 gap. Raw: `gui/gui_scroll_10000_opt2.json`,
`gui/gui_scroll_50000_opt2.json`. The `dist/evidence-armed` zip currently
contains pass-2 code; the next armed window should rebuild from this revert.
