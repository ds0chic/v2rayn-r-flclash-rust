# SP-28 gap-matrix status axes + correction addendum

Date: 2026-10-07. Docs-only, read-only audit; no cargo/flutter/tests/builds,
no processes launched, no network, no commit. Production code untouched.

Correction layer for `docs/evidence/stable-port/SP-28/` per
`gap-consistency-2026-10-07.md` (INC-01..INC-10 + nits). The GBK-encoded
`SP-28/README.md` and `field-gap-matrix.csv` rows are intentionally NOT
touched; corrections live here. All line refs below were verified by file
reads on 2026-10-07.

## 1. Two status axes (INC-01, INC-09)

INC-01 is definitional, not a data error: the same status vocabulary
(AGENTS.md rule 3) is used on two different axes.

- Matrix axis (`field-gap-matrix.csv:status`, and the `actual_effect_verified`
  state behind `settings-current-180.csv`): **effect-verification state at
  SP-28 audit time**. `identified` here means "consumer effect still open /
  unverified".
- FLD-doc axis (`SP-23/FLD-CFG-<n>.md:3`): **implementation/registration
  state**. `implemented` here means "instance registration + storage/DTO
  complete; effect acceptance still missing" (each doc says this explicitly
  and refuses `verified`).

Hence 33 of 34 matrix rows read `identified` while the matching FLD doc reads
`implemented` (`field-gap-matrix.csv:2-35` vs `SP-23/FLD-CFG-<n>.md:3`);
only FLD-CFG-080 agrees (`identified` both sides) because it has no consumer
at all. This mismatch is expected and honest, not a contradiction.

Rule for reading both together (INC-09 included): a field counts as done
only when the FLD doc is `implemented` AND the matrix carries no open row
for it AND its acceptance gap (G-09 class / named G-code) is closed with real
effect evidence. `implemented` on an owning card never implies `verified`:
G-08 (SP-00), G-10 (SP-03), G-14 (SP-13), G-11/G-16 (SP-15), G-15 (SP-16),
G-12 (SP-17), G-04/05/07 (SP-25), 147/148+L1-001 (SP-27) all sit on
`implemented` cards with known-open acceptance work. Owning-card notes
should carry their open G-codes (SP-24's manifest note already does this).

## 2. Stale README section 2 claims (INC-02, INC-03)

`SP-28/README.md` section 2 (`README.md:18-27`) is stale relative to section 6
(`README.md:98-124`, second batch) in the same file. Corrected numbers:

- Matrix size: **34 FLD rows + 3 L1 rows = 37 rows** (38 CSV lines incl.
  header), not "34" as a total. Section 2 predates the 2026-10-06 L1
  continuation (`row_kind=l1`: SP28-L1-001/002/003, all `identified`).
- "34 rows identified and not registered by SP-23" (`README.md:18`) is stale:
  section 6 registers **18** of exactly those IDs:
  001/002/056/065/066/100/102/103/105/116/117/118/119/131/138/139/140/141.
- SP-23 coverage is now **180/180** (`FLD-CFG-001.md`..`FLD-CFG-180.md`, no
  numbering gaps), not 18/36 as at README sections 1/5 writing time.
- UI-cache group: label says 12 but **13 IDs are enumerated**
  (067/068/069/081/082/083/117/118/119/136/156/157/158).
- Cross-platform group: label says 3 but **2 IDs are enumerated**
  (080/142).
- The section-2 total still reconciles either way
  (labels 2+5+12+4+1+5+2+3 = 34; enumerated 2+5+13+4+1+5+2+2 = 34),
  so these are label/count slips, and the 34 FLD-row list itself is correct.

## 3. Owner rulings

### 3.1 FLD-CFG-080 / FLD-CFG-142 -> SP-32/SP-33 (INC-04; docs fixed)

- `SETTINGS_IMPLEMENTATION_180.csv:owner_work_package` is **SD-18** for both
  080 (related SD-06) and 142 (related SD-10) — matching the SD refs already
  present in both docs (`FLD-CFG-080.md:16`, `FLD-CFG-142.md:15`).
- SD-18 maps to **SP-32/SP-33** in both repo-owned mappings:
  `tools/sp28_readonly_audit.py:24-30` (`SD_TO_SP`, citing
  `INTERFACE_AND_OWNER_MAP.md` section 2) and
  `docs/repair/stable-port-2026-10-06/INTERFACE_AND_OWNER_MAP.md:48`
  ("SD-18 platform-only: SP-32/33's unique OS x arch x flow instances").
  The matrix `suggested_owner` column agrees (`field-gap-matrix.csv:10,30`).
- The docs' `SP-18` label is stale: SP-18 is the node mouse/keyboard card
  (`ACCEPTANCE_MATRIX.md:44`), unrelated to a macOS Dock policy (080,
  `platform_scope=macos`, no consumer) or a macOS/Linux proxy-script runner
  (142, path-validation only, execution consumer absent).
- Ruling: consumer owner **SP-32/SP-33** for both. `FLD-CFG-080.md`
  (lines 5/29/45/54) and `FLD-CFG-142.md` (lines 4/29/51/58) owner references
  aligned to SP-32/SP-33; SD-level refs (SD-18/SD-06/SD-10) kept.

### 3.2 FLD-CFG-133 -> SP-17 (INC-05; doc fixed)

- `SETTINGS_IMPLEMENTATION_180.csv:owner_work_package` for 133 is **SD-13**;
  SD-13 maps to **SP-17** (`INTERFACE_AND_OWNER_MAP.md:43`;
  audit-script `SD_TO_SP`; matrix `field-gap-matrix.csv` sibling rows
  131/132/134 all `SP-17`).
- Siblings agree: FLD-CFG-131 (`SP-17`), FLD-CFG-134 (`SP-17` direction),
  FLD-CFG-135 (`SP-17` direction), container FLD-CFG-016 (leaves split
  SP-24/SP-17), and 131 documents the *same* monitor modules
  (`proxies_view.dart`, `clash_ui_config.dart`) under SP-17.
- The 133 doc's `SP-25` (lines 4/28/48/56) contradicts its own association
  line (`SD-13/SD-14`, `FLD-CFG-133.md:15`); split ownership inside one
  timer-lifecycle family is implausible. Ruling: consumer owner **SP-17**;
  the four SP-25 references in `FLD-CFG-133.md` were aligned to SP-17.
- NOT fixed (out of scope, flagged): FLD-CFG-132 carries the identical
  defect (doc `SP-25` vs CSV SD-13 / matrix SP-17, same monitor modules,
  same association `SD-13/SD-14`), and FLD-CFG-136 doc says `SP-28/monitor`
  vs CSV SD-06 / matrix SP-16. Both need owner confirmation plus the same
  mechanical fix; left untouched pending approval.

### 3.3 Matrix disposition for 133 / 135 / 137 / 149 (INC-06)

These four are **absent from the matrix by rule, not by oversight**. The
matrix is generated by `tools/sp28_readonly_audit.py:40-41`: a row is
emitted only if its id is NOT in `SP23_IDS` AND its
`settings-current-180.csv` status is `identified`. Verified 2026-10-07:

- 133/135/137/149 are `implemented` in `settings-current-180.csv`, hence
  excluded. Their FLD docs are likewise `implemented` — with acceptance
  still explicitly open (133/135: poll-frequency + save-failure rollback,
  G-12 family; 137: dedup-key defect shared with 138-141, G-16 family;
  149: save-failure text shared with 147/148). Absence from the matrix
  does NOT mean done; read them under section 1's rule.
- 129/130 are absent for the other reason: they are IN `SP23_IDS`
  (second-batch registered), even though still `identified`.
- Inclusion rule, stated once: matrix = "named consumer defect open
  (`identified`) in `settings-current-180.csv` at SP-28 audit time, minus
  SP-23-registered `SP23_IDS`", plus the three `row_kind=l1` design gaps.

### 3.4 L1-001..003 pointer rows (INC-07)

The L1 rows have no FLD-CFG doc and no `SETTINGS_IMPLEMENTATION_180.csv`
row by design (`row_kind=l1`). Nearest FLD/CSV neighbours so the 001-180
walk can reach them:

- SP28-L1-001 (update GeoFiles row, owner SP-27) <-> FLD-CFG-147/148/149
  (`CheckUpdateItem`, SD-16 update chain).
- SP28-L1-002 (cert-chain leaf-only, owner SP-03) <-> no FLD/CSV
  counterpart: it concerns a profile-node Cert field, outside the 180
  settings rows (custom/profile flow).
- SP28-L1-003 (mihomo YAML preprocess, owner SP-24) <-> FLD-CFG-129/130
  (G-06 mihomo-custom family, SD-14) and the custom-merge path.
- Tracking decision (L1 stub docs vs explicit exclusion note) still open;
  recorded here only as pointers.

### 3.5 SD-17 (FLD-CFG-143..146) owner mapping (INC-10)

Docs name "SD-17 t16 backup chain" (`FLD-CFG-143.md:4`, same 144/145/146),
which matches `SETTINGS_IMPLEMENTATION_180.csv:owner_work_package` = SD-17
for all four — no doc edit needed. SP mapping, verified:
`INTERFACE_AND_OWNER_MAP.md:47` assigns SD-17 backup/WebDAV to **SP-03**
(independent-path instances; HTTPS via SP-25, cancel/commit via SP-02),
and the audit script agrees (`SD-17 -> SP-03(+SP-25)`). So t16 WebDAV
acceptance sits with SP-03 unless reassigned.

### 3.6 INC-08 note (verified, not fixed — integrator-owned)

`tools/sp28_readonly_audit.py:39` already reads
`assert len(sp23_files) >= 18`; the INC-08 description ("asserts 18 files")
is stale in wording only. The remaining staleness is the count (18 vs 180
SP-23 files now) plus the `SP23_IDS` frozen set, already recorded as
integrator business in `SP-28/README.md:118-120` next to G-08
(`validate_plan.py:22`). Code fix belongs to the integrator; nothing
changed here.

## 4. FLD-doc fixes applied alongside this addendum

- `SP-23/FLD-CFG-011.md:8`: `UNAVAILABL` -> `UNAVAILABLE` (line 42 already
  correct; only line 8 fixed).
- `SP-23/FLD-CFG-016.md:6-8,14,42-44,55`: leaf enumerations completed from
  129/130/131/132 to 129-136 (133/134/135 save-failure-rollback class,
  136 connections-table linkage). Consumer-split lines (`016.md:4,57`,
  SP-24/SP-17) left untouched pending the 136 owner decision in section 3.2.
- `SP-23/FLD-CFG-080.md` (lines 5/29/45/54) and `SP-23/FLD-CFG-142.md`
  (lines 4/29/51/58): SP-18 -> SP-32/SP-33 per section 3.1.
- `SP-23/FLD-CFG-133.md` (lines 4/28/48/56): SP-25 -> SP-17 per section 3.2.
- Untouched as instructed: `field-gap-matrix.csv` rows, GBK `SP-28/README.md`,
  `tools/sp28_readonly_audit.py`, `execution-manifest.json`.

## 5. Still open (not fixed here)

INC-07 L1 tracking docs; INC-09 manifest G-code propagation (except SP-24's
note, already fine); 132/136 owner confirmations (section 3.2); 016 lines
4/57 consumer split; any `SP23_IDS`/audit-script refresh (integrator).
