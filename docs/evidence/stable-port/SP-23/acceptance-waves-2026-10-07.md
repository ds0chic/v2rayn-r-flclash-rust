# Acceptance waves — SP-23 FLD-CFG-001..180 (2026-10-07)

Docs-only split of all 180 ledger instances into three executable waves.
Every row derives from `acceptance-tracking-2026-10-07.csv`
(`required_evidence` / `next_action` / `existing_evidence` / `owner_card`)
plus the cited FLD-CFG doc's 允许修改的模块 / 本次必须通过的命令. Nothing invented:
test paths quoted verbatim are existing references (edge/unit, explicitly NOT
passing acceptance per the ledger §1); entries marked `to-write` are suggestions
grounded in the doc's stated command + owning package, to be written in the
heavy-load window.

Method: read-only. No cargo/flutter/tests/builds, no processes, no network
(10808 untouched), no commit. No existing file modified.

HEAD (not re-executed; per ledger doc): generation `ed22cd9`, reference
baseline `6c60da6` (SP-23 coverage-complete). `STATUS-2026-10-07.md` snapshot
HEAD `6c60da6`; RC candidate `828b785`, armed build `82d374e`.

Sources read verbatim: `SP-23/acceptance-tracking-2026-10-07.csv` (181 lines),
`SP-23/acceptance-tracking-2026-10-07.md` (§§1-7),
`SP-28/gap-consistency-2026-10-07.md` (§§0-4),
`STATUS-2026-10-07.md`, and FLD-CFG sample docs
(001/003/022/024/029/042/056/061/062/063/065/067/079/080/084/088/095/116/137/142/143/147/156/158).

## Classification rules (applied uniformly)

- Wave C: ledger `required_evidence` is `OS authorization`,
  `macOS real-device Dock (Windows N/A)`, `macOS/Linux real script exec
  (Windows N/A)`, `E2E apply (signed install + rollback)`, or
  `feasibility + release observation` (062: needs real GPU/renderer + release
  experiment per FLD-CFG-062). Rationale: needs explicit OS authorization,
  real hardware/GPU, non-Windows device, or real release infra.
- Wave B: ledger `required_evidence` is `UI->FRB->save->reopen->effect`
  (32, incl. 036) or `+ ZIP remap roundtrip` (001/002), or
  `real-protocol WebDAV E2E` (143-146: needs backup window + loopback WebDAV
  harness + SD-17→SP mapping per INC-10), or `E2E apply` whose doc gate
  explicitly requires a GUI surface first (entry editor 052-055; tray
  observation 061; logger consumer + formal entry 063; shared HttpClient +
  formal plan 064; convert/subscription window 084; download window 085;
  srs producer 086; async fetch API 087; save-failure UI + poll/rollback
  131-135). Rationale: needs GUI click-through or an armed evidence hook.
- Wave A: everything else — `group 5-state retain + patch isolation` /
  `group retain roundtrip` / `legacy migration + reopen roundtrip` and the
  remaining `E2E apply` rows whose effect asserts at the data layer
  (codegen emit, runtime plan, persistence roundtrip, DNS plan, entry→core-start
  harness on loopback ports ≥11808). No OS writes, no GUI clicks, no real
  network. Real-session / formal-window follow-ons are noted but not required
  for the Wave A gate.

Counts: Wave A 97 + Wave B 54 + Wave C 29 = 180. Zero unclassified (borderlines §5).

## Wave A — testable now, no OS side effects, no GUI clicks (97)

Run commands below are the docs' own `本次必须通过的命令` halves that avoid
GUI/OS (`cargo test -p <pkg> --locked` on loopback-safe harnesses, ports
≥11808 pre-probed per AGENTS.md). `existing:` = ledger `existing_evidence`
verbatim. `to-write:` = suggested new test in the doc's owning package.

| id | owner | ledger next_action | test entry to write/run | effect assertion |
|---|---|---|---|---|
| FLD-CFG-003 | SP-24 | run group 5-state retain E2E | existing: `persistence/tests/edge_cases.rs:92`; run `cargo test -p persistence --locked` | CoreBasicItem group patch roundtrip; unknown keys kept; other groups untouched; reopen consistent |
| FLD-CFG-004 | SP-24 | run group 5-state retain E2E | existing: `persistence/tests/edge_cases.rs:93`; run `cargo test -p persistence --locked` | group 5-state retain + patch isolation + reopen (container 004 scope) |
| FLD-CFG-005 | SP-24 | run group 5-state retain E2E | existing: `persistence/tests/edge_cases.rs:94`; run `cargo test -p persistence --locked` | group 5-state retain + patch isolation + reopen (container 005 scope) |
| FLD-CFG-006 | SP-24 | run group 5-state retain E2E | existing: `persistence/tests/edge_cases.rs:95`; run `cargo test -p persistence --locked` | group 5-state retain + patch isolation + reopen (container 006 scope) |
| FLD-CFG-007 | SP-24 | run group 5-state retain E2E | to-write: `persistence/tests/edge_cases.rs` sibling 5-state case (G-14 family per ledger); run `cargo test -p persistence --locked` | group 5-state retain + patch isolation + reopen (container 007 scope) |
| FLD-CFG-008 | unclear | confirm split ownership, then group E2E | to-write: same as 007 after owner ruling (SP-15/32 vs SP-13 vs SP-24/25/26 vs SD-07 per ledger §4); run `cargo test -p persistence --locked` | ownership confirmed, then group retain + isolation holds |
| FLD-CFG-009 | SP-17 | run group 5-state retain E2E | to-write: `persistence/tests/edge_cases.rs` sibling case; run `cargo test -p persistence --locked` | group 5-state retain + patch isolation + reopen |
| FLD-CFG-010 | unclear | confirm split ownership, then group E2E | to-write: same as 009 after owner ruling (split SP-16 / SD-06+ per ledger §4) | ownership confirmed, then group retain + isolation holds |
| FLD-CFG-011 | SP-25 | run group 5-state retain E2E | to-write: `persistence/tests/edge_cases.rs` sibling case (G-05/G-07 family); run `cargo test -p persistence --locked` | group retain + isolation; SubConvertUrl/086/087 leaves unaffected |
| FLD-CFG-012 | SP-17 | run group 5-state retain E2E | to-write: `persistence/tests/edge_cases.rs` sibling case | group 5-state retain + patch isolation + reopen |
| FLD-CFG-013 | SP-24 | run group 5-state retain E2E | to-write: `persistence/tests/edge_cases.rs` sibling case | group 5-state retain + patch isolation + reopen |
| FLD-CFG-014 | SP-24 | run group 5-state retain E2E | to-write: `persistence/tests/edge_cases.rs` sibling case | group 5-state retain + patch isolation + reopen |
| FLD-CFG-015 | SP-24 | run group 5-state retain E2E | to-write: `persistence/tests/edge_cases.rs` sibling case | group 5-state retain + patch isolation + reopen |
| FLD-CFG-016 | unclear | confirm split ownership, then group E2E | to-write: same as 013 after owner ruling (split SP-24 / SP-17 per ledger §4) | ownership confirmed, then group retain + isolation holds |
| FLD-CFG-017 | SP-15/SP-32 | run group 5-state retain E2E | to-write: `persistence/tests/edge_cases.rs` sibling case (G-16 family) | group 5-state retain + patch isolation + reopen |
| FLD-CFG-018 | SP-03 | run group 5-state retain E2E | to-write: `persistence/tests/edge_cases.rs` sibling case (G-05 family) | group 5-state retain + patch isolation + reopen |
| FLD-CFG-019 | updater方向 | run group 5-state retain E2E | to-write: `persistence/tests/edge_cases.rs` sibling case | group 5-state retain + patch isolation + reopen |
| FLD-CFG-020 | SP-24 | run group 5-state retain E2E | to-write: `persistence/tests/edge_cases.rs` sibling case (G-02 family) | group 5-state retain + patch isolation + reopen |
| FLD-CFG-021 | SP-24 | run group 5-state retain E2E | to-write: `persistence/tests/edge_cases.rs` group-retain roundtrip case | group retain roundtrip + reopen |
| FLD-CFG-023 | SP-24 | run entry->core-start E2E | to-write: `crates/application` entry→core-start harness test (ports ≥11808, loopback only) | entry revision builds core-start plan; no 10808 touch |
| FLD-CFG-024 | SP-24 | run DNS-window->core-DNS E2E | existing: `application/tests/t11_routing_dns.rs:692`; run `cargo test -p application --locked` | saved tree keeps `SimpleDNSItem.global_fake_ip`; DNS plan block present after reopen (window click is Wave B follow-on) |
| FLD-CFG-025 | SP-24 | run DNS-window->core-DNS E2E | existing: `config_codegen/tests/sp24_happy_fragment.rs`; run `cargo test -p config_codegen --locked` | DNS fragment emits expected core DNS block from persisted group |
| FLD-CFG-026 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case for 026 leaf family; run `cargo test -p config_codegen --locked` | persisted value reaches generated config (plan/emit assert; live session is follow-on) |
| FLD-CFG-027 | SP-24 | run real-session E2E | to-write: same family as 026 | emit contains 027 effect + reopen roundtrip |
| FLD-CFG-028 | SP-24 | run real-session E2E | to-write: same family as 026 | emit contains 028 effect + reopen roundtrip |
| FLD-CFG-029 | SP-24 | run real-session E2E | existing: `config_codegen/tests/xray_transport_security.rs:24` + `config_codegen/tests/singbox_transport_security.rs:21`; run `cargo test -p config_codegen --locked` | DefUserAgent chrome/curl combos land in outbound headers (xray + sing-box); empty default injects nothing |
| FLD-CFG-030 | SP-24 | run real-session E2E | existing: `config_codegen/tests/xray_global.rs:188`; run `cargo test -p config_codegen --locked` | 030 leaf effect present in xray global emit |
| FLD-CFG-031 | SP-24 | run real-session E2E | existing: `config_codegen/tests/singbox_global.rs:271` + `config_codegen/tests/xray_global.rs:188` | 031 leaf effect present in sing-box + xray emit |
| FLD-CFG-032 | SP-24 | run real-session E2E | existing: `config_codegen/tests/xray_global.rs:131` + `config_codegen/tests/singbox_transport_security.rs:157` + `config_codegen/tests/sp24_happy_fragment.rs:163` | 032 leaf effect present across all three emit paths |
| FLD-CFG-033 | SP-24 | run real-session E2E | existing: `config_codegen/tests/singbox_global.rs:271` + `config_codegen/tests/xray_global.rs:137` | 033 leaf effect present in sing-box + xray emit |
| FLD-CFG-034 | SP-24 | run real-session E2E | existing: `config_codegen/tests/singbox_global.rs:157` | 034 leaf effect present in sing-box emit |
| FLD-CFG-035 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case (inbound group family per FLD-CFG-042 doc assoc 035-041) | emit contains 035 inbound effect |
| FLD-CFG-037 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 037 effect + reopen roundtrip |
| FLD-CFG-038 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 038 effect + reopen roundtrip |
| FLD-CFG-039 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 039 effect + reopen roundtrip |
| FLD-CFG-040 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 040 effect + reopen roundtrip |
| FLD-CFG-041 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 041 effect + reopen roundtrip |
| FLD-CFG-042 | SP-24 | run real-session E2E | existing: `config_codegen/tests/xray_global.rs:9` + `config_codegen/tests/singbox_global.rs:76` + `application/tests/t18b_runtime_plan.rs:506`; run `cargo test -p config_codegen --locked` + `cargo test -p application --locked` | NewPort4LAN on→plan contains LAN second inbound; off→absent; loopback ports ≥11808 (real-LAN firewall observation is follow-on) |
| FLD-CFG-045 | SP-24 | run real-session E2E | existing: `config_codegen/tests/xray_global.rs:14` + `config_codegen/tests/singbox_global.rs:80` | 045 leaf effect in xray + sing-box emit |
| FLD-CFG-046 | SP-24 | run real-session E2E | existing: `config_codegen/tests/xray_transport_security.rs:160` + `application/tests/sp01_corrupt_config.rs:117` | 046 transport effect emits; corrupt-config recovery keeps prior group |
| FLD-CFG-047 | SP-24 | run real-session E2E | existing: `config_codegen/tests/xray_transport_security.rs:160` | 047 transport effect emits |
| FLD-CFG-048 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 048 effect + reopen roundtrip |
| FLD-CFG-049 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 049 effect + reopen roundtrip |
| FLD-CFG-050 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 050 effect + reopen roundtrip |
| FLD-CFG-051 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 051 effect + reopen roundtrip |
| FLD-CFG-057 | SP-25 | run real-session E2E | to-write: owning-package emit/plan case per doc 允许修改的模块 (SP-25) | 057 effect reaches plan/emit + reopen |
| FLD-CFG-058 | SP-25 | run real-session E2E | to-write: same as 057 | 058 effect reaches plan/emit + reopen |
| FLD-CFG-093 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 093 effect + reopen roundtrip |
| FLD-CFG-094 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 094 effect + reopen roundtrip |
| FLD-CFG-106 | SP-29 | run real-session E2E | existing: `persistence/tests/edge_cases.rs:101`; run `cargo test -p persistence --locked` | 106 persists + plan/emit reflects value + reopen |
| FLD-CFG-107 | SP-29 | run real-session E2E | to-write: sibling of 106 in owning package | 107 persists + emit reflects value + reopen |
| FLD-CFG-108 | SP-29 | run real-session E2E | to-write: sibling of 106 | 108 persists + emit reflects value + reopen |
| FLD-CFG-109 | SP-29 | run real-session E2E | to-write: sibling of 106 | 109 persists + emit reflects value + reopen |
| FLD-CFG-111 | SP-29 | run real-session E2E | to-write: sibling of 106 | 111 persists + emit reflects value + reopen |
| FLD-CFG-113 | SP-29 | run real-session E2E | to-write: sibling of 106 | 113 persists + emit reflects value + reopen |
| FLD-CFG-114 | SP-30 | run real-session E2E | to-write: owning-package (SP-30) plan/emit case | 114 effect reaches plan/emit + reopen |
| FLD-CFG-115 | SP-30 | run real-session E2E | to-write: same as 114 | 115 effect reaches plan/emit + reopen |
| FLD-CFG-116 | SP-13 | add legacy->IsActive migration, then E2E (G-14) | to-write: `crates/persistence` (or SP-00-ruled migrator) `legacy_routing_index_id_to_is_active` roundtrip test; run `cargo test -p persistence --locked` | legacy RoutingIndexId migrates one-way to IsActive; unknown ID falls back to default; ZIP/load roundtrip stable (route-page click is Wave B follow-on) |
| FLD-CFG-120 | SP-24 | run real-session E2E | existing: `config_codegen/tests/xray_protocols.rs:66` + `application/tests/t18_settings_chain.rs:76` + `persistence/tests/edge_cases.rs:102` | 120 protocol effect emits + settings chain + persist roundtrip |
| FLD-CFG-121 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 121 effect + reopen roundtrip |
| FLD-CFG-122 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 122 effect + reopen roundtrip |
| FLD-CFG-123 | SP-24 | run real-session E2E | existing: `application/tests/t18_settings_chain.rs:77` | 123 rides settings chain into plan |
| FLD-CFG-124 | SP-24 | run real-session E2E | existing: `persistence/tests/edge_cases.rs:103` | 124 persists + plan reflects value + reopen |
| FLD-CFG-125 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 125 effect + reopen roundtrip |
| FLD-CFG-126 | SP-24 | run real-session E2E | existing: `persistence/tests/edge_cases.rs:104` | 126 persists + plan reflects value + reopen |
| FLD-CFG-127 | SP-24 | run real-session E2E | existing: `persistence/tests/edge_cases.rs:104` | 127 persists + plan reflects value + reopen |
| FLD-CFG-128 | SP-24 | run real-session E2E | existing: `persistence/tests/edge_cases.rs:104` | 128 persists + plan reflects value + reopen |
| FLD-CFG-129 | SP-24 | wire custom-plan merge (G-06), then E2E | to-write: `crates/application` custom-plan merge unit test (G-06); run `cargo test -p application --locked` | custom plan merges without clobbering sibling keys; merged plan emits |
| FLD-CFG-130 | SP-24 | wire custom-plan merge (G-06), then E2E | to-write: same as 129 | same as 129 for 130 scope |
| FLD-CFG-150 | SP-24 | unify TryParseMaxSplit (G-02), then E2E | existing: `config_codegen/tests/xray_global.rs:140`; to-write G-02 unify case | single TryParseMaxSplit semantic; 150 parses + emits uniformly |
| FLD-CFG-151 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 151 effect + reopen roundtrip |
| FLD-CFG-152 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 152 effect + reopen roundtrip |
| FLD-CFG-153 | SP-24 | unify TryParseMaxSplit (G-02), then E2E | to-write: G-02 unify case (CP-SET-06); run `cargo test -p config_codegen --locked` | 153 parses under unified semantic + emits |
| FLD-CFG-154 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 154 effect + reopen roundtrip |
| FLD-CFG-155 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 155 effect + reopen roundtrip |
| FLD-CFG-159 | SP-24 | run real-session E2E | existing: `application/tests/t11_routing_dns.rs:239` | 159 routing/DNS leaf reaches plan |
| FLD-CFG-160 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 160 effect + reopen roundtrip |
| FLD-CFG-161 | SP-24 | run real-session E2E | existing: `config_codegen/tests/xray_routing_dns.rs:15` + `config_codegen/tests/fixtures_samples.rs:95` + `persistence/tests/edge_cases.rs:113` | 161 routing/DNS emits + fixture sample + persist roundtrip |
| FLD-CFG-162 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 162 effect + reopen roundtrip |
| FLD-CFG-163 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 163 effect + reopen roundtrip |
| FLD-CFG-164 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 164 effect + reopen roundtrip |
| FLD-CFG-165 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 165 effect + reopen roundtrip |
| FLD-CFG-166 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 166 effect + reopen roundtrip |
| FLD-CFG-167 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 167 effect + reopen roundtrip |
| FLD-CFG-168 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 168 effect + reopen roundtrip |
| FLD-CFG-169 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 169 effect + reopen roundtrip |
| FLD-CFG-170 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 170 effect + reopen roundtrip |
| FLD-CFG-171 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 171 effect + reopen roundtrip |
| FLD-CFG-172 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 172 effect + reopen roundtrip |
| FLD-CFG-173 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 173 effect + reopen roundtrip |
| FLD-CFG-174 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 174 effect + reopen roundtrip |
| FLD-CFG-175 | SP-24 | run real-session E2E | to-write: `config_codegen/tests/` emit case | emit contains 175 effect + reopen roundtrip |
| FLD-CFG-176 | SP-24 | fix G-01 gate, then real-session E2E | to-write: G-01 gate unit case (CP-SET-05); run `cargo test -p config_codegen --locked` | gate fixed; 176 passes gate + emits |
| FLD-CFG-177 | SP-24 | fix G-01 gate, then real-session E2E | existing: `config_codegen/tests/xray_routing_dns.rs:247`; to-write G-01 gate case | gate fixed; 177 passes gate + emits |
| FLD-CFG-178 | SP-24 | fix G-01 gate, then real-session E2E | to-write: G-01 gate unit case | gate fixed; 178 passes gate + emits |
| FLD-CFG-179 | SP-24 | fix G-01 gate, then real-session E2E | to-write: G-01 gate unit case | gate fixed; 179 passes gate + emits |
| FLD-CFG-180 | SP-24 | fix G-01 gate, then real-session E2E | to-write: G-01 gate unit case | gate fixed; 180 passes gate + emits |

## Wave B — needs GUI click-through or an armed evidence hook (54)

`hook` = click path or armed hook per the doc's 本次必须通过的命令. `armed`
means the STATUS-2026-10-07 §"needs a decision" hook path (small evidence hook
+ rebuild on an isolated run), not the current docs-only window.

| id | owner | ledger required_evidence / next_action | click-through / hook needed |
|---|---|---|---|
| FLD-CFG-001 | SP-03 | ZIP remap roundtrip / prove IndexId/mirror healing + ZIP roundtrip E2E | node-list select B → save → independent reopen still B; native ZIP import→remap→activate→roundtrip (formal list + import window) |
| FLD-CFG-002 | SP-03 | ZIP remap roundtrip / prove group select/remap roundtrip E2E | group select/remap roundtrip via formal list + ZIP import window |
| FLD-CFG-036 | SP-24 | UI->FRB / add Protocol entry editor, then E2E | add Protocol entry editor first (missing control per ledger), then formal-entry→reopen→effect |
| FLD-CFG-043 | SP-24 | UI->FRB / formal-entry E2E | formal entry→FRB→save→reopen→effect (inbound auth user/pass window) |
| FLD-CFG-044 | SP-24 | UI->FRB / formal-entry E2E | formal entry→FRB→save→reopen→effect (inbound auth window) |
| FLD-CFG-052 | SP-24 | E2E apply / add gRPC entry editor, then real-session E2E | add gRPC entry editor first (existing: `xray_transport_security.rs:249` + `singbox_transport_security.rs:45` cover emit only); then window→save→reopen→real session |
| FLD-CFG-053 | SP-24 | E2E apply / gRPC editor, then E2E | same as 052 |
| FLD-CFG-054 | SP-24 | E2E apply / gRPC editor, then E2E | same as 052 |
| FLD-CFG-055 | SP-24 | E2E apply / gRPC editor, then E2E | same as 052 (no existing emit path cited) |
| FLD-CFG-059 | SP-26 | UI->FRB / formal-entry E2E | formal entry→reopen→effect (SP-26 settings window) |
| FLD-CFG-060 | SP-26 | UI->FRB / formal-entry E2E | formal entry→reopen→effect (SP-26 settings window) |
| FLD-CFG-061 | SP-27 | E2E apply / tray order/switch E2E (existing: `sp01_config_text.rs:23`, `sp01_corrupt_config.rs:27`) | formal window→save→reopen→real native tray: order/current-mark/switch on 0/1/limit/large lists; needs tray observation (armed snapshot or manual) |
| FLD-CFG-063 | SP-25 | E2E apply / logger consumer + entry, then E2E (G-04) (existing: `edge_cases.rs:97`) | add logger consumer + formal entry first; then window→save→restart_app→real log start/stop/rotation/redaction observed |
| FLD-CFG-064 | SP-25 | E2E apply / shared HttpClient via SP-00, then E2E (G-05) | shared HttpClient via SP-00 first; then formal plan→download E2E |
| FLD-CFG-065 | SP-17 | UI->FRB / seed canonical into log view, then E2E (G-12) | seed canonical into log view (G-12); formal log page set filter→save→reopen→initial filter + live/pause consistency |
| FLD-CFG-066 | SP-17 | UI->FRB / seed canonical into log view, then E2E (G-12) | same as 065 (AutoRefresh same page) |
| FLD-CFG-067 | SP-28 | UI->FRB / formal-entry E2E | settings window→save→reopen→real column-width calc (on/off × DPI × manual widths) |
| FLD-CFG-068 | SP-28 | UI->FRB / formal-entry E2E | same window path; splitter/geometry effect observed |
| FLD-CFG-069 | SP-28 | UI->FRB / formal-entry E2E | same window path; column/geometry effect observed |
| FLD-CFG-070 | SP-28 | UI->FRB / formal-entry E2E | formal entry→reopen→effect (monitor/paging surface) |
| FLD-CFG-071 | SP-28 | UI->FRB / formal-entry E2E | formal entry→reopen→effect (monitor surface) |
| FLD-CFG-072 | SP-28 | UI->FRB / formal-entry E2E | formal entry→reopen→effect (monitor surface) |
| FLD-CFG-073 | SP-28 | UI->FRB / formal-entry E2E (existing: `sp01_config_text.rs:31`, `sp01_corrupt_config.rs:26`) | formal entry→reopen→effect on monitor surface |
| FLD-CFG-074 | SP-16 | UI->FRB / formal-entry E2E | formal entry→reopen→effect (main window surface) |
| FLD-CFG-075 | SP-16 | UI->FRB / formal-entry E2E | formal entry→reopen→effect |
| FLD-CFG-076 | SP-16 | UI->FRB / formal-entry E2E | formal entry→reopen→effect |
| FLD-CFG-077 | SP-16 | UI->FRB / formal-entry E2E | formal entry→reopen→effect |
| FLD-CFG-078 | SP-16/SP-18 | UI->FRB / formal-entry E2E | formal entry→reopen→effect (window-lifecycle group) |
| FLD-CFG-079 | SP-16/SP-18 | UI->FRB / Linux close-box E2E; clarify scope | close-box dual path (X hide-vs-quit) + independent-window对照 + tray menu + reopen; FIRST clarify platform_scope linux-vs-Windows (ledger §2) |
| FLD-CFG-081 | SP-28 | UI->FRB / formal-entry E2E | formal entry→reopen→effect (column family with 117-119) |
| FLD-CFG-082 | SP-28 | UI->FRB / formal-entry E2E | formal entry→reopen→effect (WindowSizeItem container family) |
| FLD-CFG-083 | SP-28 | UI->FRB / formal-entry E2E | formal entry→reopen→effect |
| FLD-CFG-084 | SP-26 | E2E apply / prove convert->parse->replace E2E (existing: `edge_cases.rs:100`) | formal window→save→synthetic convert service→real parse→transactional replace→reopen; error/cancel keep old group |
| FLD-CFG-085 | SP-25 | E2E apply / wire download into formal plan, then E2E | wire download into formal plan first; then window→save→reopen→effect |
| FLD-CFG-086 | SP-25 | E2E apply / add local_srs_files producer, then E2E (G-07) (existing: `singbox_global.rs:254`) | add producer first; then window→plan→reopen→effect |
| FLD-CFG-087 | SP-25 | E2E apply / new async fetch API via SP-00, then E2E | new async fetch API via SP-00 first; then E2E |
| FLD-CFG-110 | SP-29 | UI->FRB / formal-entry E2E | formal entry→reopen→effect (SP-29 window) |
| FLD-CFG-112 | SP-29 | UI->FRB / formal-entry E2E | formal entry→reopen→effect (SP-29 window) |
| FLD-CFG-117 | SP-16 | UI->FRB / wire canonical to table, then E2E (G-15) | wire canonical to table first (G-15); then formal entry→reopen→table effect |
| FLD-CFG-118 | SP-16 | UI->FRB / wire canonical to table, then E2E (G-15) | same as 117 |
| FLD-CFG-119 | SP-16 | UI->FRB / wire canonical to table, then E2E (G-15) | same as 117 |
| FLD-CFG-131 | SP-17 | E2E apply / wire save-failure rollback, then E2E | wire save-failure rollback first; then save-failure UI + reopen→rollback observed |
| FLD-CFG-132 | SP-17 | E2E apply / save-failure rollback + poll E2E | same as 131 + poll observation |
| FLD-CFG-133 | SP-17 | E2E apply / poll + rollback E2E | poll + rollback observed (owner SP-17 per ledger §4 ruling note) |
| FLD-CFG-134 | SP-17 | E2E apply / wire save-failure rollback, then E2E | same as 131 |
| FLD-CFG-135 | SP-17 | E2E apply / frequency + rollback E2E | frequency + rollback observed |
| FLD-CFG-136 | SP-16 | UI->FRB / formal-entry E2E | formal entry→reopen→effect (owner SP-16 per ledger §4 ruling note) |
| FLD-CFG-143 | SD-17 t16备份链路 | real-protocol WebDAV E2E / map SD-17/t16 to SP card | FIRST map SD-17/t16→SP card (INC-10); then backup-settings window→save→synthetic-local WebDAV PROPFIND/PUT/GET→bundle roundtrip→reopen |
| FLD-CFG-144 | SD-17 t16备份链路 | same as 143 | same as 143 (auth/failure/retry/interrupt matrix) |
| FLD-CFG-145 | SD-17 t16备份链路 | same as 143 | same as 143 |
| FLD-CFG-146 | SD-17 t16备份链路 | same as 143 | same as 143 |
| FLD-CFG-156 | SP-28 | UI->FRB / formal-entry E2E | real multi-window open→TypeName locate→geometry restore observed |
| FLD-CFG-157 | SP-28 | UI->FRB / formal-entry E2E | real window geometry (Width) restore observed |
| FLD-CFG-158 | SD-06 UI/持久化方向 | UI->FRB / formal-entry E2E | formal entry→save→reopen→same-TypeName window Height restore observed |

## Wave C — needs explicit OS authorization or hardware (29)

All runs on an authorized isolated machine/VM/device only; never the daily
driver. Host proxy (WinINET/registry) and 10808 stay untouched per AGENTS.md.

| id | owner | ledger required_evidence / next_action | authorization / hardware needed |
|---|---|---|---|
| FLD-CFG-022 | 桌面集成/热键链路 | OS authorization / isolated-OS hotkey register E2E | isolated-OS real hotkey register→keypress→dispatch→re-register after restore |
| FLD-CFG-056 | SP-15/SP-32 | OS authorization / Run-key fact + admin-path E2E | isolated machine Run-key/task fact read-back on check/uncheck; failure retry; admin/no-permission path |
| FLD-CFG-062 | SP-26 | feasibility + release observation / SP-26 feasibility first (G-03) | ADR first: engine/embedder source evidence + minimal release experiment on real GPU (renderer/frame-time observed); or mark blocked with extension path |
| FLD-CFG-080 | SP-32/SP-33 | macOS real-device Dock (Windows N/A) / macOS-device Dock E2E | macOS device only (Windows N/A, zero side effects to confirm); add activation-policy reader (consumer absent per doc) then Dock show/hide + restart restore |
| FLD-CFG-088 | SP-15 | OS authorization / key-combo register E2E | isolated-OS native hotkey register per Alt/Control/Shift/KeyCode combo + conflict/retry; timing (next_launch vs immediate)核定 per doc |
| FLD-CFG-089 | SP-15 | OS authorization / key-combo register E2E | same as 088 (same entry combo family) |
| FLD-CFG-090 | SP-15 | OS authorization / key-combo register E2E | same as 088 |
| FLD-CFG-091 | SP-15 | OS authorization / key-combo register E2E | same as 088 |
| FLD-CFG-092 | SP-15 | OS authorization / key-combo register E2E | same as 088 |
| FLD-CFG-095 | SP-24 | OS authorization / isolated-VM real TUN session E2E | isolated-VM real TUN session/route-takeover/stop-cleanup (EnableTun master gate) |
| FLD-CFG-096 | SP-24 | OS authorization / isolated-VM real TUN session E2E | same TUN session rig (096 group scope) |
| FLD-CFG-097 | SP-24 | OS authorization / isolated-VM real TUN session E2E | same TUN session rig |
| FLD-CFG-098 | SP-24 | OS authorization / isolated-VM real TUN session E2E | same TUN session rig |
| FLD-CFG-099 | SP-24 | OS authorization / isolated-VM real TUN session E2E | same TUN session rig |
| FLD-CFG-100 | SP-24 | OS authorization / isolated-VM real TUN session E2E | same TUN session rig (G-13 family per ledger) |
| FLD-CFG-101 | SP-24 | OS authorization / isolated-VM real TUN session E2E | same TUN session rig |
| FLD-CFG-102 | SP-24 | OS authorization / isolated-VM real TUN session E2E | same TUN session rig (G-13 family) |
| FLD-CFG-103 | SP-24 | OS authorization / isolated-VM real TUN session E2E | same TUN session rig (G-13 family) |
| FLD-CFG-104 | SP-24 | OS authorization / isolated-VM real TUN session E2E | same TUN session rig |
| FLD-CFG-105 | SP-24 | OS authorization / isolated-VM real TUN session E2E | same TUN session rig (G-13 family) |
| FLD-CFG-137 | SP-15 | OS authorization / dedup-key fix + isolated-OS E2E (existing: `t13_platform.rs:342`, `edge_cases.rs:106`) | fix dedup key (add content/PAC hash, +141 fallback) then isolated-OS real proxy/PAC query + restore-ownership |
| FLD-CFG-138 | SP-15/SP-32 | OS authorization / dedup-key fix + isolated-OS E2E | same dedup fix + isolated-OS E2E (138 scope) |
| FLD-CFG-139 | SP-15/SP-32 | OS authorization / dedup-key fix + isolated-OS E2E | same dedup fix + isolated-OS E2E |
| FLD-CFG-140 | SP-15/SP-32 | OS authorization / dedup-key fix + isolated-OS E2E | same dedup fix + isolated-OS E2E |
| FLD-CFG-141 | SP-15/SP-32 | OS authorization / dedup-key fix + isolated-OS E2E | same dedup fix + 141-fallback + isolated-OS E2E |
| FLD-CFG-142 | SP-32/SP-33 | macOS/Linux real script exec (Windows N/A) / add exec consumer, then device E2E (existing: `models_and_script.rs:85`, `:60`) | add OSX/Linux script-exec consumer (only path validation exists); then authorized macOS/Linux device real exec (cwd/success/fail/timeout/cancel) + Windows zero-effect |
| FLD-CFG-147 | SP-27 | signed install + rollback / flags + save-failure UI, then signed-install E2E | wire flags + save-failure UI first; then real release repo/pubkey/signature/install/runner/backup-rollback chain (needs release assets) |
| FLD-CFG-148 | SP-27 | signed install + rollback / same as 147 | same release chain (via_proxy scope) |
| FLD-CFG-149 | SP-27 | signed install + rollback / show save failure, then signed-install E2E | same release chain (SelectedCoreTypes scope) |

## Per-wave counts

| wave | n | ids |
|---|---|---|
| A — now, no OS/GUI | 97 | 003-021 (19), 023-035 (13), 037-042 (6), 045-051 (7), 057,058, 093,094, 106-109, 111, 113-116, 120-130, 150-155, 159-180 (22) |
| B — GUI/armed hook | 54 | 001,002, 036, 043,044, 052-055, 059,060, 061, 063-073, 074-079, 081-087, 110, 112, 117-119, 131-136, 143-146, 156-158 |
| C — OS auth/hardware | 29 | 022, 056, 062, 080, 088-092, 095-105, 137-142, 147-149 |
| total | 180 | 97+54+29 = 180; every FLD-CFG-001..180 appears exactly once |

Cross-checks: Wave C 23 OS-authorization rows (022,056,088-092,095-105,137-141)
+ 080 + 142 + 147-149 + 062 = 29. Wave B 32 plain UI + 2 ZIP + 4 WebDAV +
16 E2E-with-GUI-gate (052-055,061,063,064,084-087,131-135) = 54.
Wave A 77 remaining E2E + 19 group + 116 = 97.

## Turnkey prep list (for a future heavy-load window)

Pre-stage only (no execution in this window):

1. Wave A harness pack: new `persistence/tests/edge_cases.rs` sibling cases for
   007-021 (5-state incl. G-14/G-12/G-15/G-16/G-02/G-05/G-07 families) and
   106-115 siblings; new `config_codegen/tests/` emit cases for the ~50
   no-evidence leaves reusing the 029/032/042/120/161 file patterns;
   `application` plan cases for 023/129/130/150/153/176-180 gates; legacy
   migration test slot for 116 (owner SP-13, migrator归属 via SP-00).
   Loopback ports ≥11808 pre-probe helper; 10808 guard assertion in harness.
2. Owner rulings (unblock Wave A/B entries): 008/010/016 split ownership
   (ledger §4); 079 platform_scope linux-vs-Windows; 080/142 SP-18 vs
   SP-32/SP-33 (SP-28 INC-04); 133/135 SP-17 vs SP-25 (INC-05);
   SD-17/t16→SP mapping for 143-146 (INC-10). Short written rulings, no code.
3. Wave B click scripts: formal-entry→save→reopen checklists per window
   (settings core tab, DNS window, log page, subscription/convert, backup,
   update, tray observe, close-box dual path); missing-control build items:
   Protocol editor (036), gRPC editor (052-055), logger consumer + entry
   (063), canonical→table wiring (117-119, G-15), canonical→log seeding
   (065/066, G-12), save-failure rollback UI (131-135,147-149).
4. Wave B armed-hook option: minimal evidence hook design (per STATUS S3/S5+
   path) for table/column effects, splitter/geometry, tray/hotkey UI so
   click-through can be replaced by rebuild + snapshot where approved.
5. Wave C authorization + rigs: isolated VM (TUN 095-105 session/route/cleanup),
   isolated OS user (Run-key 056, hotkeys 022/088-092, sysproxy 137-141 with
   fixed dedup key + PAC-hash), macOS device (080 Dock + activation-policy
   reader), macOS/Linux device (142 script-exec consumer), real GPU machine +
   release experiment slot (062 ADR), release repo/pubkey/assets + signed
   install/rollback rig (147-149), synthetic-local WebDAV server harness
   (143-146, loopback, synthetic data only — never real credentials).
6. Fix-first items before their E2E: G-01 gate (176-180), G-02 unify
   (150/153), G-06 merge (129/130), G-04 logger (063), G-05 HttpClient (064),
   G-07 srs producer (086), fetch API (087), download→plan (085),
   legacy migration (116, G-14), dedup key +141 fallback (137-141, G-16),
   convert→parse→replace (084), tray model already wired (061 observe only).

## §5. Borderlines (classified, flagged — none unclassified)

- 023/024/025 `preserved_only` but plan-assertable → Wave A for the data-layer
  gate; their formal-window halves belong to Wave B follow-on.
- 042 real LAN listen → Wave A (loopback ≥11808 plan+listen assert); host
  firewall/LAN-wide observation is follow-on, not the gate.
- 079 platform_scope=['linux'] → Wave B, scope ruling first (Wave B prep 2).
- 116 route-page click → Wave A covers migration roundtrip; page click is
  Wave B follow-on.
- 143-146 real-protocol WebDAV → Wave B (loopback harness + backup window),
  not Wave C (no OS privilege needed); SP mapping INC-10 first.
- 008/010/016 `unclear` ownership → Wave A by evidence type; owner ruling
  first (prep 2).
- No instance was unclassifiable: every row carries a ledger
  `required_evidence` + doc gate sufficient for the A/B/C rule above.
