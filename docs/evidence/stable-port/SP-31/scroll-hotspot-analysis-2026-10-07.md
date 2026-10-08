# SP-31 scroll build hotspot — static analysis (2026-10-07, read-only)

No build, no GUI run in this pass. This document records the read-only code
analysis of the release-GUI scroll gap (build p50/p95 14.8/22.9ms at 10k,
23.3/33.5ms at 50k, dropped 48.9%/99.7% against the 16.7ms budget; raster
p95 ~2ms, see `gui/README.md`) and the exact future heavy-load plan.

## Where the frame time goes (code reading)

`apps/desktop/lib/features/profiles/profiles_table.dart` (~1300 lines):

- `TableView.builder` (two_dimensional_scrollables) with
  `rowBuilder: (index) => _buildRowSpan(index, context)` and a `cellBuilder`
  per cell — rows/cells are lazily built, so the cost is **per visible row per
  build**, not per dataset row.
- `_buildRowSpan` constructs a fresh `TableSpan` (with decorations) for every
  visible row on every build; no per-row cache keyed by row identity/revision
  and no `RepaintBoundary` around rows.
- Cells read overlay/delay state and resolved column layout during build; the
  column-layout resolution added by the G-15 canonical wiring runs per build
  (cheap alone, but it sits in the same hot path).
- The header span and static decorations are rebuilt with the same builder.

Consistent with the measurements: raster is flat (~2ms) while build scales
with rows × per-row widget construction, so the optimization target is
row-span/cell build reuse, not painting.

## Candidate optimizations (to be tried one at a time, measured)

1. Cache the per-row `TableSpan` by `(rowKey, columnRevision, overlayRevision)`
   and reuse it while those revisions are unchanged.
2. Wrap row spans in `RepaintBoundary`; make static cell subtrees `const`.
3. Hoist column-layout/overlay lookups out of `cellBuilder` into a per-build
   precomputed view-model list (built once per frame, not per cell).
4. If (1)-(3) are insufficient: move row span construction behind a
   `ValueNotifier`-driven cache that only invalidates on visible-range change.

Each change must keep the existing fake-bridge/widget tests green (G-15
coverage: `wave_b_g15_columns_test.dart`, `re_prof_14_*`, `t05_profiles_ui`).

## Future heavy-load plan (DO NOT run as a per-change check)

| step | command | load | duration |
|---|---|---|---|
| armed rebuild (only if a hook change is needed) | `tools/release/build_windows.ps1 -SmokeArmed` | full cores | ~4–5 min |
| scroll 10k | `tools/perf/run_t18_release.ps1 -Zip dist/evidence-armed/<zip> -Scenario scroll -Rows 10000 -Steps 600 -Tag <tag>` | 1 core + app | ~20 s |
| scroll 50k | same with `-Rows 50000` | 1 core + app | ~30 s |
| startup | same with `-Scenario startup` | light | ~1 s |

Compare against the baseline in `gui/README.md` (10k build p50/p95
14.8/22.9ms, 50k 23.3/33.5ms). Success criterion: frames <16.7ms >99% at 10k
(the 50k target needs a separate ruling; upstream parity is the fallback
criterion).

## Low-load evidence already available

`tools/perf/sp31_ext_bench.dart` (pure Dart, headless) covers the data layer:
pagewalk 10k p95 4.81ms, overlay 10k p95 0.62ms, snapshot p95 1.31ms — the
data layer is not the bottleneck.
