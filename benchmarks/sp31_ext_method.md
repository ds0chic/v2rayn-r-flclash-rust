# SP-31 extended harness method §C (synthetic, post-baseline features)

Companion to `sp31_method.md` (§A micro-baseline, §B release-GUI method).
This file documents only the extension tool
`tools/perf/sp31_ext_bench.dart`; §A/§B and `sp31_baseline_*` are unchanged.

## C. Extension layer (landed 2026-10-07, pure Dart, headless)

- Tool: `tools/perf/sp31_ext_bench.dart` (no Flutter/prod imports).
  `dart run tools/perf/sp31_ext_bench.dart --out docs/evidence/stable-port/SP-31`.
- Fixed quantities: seed=20261007; repeats 30 per scenario (10 for
  `pagewalk_100k_ms`, cost-bounded); percentiles nearest-rank
  (`rank=ceil(p/100*n)`), same as §A.
- Scenarios mirror production bounds, not production code paths:
  - `pagewalk_10k_ms` / `pagewalk_100k_ms` (SP-21): freeze filter + stable
    sort (remark, id tiebreak) once per sample, then cursor-walk pages of 500
    with per-page datasetRevision/requestGeneration guards, collecting ids
    (select-all-across-pages shape). Deliberately sorts once (frozen revision)
    rather than the Rust harness's per-page OFFSET re-sort; the two numbers
    are not comparable and neither is a production claim.
  - `overlay_10k_ms` (SP-22): full ID-keyed delay-map overwrite over 10k rows
    + 200-row visible-window merge.
  - `logtail_merge_ms` (SP-22): 10k-line flood coalesced with a naive
    shift-based cap of 2000 + tail merge into a retained 2000 buffer. The
    shift cost is a mirror artifact; production uses ring/bounded coalesce
    (Rust-measured separately in SP-22 evidence).
  - `snapshot_assemble_10k_ms` (SP-17): per-row actual-descriptor view labels
    (frozen target id + generation + core version + redacted exit label).
  - `update_flags_10k_ms` (SP-27): per-operation flag freeze + with-flags vs
    no-arg-default dispatch over 10k ops; zero network by construction.
  - `lease_renew_10k_ms` (SP-09): virtual-time (u64 ms, no sleep/clock)
    renew_due / lease_expired / reconcile_due evaluation for 10k sidecars
    under the 15s / 90s / 3-fail bounds.
- Output: `docs/evidence/stable-port/SP-31/sp31_ext_<UTC>Z.json`
  (p50/p95/p99/min/max per scenario) + `sp31_ext_<UTC>Z_raw.csv` (raw samples).
  Stdout prints a consumption checksum, the scenario table, and written paths.
- Runs in this repo: Dart SDK 3.13.4 on Windows x64 (JIT `dart run`);
  numbers are machine-local mirrors for regression tracking, not release claims.
- Release GUI frames/startup/memory remain §B (blocked on candidate build).
