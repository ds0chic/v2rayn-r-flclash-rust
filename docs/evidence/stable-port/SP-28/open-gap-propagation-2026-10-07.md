# Open gap propagation — named gaps to owning cards (2026-10-07)

Docs-only, no cargo/flutter/tests/builds, no processes, no network, no commit.

Derived from `gap-consistency-2026-10-07.md` (section 1 owner table, INC-09),
`gap-matrix-status-axes.md` (sections 1, 3.1-3.5), the SP-23 FLD docs
(`消费者归属` lines), `SETTINGS_IMPLEMENTATION_180.csv`
(`owner_work_package`, authoritative at SD level), and
`acceptance-tracking-2026-10-07.csv` (`owner_card` / `open_gaps` columns).
SD-to-SP mapping per `INTERFACE_AND_OWNER_MAP.md` section 2 and
`tools/sp28_readonly_audit.py` (`SD_TO_SP`).

## Container rulings applied in this pass

- FLD-CFG-008 (GuiItem): no single primary consumer. Keep justified composite
  `SP-15/SP-32+SP-17+SP-24+SP-25+SP-26` (CSV SD packages: 056 SD-05, 057/058
  SD-13, 059/060/061 SD-07, 062 SD-11, 063 SD-12, 064 SD-10). The doc's
  `SP-13` label was a typo for SP-17 (SD-13); fixed in FLD-CFG-008.md.
  Leaf-doc effect attributions for 057-061 (SP-25/26/27) diverge from CSV
  SD packages; card-level ruling follows CSV, divergence flagged below.
- FLD-CFG-010 (UiItem): primary consumer SP-16. All true UiItem leaves are
  SD-06 (G-15 family); `SP-16` vs `SD-06` was one owner in two labels, not a
  split. Exception: 080 (SD-18, macOS-only, no consumer) belongs to
  SP-32/SP-33 per the section 3.1 ruling. Doc line fixed accordingly.
- FLD-CFG-016 (ClashUIItem): keep justified composite `SP-24+SP-17`
  (129/130 SD-14, 131-135 SD-13). Exception: 136 (SD-06) points to SP-16 per
  CSV/acceptance/matrix, but its leaf doc says SP-28/monitor (unconfirmed),
  so the container doc lines were left untouched pending confirmation.

## Gap -> owner table

| Gap code | Fields / scope | Owning card(s) | Basis |
|---|---|---|---|
| G-01 | 176 (params 177-180 share the gate) | SP-24 | consistency s1; acceptance 176-180 |
| G-02 | 153 (family 150-155, TryParseMaxSplit) | SP-24 | consistency s1; acceptance 150-155 |
| G-03 | 062 (HWA feasibility) | SP-26 | consistency s1; CSV SD-11; doc SP-26 |
| G-04 | 063 (EnableLog, no logger reader) | SP-25 | consistency s1; CSV SD-12; doc SP-25 |
| G-05 | 064 (cert provider); 087 shares the class | SP-25 | consistency s1; CSV SD-10; doc SP-25 |
| G-06 | 129/130 (mihomo custom merge) | SP-24 | consistency s1; CSV SD-14; docs SP-24 |
| G-07 | 086 (SRS producer) | SP-25 | consistency s1; acceptance 086 |
| G-08 | `validate_plan.py:22` stale all-`identified` assert | SP-00 | status-axes s1/s3.6; integrator-owned |
| G-09 | acceptance-level: formal-entry/final-effect E2E still open on all 180 fields | per-field owner (see acceptance-tracking `owner_card`) | consistency s0; SP-23 manifest note |
| G-10 | 001/002 (canonical IndexId/ZIP roundtrip) | SP-03 | consistency s1 |
| G-11 | 056 (AutoRun) | SP-15/SP-32 | consistency s1; CSV SD-05; doc SP-15/SP-32 |
| G-12 | 065/066/131 (family 132/134; 133/135 same poll/rollback class) | SP-17 | consistency s1; CSV SD-13; 133 fixed to SP-17 |
| G-13 | 100/102/103/105 (TUN rest) | SP-24 | consistency s1 |
| G-14 | 116 (legacy to IsActive migration) | SP-13 | consistency s1; acceptance 116 |
| G-15 | 117/118/119 (family 067/068/069/081/082/083/136/156/157/158, UI cache) | SP-16 | consistency s1; CSV SD-06; docs SP-16 |
| G-16 | 138-141 (family 137, dedup key) | SP-15/SP-32 | consistency s1; CSV SD-05 |
| CP-SET-01 | group 5-state retain + patch isolation (all containers) | SP-23 registry; shared SP-01/02/12 | consistency s1; acceptance containers |
| CP-SET-02 / CP-SET-15 | matrix `cp_code` on all 37 rows | cross-cutting SP-23 / SP-01/02/12 | consistency s1 |
| CP-SET-03 | 063 logger consumer entry | SP-25 | acceptance 063 |
| CP-SET-05 | 176 gate fix | SP-24 | acceptance 176 |
| CP-SET-06 | 153 MaxSplit unify | SP-24 | acceptance 153 |
| CP-SET-07 | 064 shared HttpClient trust source | SP-25 | acceptance 064 |
| CP-SET-08 | 086 local SRS producer; 129 custom-plan merge | SP-25 / SP-24 | acceptance 086 / 129 |
| SD-17 t16 | 143-146 WebDAV real-protocol acceptance | SP-03 | status-axes s3.5 (`INTERFACE_AND_OWNER_MAP.md:47`) |
| SP28-L1-001 | update GeoFiles row (147/148/149 family) | SP-27 | consistency s1; status-axes s3.4 |
| SP28-L1-002 | cert-chain leaf-only (profile-node Cert, outside 180 rows) | SP-03 | status-axes s3.4 |
| SP28-L1-003 | mihomo YAML preprocessing (129/130 family) | SP-24 | consistency s1; status-axes s3.4 |
| (080) | macOS Dock policy, no consumer (`identified`) | SP-32/SP-33 | section 3.1 ruling; CSV SD-18 |
| (142) | proxy-script path validation only, exec consumer absent | SP-32/SP-33 | section 3.1 ruling; CSV SD-18 |
| (088-092) | hotkey OS effect unverified | SP-15 per acceptance; CSV SD-18 -> confirm | consistency s1 (SP-18 family); divergence open |

## Divergences left open (not ruled here)

- FLD-CFG-057/058 leaf docs say SP-25 vs CSV SD-13 (SP-17/22).
- FLD-CFG-059/060 leaf docs say SP-26 and 061 says SP-27 vs CSV SD-07
  (per-ID package; codegen SP-24, others per consumer).
- FLD-CFG-088-092 acceptance says SP-15 vs CSV SD-18.
- FLD-CFG-136 leaf doc says SP-28/monitor vs CSV SD-06 / matrix SP-16.
- Each needs per-leaf consumer confirmation; container-level rulings above
  follow the CSV (authoritative) and are unaffected.

(End of file)
