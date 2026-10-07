# Per-instance acceptance tracking ledger — SP-23 FLD-CFG-001..180 (2026-10-07)

Scope: acceptance only, not implementation. Every one of the 180 field instances is
registered in `SP-23/FLD-CFG-*.md`, but per `SETTINGS_IMPLEMENTATION_180.csv`
(every row ends in an open acceptance statement) and the SP-28 gap-consistency
report, **no instance has passed formal-entry → reopen → final-effect acceptance**.
This ledger tracks that acceptance debt per instance; it changes no status of any
existing doc, matrix, manifest, or code file.

- Ledger: `docs/evidence/stable-port/SP-23/acceptance-tracking-2026-10-07.csv`
  (181 lines: header + 180 rows, one per FLD-CFG-001..180).
- Method: read-only. No cargo/flutter/tests/builds, no processes, no network, no commit.
- HEAD at generation: `ed22cd9` (current). Reference baseline `6c60da6`
  (SP-23 coverage-complete commit: final 36 docs → 180/180).
- Sources read verbatim: all 180 `SP-23/FLD-CFG-*.md`,
  `docs/repair/stable-port-2026-10-06/SETTINGS_IMPLEMENTATION_180.csv` (180 rows),
  `docs/evidence/stable-port/SP-28/gap-consistency-2026-10-07.md`,
  `docs/evidence/stable-port/SP-28/field-gap-matrix.csv` (37 rows),
  `docs/evidence/stable-port/SP-23/README.md` (G-01..G-16 definitions), `AGENTS.md`.

## 1. Column derivation rules (no invented values)

- `owner_card`: consumer owner stated in the FLD doc task line
  (`消费者归属 ...`), normalized (`SP-17 方向`→`SP-17`, `SP-24 DNS codegen 方向`→`SP-24`,
  composites kept: `SP-15/SP-32`, `SP-16/SP-18`, `SP-32/SP-33`, `SP-28/monitor`;
  non-SP kept raw: `SD-17 t16备份链路`, `SD-06 UI/持久化方向`,
  `桌面集成/热键链路`, `updater方向`). Split-ownership containers → `unclear` (§4).
- `impl_status`: doc `状态` line verbatim (`implemented` / `preserved_only` / `identified`).
- `acceptance_state`: `implemented`→`implemented_unverified` (all 180 carry the
  universal open acceptance gap, SP-28 §0; `verified` count is 0 everywhere),
  `preserved_only`→`preserved_only`, `identified`→`identified`.
  `effect_verified`/`not_applicable` occur 0 times as row states (see note on 080/142 below).
- `open_gaps`: always `G-09` (universal "正式 UI→FRB→保存→重开→最终消费者" evidence gap,
  SP-23 README G-09 + SP-28 §0) plus `G-xx` / `CP-SET-xx` codes literally cited in
  that doc's 缺口 section. G-14 (116), G-15 (117-119), G-12 (065/066), G-16 (137-141)
  appear in `next_action` via README §6.1 + container docs (007/009/010/017), not in
  the leaf docs' own `open_gaps`.
- `required_evidence`: one short phrase from a controlled set, keyed off CSV
  `required_verification` + doc 缺口 theme (counts in §5).
- `existing_evidence`: first-cited `*/tests/*.rs(:line)` path(s) in the doc, else `-`.
  These are referenced related tests (edge/unit), explicitly NOT passing acceptance —
  every doc states its formal scenario is unrun/unverified.
- `next_action`: short action from the doc 缺口 first clause (+ owning card/G-code).

Matrix cross-reference (not duplicated per row): all 37 `field-gap-matrix.csv` rows
carry `CP-SET-01;CP-SET-02;CP-SET-03;CP-SET-15`; per-row `current_state` CP codes are
CP-SET-03/04/07/09/10/12/13/14. SP28-L1-001/002/003 have no FLD rows (SP-28 INC-07).

## 2. Counts per acceptance_state (from the CSV, n=180)

| acceptance_state | n |
|---|---|
| effect_verified | 97 (Wave A plan/emit-level acceptance passed, 2026-10-07) |
| implemented_unverified | 81 (Wave B/C remain) |
| preserved_only | 1 (FLD-CFG-022, Wave C OS hotkey) |
| identified | 1 (FLD-CFG-080, macOS-only) |
| not_applicable | 0 |

Wave A (97) update 2026-10-07: every Wave A instance now has a real passing
test run (see `waveA-persistence-2026-10-07.md`,
`waveA-codegen-1-2026-10-07.md`, `waveA-codegen-2-2026-10-07.md`,
`waveA-application-2026-10-07.md`, plus `wave_a_core_binding.rs` for
FLD-CFG-093/094), so its `acceptance_state` moved to `effect_verified` at the
plan/emit level. `impl_status` is unchanged (174 implemented / 5 preserved_only
/ 1 identified). The GUI/real-core halves listed for some instances in the
Wave B/C sections are NOT covered by this flip.

`not_applicable = 0` as a row state by rule (no doc carries that status).
Platform-N/A aspects are kept inside `required_evidence`/`next_action`:
080 macOS-only (Windows host N/A, macOS consumer absent → `identified`),
142 macos+linux (Windows N/A), 079 `platform_scope=['linux']`.

## 3. Counts per owner_card (from the CSV, n=180)

| owner_card | n | IDs |
|---|---|---|
| SP-24 | 98 | 003-007,013-015,020,021,023-055,059-061,093-105,120-130,150-155,159-180 |
| SP-28 | 12 | 067-073,081-083,156,157 |
| SP-17 | 11 | 009,012,057,058,065,066,131-135 |
| SP-16 | 9 | 010,074-077,117-119,136 |
| SP-29 | 8 | 106-113 |
| SP-32/SP-33 | 7 | 080,088-092,142 |
| SP-15/SP-32 | 6 | 017,056,138-141 |
| SP-25 | 6 | 011,063,064,085,086,087 |
| SD-17 t16备份链路 | 4 | 143-146 (non-SP; needs SP mapping, SP-28 INC-10) |
| SP-03 | 3 | 001,002,018 |
| SP-27 | 3 | 147-149 |
| SP-30 | 2 | 114,115 |
| SP-16/SP-18 | 2 | 078,079 |
| SP-26 | 2 | 062,084 |
| 桌面集成/热键链路 | 1 | 022 (non-SP link) |
| SP-15/SP-32+SP-17+SP-24+SP-25+SP-26 | 1 | 008 (justified composite, §4) |
| SP-15 | 1 | 137 |
| SP-13 | 1 | 116 |
| SD-06 UI/持久化方向 | 1 | 158 (non-SP direction) |
| SP-24+SP-17 | 1 | 016 (justified composite, §4) |
| updater方向 | 1 | 019 (non-SP direction) |

Check: 98+12+11+9+8+7+6+6+4+3+3+2+2+2+1+1+1+1+1+1+1 = 180.

## 4. Ownership rulings (integrator, 2026-10-07; CSV deps authoritative)

All previously `unclear` owner cells are now ruled; the CSV rows and FLD docs
were updated in place:

- FLD-CFG-008 (GuiItem container) — **kept as justified composite**
  `SP-15/SP-32+SP-17+SP-24+SP-25+SP-26`: no leaf majority; leaf SDs are
  056 SD-05→SP-15/32, 057/058 SD-13→SP-17, 059–061 SD-07→SP-24, 062 SD-11→SP-26,
  063/064 SD-12/SD-10→SP-25. The doc's earlier `SP-13` was wrong and was fixed.
- FLD-CFG-010 (UiItem) — **primary `SP-16`** (all true UiItem leaves are
  SD-06→SP-16, G-15 family); exception 080 (SD-18) → SP-32/SP-33.
- FLD-CFG-016 (ClashUIItem container) — **kept as composite `SP-24+SP-17`**
  (129/130 SD-14→SP-24, 131–135 SD-13→SP-17).
- Leaf fixes against CSV deps: 057/058 → `SP-17` (SD-13), 059/060/061 → `SP-24`
  (SD-07), 088-092 → `SP-32/SP-33` (SD-18). Docs and ledger rows updated.
- Earlier rulings kept: 080/142 → `SP-32/SP-33`, 133 → `SP-17`, 132 → `SP-17`,
  136 → `SP-16`.
- Still open: 143-146 owner `SD-17 t16备份链路` is not an SP card (SP-28 INC-10).

## 5. required_evidence phrase counts (from the CSV)

| required_evidence | n |
|---|---|
| E2E apply | 93 |
| UI->FRB->save->reopen->effect | 32 |
| OS authorization | 23 (022,056,088-092,095-105,137-141) |
| group 5-state retain + patch isolation | 18 (containers 003-020) |
| real-protocol WebDAV E2E | 4 (143-146) |
| E2E apply (signed install + rollback) | 3 (147-149) |
| UI->FRB->save->reopen->effect + ZIP remap roundtrip | 2 (001,002) |
| group retain roundtrip | 1 (021) |
| feasibility + release observation | 1 (062) |
| macOS real-device Dock (Windows N/A) | 1 (080) |
| legacy migration + reopen roundtrip | 1 (116) |
| macOS/Linux real script exec (Windows N/A) | 1 (142) |

`existing_evidence`: 37 rows cite a test path (e.g. `persistence/tests/edge_cases.rs:92`,
`config_codegen/tests/xray_global.rs:140`, `platform/tests/models_and_script.rs:85`);
143 rows cite none (`-`). Cited paths are related unit/edge coverage, not acceptance.

## 6. Top-20 next actions grouped by theme (all from the CSV, 39 themes total)

| # | n | next_action (theme) |
|---|---|---|
| 1 | 65 | run real-session E2E |
| 2 | 25 | run formal-entry->reopen->effect E2E |
| 3 | 16 | run group 5-state retain E2E |
| 4 | 11 | run isolated-VM real TUN session E2E |
| 5 | 5 | run isolated-OS key-combo register E2E |
| 6 | 5 | fix dedup key (+141 fallback), then isolated-OS E2E |
| 7 | 5 | fix G-01 gate, then real-session E2E |
| 8 | 4 | add gRPC entry editor, then real-session E2E |
| 9 | 4 | real-protocol WebDAV E2E; map SD-17/t16 to SP card |
| 10 | 3 | confirm split ownership, then group E2E |
| 11 | 3 | wire canonical to table, then E2E (G-15) |
| 12 | 2 | run DNS-window->core-DNS E2E |
| 13 | 2 | seed canonical into log view, then E2E (G-12) |
| 14 | 2 | wire custom-plan merge (G-06), then E2E |
| 15 | 2 | wire save-failure rollback, then E2E |
| 16 | 2 | wire flags + save-failure UI, then signed-install E2E |
| 17 | 2 | unify TryParseMaxSplit (G-02), then E2E |
| 18-20 | 1 each (tie) | 22 singleton themes: prove IndexId/mirror healing + ZIP roundtrip E2E; prove group select/remap roundtrip E2E; run isolated-OS hotkey register E2E; run entry->core-start E2E; add Protocol entry editor, then E2E; isolated-OS Run-key fact + admin-path E2E; run tray order/switch E2E; SP-26 feasibility first (G-03); add logger consumer + entry, then E2E (G-04); shared HttpClient via SP-00, then E2E (G-05); Linux close-box E2E; clarify scope; macOS-device Dock E2E; Windows N/A; prove convert->parse->replace E2E; wire download into formal plan, then E2E; add local_srs_files producer, then E2E (G-07); new async fetch API via SP-00, then E2E; add legacy->IsActive migration, then E2E (G-14); wire save-failure rollback + poll E2E; poll + rollback E2E; frequency + rollback E2E; add OSX/Linux script exec consumer, then device E2E; show save failure, then signed-install E2E |

Theme counts sum to 180 (65+25+16+11+5+5+5+4+4+3+3+2+2+2+2+2+2+22×1).
The top-20 cutoff falls inside the count-1 tie, so all singletons are listed rather
than arbitrarily cut. No `next_action` claims any work done — every theme is an
unrun E2E/prove/fix step owned by the card in `owner_card`.

## 7. What this ledger does not do

- Not implementation tracking: `implemented` here means instance registration
  complete (doc status), acceptance still open for all 180 rows.
- No existing file modified: no FLD-CFG doc, no SP-28 report, no
  `SETTINGS_IMPLEMENTATION_180.csv`, no manifest, no code touched.
  (Note: the worktree already held uncommitted edits to FLD-CFG-011/016/080/133/142
  and an untracked SP-28 note before this task; they were left untouched, and the
  ledger reflects the docs as read.)
- Files created (2): `acceptance-tracking-2026-10-07.csv` + this `.md`, both under
  `docs/evidence/stable-port/SP-23/`.
