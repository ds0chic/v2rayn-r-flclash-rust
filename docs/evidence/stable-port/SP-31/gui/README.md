# SP-31 release-GUI measurement (armed evidence build 82d374e)

Run 2026-10-07 on this host (Windows 11 x64, release build). Driver:
`tools/perf/run_t18_release.ps1` extracts the **armed** evidence zip
(`dist/evidence-armed/v2rayN-R-1.0.0+1-windows-x64.zip`, build-info commit
`82d374e`, `smoke_armed=true`, `git_dirty=false`) and launches the packaged exe
with the app's own T18 benchmark hook (`V2RAYN_R_T18_BENCH=1` + scenario/rows);
the app writes every number itself (`SchedulerBinding.addTimingsCallback`).
Raw summaries: `gui_startup.json`, `gui_scroll_10000_cand82d.json`,
`gui_scroll_50000_cand82d.json` (this directory).

The official unarmed RC (`dist/...zip`, commit `828b785`) is unaffected: the
benchmark hook is compiled out of it (ISSUE-08).

## Numbers (release build, 600 scroll steps, 16.7ms budget)

| scenario | samples | build p50/p95 | raster p50/p95 | total p95 | dropped frames | dropped rate |
|---|---|---|---|---|---|---|
| startup | first frame | — | — | main→first frame 159.2ms (wall 506ms incl. process start) | — | — |
| scroll 10k rows | 589 | 14.8 / 22.9ms | 1.7 / 2.1ms | 24.9ms | 288 | 48.9% |
| scroll 50k rows | 585 | 23.3 / 33.5ms | 1.8 / 2.2ms | 35.4ms | 583 | 99.7% |

## Reading

- Raster stays far under budget (p95 ≈ 2ms); the **UI build** of the
  virtualized table is the bottleneck (10k p50 14.8ms, 50k p50 23.3ms).
- Against the SP-31 acceptance wording (frames < 16.7ms at > 99%) the scroll
  scenarios **miss** the budget on this host: dropped 48.9% at 10k and 99.7%
  at 50k.
- This is not a regression against the earlier T18 baseline
  (`benchmarks/T18/scroll_10000.json`, build p95 21.8ms, dropped 55.6%): the
  current build is slightly better, same order.
- The data layer is not the bottleneck (SP-31 prep: filter_10k p95 1.48ms;
  extension bench: pagewalk/overlay/snapshot p95 < 5ms).

## Follow-up (identified, not fixed here)

- 10k/50k scroll build cost: candidate optimizations (const row widgets, key
  strategy, narrower rebuild scope, incremental row diffing) need a dedicated
  perf pass with the same harness as the gate. Recorded as an open SP-31 gap.
- Startup 159ms main→first-frame on this host; no target was breached, no
  claim is made for other hosts.
