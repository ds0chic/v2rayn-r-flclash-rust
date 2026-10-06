# SP-31 continuation 2026-10-07: extended synthetic harness (post-baseline features)

Status: identified (synthetic data-layer coverage extended; release GUI sampling still blocked).
Scope: only `tools/perf/sp31_ext_bench.dart` (new), `benchmarks/sp31_ext_method.md` (new),
this dir's new `sp31_ext_*` JSON+CSV + this note, and the SP-31 manifest note.
No prod Dart/Rust/C++, no FRB, no Cargo lock, no commit; baselines
(`benchmarks/sp31_baseline_*`, `README.md`, `observations.json`) untouched.
Synthetic only; 127.0.0.1:10808 never touched; no OS side effects.

## New file

- `tools/perf/sp31_ext_bench.dart` (pure Dart, no Flutter/prod imports; headless).
  Mirrors the data-shape/contract logic of features landed since baseline 92d46dd
  with the same bounds as production (page 500, 2000-line cap, 15s/90s/3 fails).
  These are Dart-side mirrors, NOT Rust timings; Rust-side numbers stay in
  SP-21/SP-22 evidence. No GUI frame numbers claimed.

## Commands (exit 0 unless noted)

- `dart format tools/perf/sp31_ext_bench.dart` -> formatted 1 file, then
  `dart format --output=none --set-exit-if-changed tools/perf/sp31_ext_bench.dart` -> exit 0.
- `dart run tools/perf/sp31_ext_bench.dart --out docs/evidence/stable-port/SP-31` -> exit 0,
  stdout: checksum + per-scenario p50/p95/p99/min/max + written paths.
- `cargo` gates: not run (no Rust code touched, per task targeted-checks rule).

## Measured numbers (nearest-rank; raw samples in `sp31_ext_2026-10-06T22-36-56Z_raw.csv`)

| scenario | n | p50 | p95 | p99 |
|---|---|---|---|---|
| pagewalk_10k_ms (SP-21: sort + full 500/page cursor walk, guards) | 30 | 2.89 | 4.81 | 8.90 |
| pagewalk_100k_ms (SP-21, same shape x10) | 10 | 43.36 | 51.15 | 51.15 |
| overlay_10k_ms (SP-22: 10k ID-keyed delay merge + 200-row visible merge) | 30 | 0.18 | 0.62 | 3.69 |
| logtail_merge_ms (SP-22 mirror: 10k flood -> 2000 cap + tail merge) | 30 | 15.31 | 42.14 | 42.83 |
| snapshot_assemble_10k_ms (SP-17: 10k-row descriptor view assembly) | 30 | 0.96 | 1.31 | 2.31 |
| update_flags_10k_ms (SP-27: 10k ops freeze+dispatch, no network) | 30 | 0.01 | 0.29 | 0.60 |
| lease_renew_10k_ms (SP-09: 10k sidecars virtual-time renew/expire/reconcile) | 30 | 0.04 | 0.56 | 0.68 |

Threshold read (ACCEPTANCE_MATRIX §4): pagewalk_10k p95 4.81ms << 200ms 10k budget;
overlay/snapshot/flags/lease bookkeeping p95 all < 2ms, no pressure on the 100ms
feedback budget at the data layer. logtail p95 42ms is a mirror artifact (naive
Dart list-shift in the mirror; production uses ring + 2000 cap, Rust-measured
50k flood 7.3ms in SP-22 evidence) — recorded as-is, not a production claim.
100k full-walk shape is harness-only; production callers wait on a bounded
single page (SP-21), so no per-page inference is drawn beyond the 10k pagewalk.

## Cross-reference (Rust-side numbers, already in owner evidence, not re-run)

- SP-21 real-SQLite worker walk: 10k 670ms/20p, 100k 26636ms/200p (debug harness).
- SP-22: 10k overlay 6.7ms (Rust), mergeDelayMap 3ms / sort-10k 10ms (Dart).

## Still blocked on the candidate build (not claimed)

Release GUI acceptance: single release build + synthetic real DB + DevTools
Timeline (cold 20 / warm 30), UI/raster frame times, startup, memory/queue curves.
Blocked because the running dist instance holds the build; owner = integrator.
`git status --short` at run time showed only this card's 3 new files (worktree
otherwise clean); nothing committed.
