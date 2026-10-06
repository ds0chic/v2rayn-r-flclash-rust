# SP-21 Dart adoption: profiles list consumes `query_profiles_page_async` (2026-10-07)

Status: implemented (synthetic-bridge contracts green; real-GUI 100k sampling
stays with SP-31; no commit per task instruction).

## What changed (scope: `profiles/**`, `test/**` only)

- `apps/desktop/lib/features/profiles/profiles_controller.dart`
  - `reloadPaged({pageSize})` now walks pages through
    `BridgePort.queryProfilesPageAsync` (frozen full-store `indexId` filter +
    sort, `requestGeneration` = owner generation). Each awaited page is
    dropped when the pager generation moved on **or** its echoed
    `requestGeneration` mismatches; a `datasetRevision` change under the
    cursor restarts the walk from zero (max 5 restarts, same bound as before).
    Pages publish incrementally (`_baseSummaries` + `_recompute` +
    `pagedLoading: true`) with a `Duration.zero` yield between them; the
    walked DTOs become `state.profiles` directly (no second sync full read).
    No sync-seam fallback: the base synthetic port implements the async entry,
    so none is needed.
  - `selectAllAcrossPages({pageSize})` walks the same async seam with the same
    echo/revision guards, returning the walked ids restricted to the current
    view; cancel/supersede/stale-echo keeps the previous selection.
  - `_pagePager` doc comment updated (async entry consumed, gap closed on the
    Dart side).
- `apps/desktop/test/sp21_paged_load_test.dart` (10 tests)
  - `RecordingAsyncBridge` (full-window async fake mirroring sync slices with
    real cursors + echoed generation; counts `asyncCalls`/`asyncCursors`/
    `asyncGenerations` and controller `querySummaryPage` use) +
    `makeAsyncContainer` helper. All load/select tests run through it.
  - New: `reloadPaged walks through queryProfilesPageAsync` (asyncCalls > 0,
    first cursor 0, `syncPageCalls == 0`, single owner generation, 1200 rows
    no dup/miss).
  - New: `stale-generation async pages are dropped` (wrong-echo fake leaves
    the prior view untouched, flag cleared).
  - New: `async revision change mid-walk restarts without dup or miss`
    (revision bump after page one triggers one restart, still 1200/1200).
  - Existing cancel test now lets the first async page land before
    `cancelProfilePageQuery` (async first-page await vs the old sync first
    page), keeping the "late pages dropped, partial < full" semantics.

## Checks (Windows, pwsh, synthetic only, no 10808)

| Command (in `apps/desktop`) | Result |
|---|---|
| `dart format lib/features/profiles test/sp21_paged_load_test.dart test/sp21_async_page_test.dart` | 1 file changed by formatter, then clean |
| `dart format --output=none --set-exit-if-changed lib test` | 0 changed (432 files) |
| `flutter analyze` | No issues found |
| `flutter test test/sp21_paged_load_test.dart test/sp21_async_page_test.dart` | 14/14 pass (10 paged + 4 pager) |
| Regression, per-file processes | `profiles_filter`, `r4_09` (8), `recheck_r3_prof_controller` (8), `t06a_controller`, `t06a_frb_bridge` (real-DLL round-trip), `profiles_keyboard`, `ux_space01_entries`, `r4_22` (8), `fix10b_profile_order` (5), `context_menu_move` (3), `t10_groups_panel`, `ux_space01_group_flow`, `t15b_speedtest` (7), `recheck05_hidden_selection`, `recheck_r3_prof10_default_selection` (6) + `re_prof_06_scope` (3), `t05_profiles_ui` — all green. `t05_profiles_ui` and `profiles_keyboard` each needed a single-file retry after a `did not complete` (known `flutter_tester` instability, also observed on the unmodified baseline; pass on retry). |

## Not done / not claimed

- Real-GUI 100k timer/interaction sampling stays with SP-31 (release GUI).
- Card status stays `identified` until FRB-backed store + SP-16/SP-31
  measurements complete; this file only evidences the Dart-side adoption.
- Did not touch `crates/**`, `apps/desktop/lib/bridge/**`, other features;
  no commit.
