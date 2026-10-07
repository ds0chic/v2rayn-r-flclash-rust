# Gap consistency report — SP-28 matrix vs SP-23 FLD docs vs SETTINGS_IMPLEMENTATION_180.csv

Date: 2026-10-07. Method: read-only (file reads + content search only).
No cargo/flutter/tests/builds, no processes launched, no network, no commit.
Production code untouched; `work/`/`outputs/`/`compat/` untouched.

Sources (exact revisions read):
- `docs/evidence/stable-port/SP-28/README.md` (121 lines, incl. 2026-10-06 L1 continuation)
- `docs/evidence/stable-port/SP-28/field-gap-matrix.csv` (38 lines: header + 34 FLD rows + 3 L1 rows)
- `docs/evidence/stable-port/SP-23/FLD-CFG-001.md` .. `FLD-CFG-180.md` (180 files, full 001-180 coverage)
- `docs/evidence/stable-port/SP-23/README.md` (144 lines, batch 1 + batch 2)
- `docs/repair/stable-port-2026-10-06/SETTINGS_IMPLEMENTATION_180.csv` (181 lines: header + 180 rows)
- `docs/repair/stable-port-2026-10-06/execution-manifest.json` (SP-03/13/15/16/17/23-28 status+note blocks)
- `docs/evidence/stable-port/SP-28/SP-33-readonly-note.md` (context only, no findings against it)

Status vocabulary used below follows AGENTS.md rule 3
(`identified / implemented / verified / preserved_only / blocked / not_applicable`).

## 0. Headline counts

- SP-23 FLD docs: 180/180 present (001-180, no gaps in numbering).
  Doc self-status: **implemented 174 / preserved_only 5 / identified 1 / blocked 0 / verified 0**.
  - `preserved_only`: FLD-CFG-021, 022, 023, 024, 025 (structure-only containers/groups).
  - `identified`: FLD-CFG-080 only (MacOSShowInDock, no platform consumer).
- field-gap-matrix.csv: **37 rows** = 34 FLD rows (001/002/056/065/066/067/068/069/080/081/082/083/
  100/102/103/105/116/117/118/119/131/132/134/136/138/139/140/141/142/147/148/156/157/158,
  all `identified`) + 3 `row_kind=l1` rows (SP28-L1-001/002/003, all `identified`).
- SETTINGS_IMPLEMENTATION_180.csv: 180 rows (2 internal / 23 container / 155 leaf).
  Every row's `current_gap` ends with an open acceptance statement
  ("本行尚未完成逐字段正式入口/最终效果验收" or a named defect + the same);
  i.e. at acceptance level (G-09 class) **all 180 fields still have an open gap**,
  which is consistent with "verified 0" above and with the SP-23 manifest note
  ("SP-23 docs passing does not mean 180 features pass").

## 1. Fields with open gaps, grouped by owner card

"Open gap" = doc status is `identified`/`blocked`/`preserved_only`,
or the doc/CSV records a named gap (G-xx) or an acceptance-level gap (G-09 class).
Owner = consumer owner stated in the FLD doc (`消费者归属`), falling back to
`field-gap-matrix.csv:suggested_owner` and CSV `owner_work_package` (SD) mapping.
Counts below are CSV-row counts per SD package (verifiable end-to-end); they sum to 180.

| Owner card | SD source (CSV rows) | n | Named gaps in this group |
|---|---|---|---|
| SP-24 | SD-07 (83) + SD-08 (5) + SD-09 (6) + SD-14 (2) | 96 | G-01 (176; params 177-180 share the gate), G-02 (153; family 150-155), G-06 (129/130), G-13 (100/102/103/105), SP28-L1-003 (mihomo YAML preprocess) |
| SP-16 | SD-06 | 23 | G-15 (117/118/119; same family 067/068/069/081/082/083/136/156/157/158) |
| SP-23 structural / SP-01/02/12 cross-cutting | SD-01 containers | 23 | Group-retention acceptance; CP-SET-01/02/03/15 (matrix `cp_code` on all 37 rows) |
| SP-17 | SD-13 | 9 | G-12 (065/066/131; family 132/134; see INC-05 for 133) |
| SP-15/SP-32 | SD-05 (056 + 137-141) | 6 | G-11 (056), G-16 (138-141; family 137) |
| SP-18 family (see INC-04) | SD-18 (080 + 088-092 + 142) | 7 | 080/142 have no consumer (080 `identified`); 088-092 hotkey OS effect unverified |
| SP-25 | SD-10 (064) + SD-12 (063) + SD-15 (085/086/087) | 5 | G-04 (063), G-05 (064; 087 shares the class), G-07 (086) |
| SD-17 t16 chain (see INC-10) | SD-17 (143-146 WebDAV) | 4 | Real-protocol acceptance open; owner is not an SP card |
| SP-27 | SD-16 (147/148/149) | 3 | 147/148 save-failure family; SP28-L1-001 (GeoFiles row) |
| SP-03 | SD-03 (001/002) | 2 | G-10; SP28-L1-002 (cert-chain leaf-only) |
| SP-26 | SD-11 (062) | 1 | G-03 (feasibility card, manifest `identified`) |
| SP-13 | SD-15 (116) | 1 | G-14 (legacy→IsActive migration missing) |

Check: 96+23+23+9+6+7+5+4+3+2+1+1 = 180. OK.

Notes on the table:
- FLD-CFG-084 (SubConvertUrl, CSV SD-07, counted in SP-24's 83) is grouped with
  SP-25 leaves 086/087 in container doc FLD-CFG-011 (`叶子消费者归属 SP-25`);
  its SP attribution is ambiguous — flagged for owner confirmation, count unchanged.
- FLD-CFG-133 doc states owner SP-25 while CSV SD-13 and siblings 131/132/134 point
  to SP-17 — see INC-05. Counted above in SP-17's 9 (CSV basis).
- Owning-card manifest status (2026-10-07 notes): SP-24 `identified`, SP-26 `identified`,
  SP-28 `identified`, SP-32/SP-33 `identified`; SP-03/13/15/16/17/25/27 `implemented`
  (wired + synthetic tests, explicitly not verified). No card is `verified`, so no named
  gap is owned by a truly closed card — but see INC-09 on `implemented` owners.

## 2. Inconsistencies (highest first)

- INC-01 (high) — Same vocabulary, opposite values. 33 of 34 matrix rows say `identified`
  while the matching SP-23 FLD doc says `implemented`; only FLD-CFG-080 agrees
  (`identified` both sides). Affected: 001/002/056/065/066/067/068/069/081/082/083/
  100/102/103/105/116/117/118/119/131/132/134/136/138/139/140/141/142/147/148/156/157/158
  (`field-gap-matrix.csv:2-35` vs `SP-23/FLD-CFG-<n>.md:3`).
  Root cause is definitional (matrix = consumer-effect still open at SP-28 audit time;
  doc = instance registration complete, effect acceptance still missing), but per AGENTS.md
  rule 3 the same status word must mean the same thing across artifacts. Needs a written
  disambiguation, not a silent convention split.
- INC-02 (high) — SP-28 README §2 says the 34 matrix rows are "identified 且未被 SP-23 登记",
  but README §6 (second batch) registers 18 of exactly those IDs
  (001/002/056/065/066/100/102/103/105/116/117/118/119/131/138/139/140/141).
  §2's exclusion claim is stale relative to §6 in the same file
  (`SP-28/README.md:18` vs `SP-28/README.md:98-124`).
- INC-03 (medium) — README §2 arithmetic slips: "UI 缓存 12" but enumerates 13 IDs
  (067/068/069/081/082/083/117/118/119/136/156/157/158); "跨平台 3" but enumerates 2
  (080/142). Total 34 still reconciles (2+5+13+4+1+5+2+2), so this is a label/count slip,
  ref `SP-28/README.md:20-27`.
- INC-04 (medium) — Owner contradiction on cross-platform leaves.
  FLD-CFG-080 doc: "消费者归属 SP-18/SD-06 macOS runner" (`FLD-CFG-080.md:5`);
  matrix: `SP-32/SP-33` (`field-gap-matrix.csv:10`). Same for FLD-CFG-142:
  doc "消费者归属 SP-18/SD-10 平台代理" (`FLD-CFG-142.md:4`) vs matrix `SP-32/SP-33`
  (`field-gap-matrix.csv:30`). SP-18/SD-10 vs SP-32/SP-33 must be picked per field.
- INC-05 (medium) — FLD-CFG-133 doc states "消费者归属 SP-25" (`FLD-CFG-133.md:4`)
  while CSV owner is SD-13 and siblings 131/132/134 (+ container FLD-CFG-016) point to
  SP-17. Either 133 (and possibly 135) belongs to SP-25 while 131/132/134 belong to SP-17
  (split ownership inside one timer-lifecycle family — unlikely), or the 133 doc owner
  line is wrong.
- INC-06 (medium) — Matrix inclusion boundary is undocumented and looks uneven.
  Same-family fields with the same class of open gap are in/out without a stated rule:
  137 in / out vs 138-141 in (G-16 family, same dedup-key defect text in CSV);
  133/135 out vs 131/132/134 in (same ClashUI poll family); 149 out vs 147/148 in
  (same update save-failure text). Either the criterion is "named consumer defect only"
  (then say so in the matrix/README) or 4 rows are missing.
- INC-07 (low) — SP28-L1-001/002/003 (`field-gap-matrix.csv:36-38`) have no FLD-CFG doc
  and no SETTINGS_IMPLEMENTATION_180.csv row (by design, `row_kind=l1`), and no pointer
  from the SP-23 set back to them. A reader walking 001-180 will never find L1-001..003.
  Needs a tracking decision (L1 docs or an explicit exclusion note), not code.
- INC-08 (low) — `tools/sp28_readonly_audit.py:39` asserts 18 SP-23 FLD files; reality is
  180 (36 at the time README §5 was written, 180 now), so the script exits 1.
  Already recorded in `SP-28/README.md:118-120` as integrator business, but still open
  and now 10x staler. Pairs with G-08 (`validate_plan.py:22` stale all-`identified`
  assert, owner SP-00).
- INC-09 (low, structural) — Open gaps owned by `implemented` cards (G-08→SP-00,
  G-10→SP-03, G-14→SP-13, G-11/G-16→SP-15, G-15→SP-16, G-12→SP-17, G-04/05/07→SP-25,
  147/148+L1-001→SP-27). This is legitimate under "implemented ≠ verified", but it means
  every one of those cards carries known-open acceptance work while reading as done in
  summary views. Recommend each owning card's note carry its open G-codes (SP-24's note
  already does this well: "G-07/G-13-rest/TUN-session blocked").
- INC-10 (low) — FLD-CFG-143..146 (SD-17, WebDAV) name their owner as "SD-17 t16 备份链路"
  (`FLD-CFG-143.md:4`), which is not an SP card; no SP mapping for SD-17 was found in the
  artifacts read. Needs a mapping note (which SP owns t16 WebDAV acceptance).
- Nits: `SP-23/FLD-CFG-011.md:8` typo "UNAVAILABL"; container `SP-23/FLD-CFG-016.md:8`
  enumerates only leaves 129/130/131/132 while the group also owns 133/134/135/136.

Explicitly checked, no issue found:
- No `blocked` doc statuses; no `verified` claims anywhere (correct — nothing ran E2E).
- No two artifacts assign the same G-code to different fixes: shared codes (G-01 across
  176-180, G-02 across 150-155, G-12 across 065/066/131, G-06 across 129/130) are
  by-design family references to one shared root cause, each doc pointing at the same
  owning card.
- Matrix `row_kind` for 001/002 (`internal`) matches CSV; matrix `suggested_owner` agrees
  with doc owners except INC-04/INC-05 cases.
- `SP-33-readonly-note.md` claims zero modification of the 3 SP-28 files; directory
  listing is consistent with that claim (no new files beyond the note itself).

## 3. Prioritized next low-load work (documentation/audit only, no builds)

1. Disambiguate INC-01 in writing: one paragraph in `SP-28/README.md` (or a
   `status-note.md` next to `field-gap-matrix.csv`) defining matrix-`identified`
   (consumer effect open) vs doc-`implemented` (registration complete). Unblocks every
   downstream cross-check. Files: `SP-28/README.md`, `SP-28/field-gap-matrix.csv` (header only).
2. Fix INC-02/INC-03 in `SP-28/README.md:18-27`: correct "未被 SP-23 登记" (18 of 34 now
   registered, cite §6) and the 12→13 / 3→2+1 counts. 10-minute doc edit.
3. Rule on INC-04 (080/142 owner): confirm SP-18 vs SP-32/SP-33 per field against the
   frozen upstream platform split (080 macos-only, 142 macos+linux, both Windows-N/A),
   then fix the losing side's owner line. Files: `SP-23/FLD-CFG-080.md:5`,
   `SP-23/FLD-CFG-142.md:4`, `SP-28/field-gap-matrix.csv:10,30`.
4. Rule on INC-05 (133/135 owner): read `SP-23/FLD-CFG-133.md`, `FLD-CFG-135.md`,
   container `FLD-CFG-016.md` against CSV SD-13 rows; one owner line fix either side.
5. Close INC-06: write the matrix inclusion rule down (e.g. "named consumer defect only")
   and either add rows for 133/135/137/149 or record an explicit exclusion rationale.
   Files: `SP-28/field-gap-matrix.csv`, `SP-28/README.md` §2.
6. Close INC-07: add an L1 pointer (3-line section in `SP-23/README.md` or three small
   `SP28-L1-*.md` stubs in SP-28) so the 001-180 walk reaches L1-001..003.
7. File INC-08 as a tracked integrator item next to G-08 (both are stale-assert script
   debt owned by SP-00/integrator): `tools/sp28_readonly_audit.py:39` (18→180),
   `docs/repair/stable-port-2026-10-06/validate_plan.py:22`. Doc-only proposal; code fix
   belongs to the integrator.
8. Close INC-10: one-line SD-17/t16→SP mapping note covering FLD-CFG-143..146
   (check `docs/tasks/` for the t16 owner card first).
9. Propagate INC-09: append open G-codes to each owning card's manifest `note`
   (SP-03: G-10 + L1-002; SP-13: G-14; SP-15: G-11/G-16; SP-16: G-15; SP-17: G-12;
   SP-25: G-04/05/07; SP-27: L1-001), mirroring the SP-24 note style.
   File: `docs/repair/stable-port-2026-10-06/execution-manifest.json` (notes only, keep valid JSON).
10. Nits: fix "UNAVAILABL" (`SP-23/FLD-CFG-011.md:8`); complete leaf enumeration in
    `SP-23/FLD-CFG-016.md:8` (add 133/134/135/136).

## 4. Appendix — matrix row vs doc status (all 37)

| Matrix row | Matrix status | Doc status (`SP-23/FLD-CFG-<n>.md:3`, `080:3-4`) | Match |
|---|---|---|---|
| 001/002 | identified | implemented | NO (INC-01) |
| 056/065/066/067/068/069 | identified | implemented | NO (INC-01) |
| 080 | identified | identified | yes |
| 081/082/083/100/102/103/105 | identified | implemented | NO (INC-01) |
| 116/117/118/119/131/132/134/136 | identified | implemented | NO (INC-01) |
| 138/139/140/141/142/147/148/156/157/158 | identified | implemented | NO (INC-01) |
| SP28-L1-001/002/003 | identified | (no doc — INC-07) | n/a |

Score: 1/34 agree, 33/34 definitional mismatch (INC-01), 3 L1 rows without docs (INC-07).
