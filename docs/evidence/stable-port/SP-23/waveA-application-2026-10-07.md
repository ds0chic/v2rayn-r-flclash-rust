# Wave A acceptance — application package (SP-23, 2026-10-07)

Scope: FLD-CFG-023/024/042/057/058/114/115/123/129/130/159 (Wave A rows
assigned to `crates/application` in
`docs/evidence/stable-port/SP-23/acceptance-waves-2026-10-07.md`).
New test file only: `crates/application/tests/wave_a_plan.rs` (11 tests).
No existing test file edited; no FLD doc / ledger CSV / manifest touched.

Command (exact, from workspace root):
`cargo test -p application --locked`
Result: ok — 627 passed, 0 failed (lib 350 + all integration suites,
incl. new `wave_a_plan` 11/11). No other package run, no workspace-wide
run, no release. Nothing introduced needed fixing beyond the new file
itself (first compile already green).

Hard-constraint compliance: synthetic profiles/settings only; every
probed listener port loopback `>= 11808` (`free_port` helper asserts and
skips 10808); every plan body asserts absence of `10808`; no kernel
started (`NullRuntimeClient` / pure plan build only); no OS side effects;
no commit.

## Per-instance results

| id | test(s) | command | result | assertion summary |
|---|---|---|---|---|
| 023 | `wave_a_plan::fld023_entry_core_start_plan_follows_canonical_binding` (new) | `cargo test -p application --locked` | pass | Entry without explicit core builds a core-start plan from canonical settings: default (post-`init_core_type_items`) plans Xray with loopback listen + probed port; rebinding Vless→SingBox moves the same entry's plan to SingBox. No 10808. Existing `r4_13_s06` covers only the codegen-selection level, so the plan-level case is new. |
| 024 | existing `t11_routing_dns::settings_save_preserves_simple_dns_global_fake_ip_and_extra` (:692) + `dns_config_feeds_generation_input` + new `wave_a_plan::fld024_dns_global_fake_ip_reaches_plan_after_reopen` | same | pass | Existing tests covered persist+reopen and plan-mapping from a fresh struct, but NOT save→reopen→plan. New case closes the loop: save `global_fake_ip=false` → independent reopen keeps it → `dns_to_codegen` plan carries `simple.global_fake_ip == Some(false)`. |
| 042 | new `wave_a_plan::fld042_new_port4_lan_second_inbound_on_off` (existing `t18b_runtime_plan.rs:506` sets the flag but asserts only loglevel/auth — on/off assertion was missing) | same | pass | on → xray plan has 2 inbounds (`socks` loopback on probed base + `socks3` LAN second inbound on `0.0.0.0`); off → single inbound, no second entry. All emitted ports `>= 11808`, no 10808. |
| 057/058 | new `wave_a_plan::fld057_058_monitor_flags_reach_consumer` (consumer EXISTS: `AppEngine::monitor_settings` + `StatsService` — gap registration not needed) | same | pass | Persisted `EnableStatistics`/`DisplayRealTimeSpeed` reach the consumer-visible `monitor_settings()` tuple and `StatsService::{is_enabled,display_speed,active}`; reopen roundtrip keeps both; both-off → inactive (no forged zeros). Frozen default `(false,false)` pinned. |
| 114 | existing `t18_settings_chain::settings_reach_generated_config` (codegen-input level) + new `wave_a_plan::fld114_domain_strategy_reaches_xray_plan` (runtime-plan level) | same | pass | Saved `domain_strategy=IPIfNonMatch` reaches the xray runtime plan body as `"domainStrategy":"IPIfNonMatch"`. |
| 115 | new `wave_a_plan::fld115_domain_strategy4singbox_reaches_singbox_plan` | same | pass | Saved `domain_strategy4_singbox=prefer_ipv4` (gated on `IPIfNonMatch` per upstream sing-box parity) reaches the sing-box runtime plan resolve rule as `"strategy":"prefer_ipv4"`. |
| 123 | existing `t18_settings_chain.rs:77` + `:139` (chain covered at codegen-input level — no gap) + new `wave_a_plan::fld123_mux_sbox_protocol_reaches_singbox_plan` (runtime-plan level) | same | pass | Saved `Mux4Sbox.Protocol=smux` reaches the sing-box runtime plan `multiplex.protocol` for a mux-enabled node. |
| 129 | existing `r3_03_native_custom::mihomo_yaml_custom_merges_runtime_rewrites` (G-06 engine wiring, see below) + new `wave_a_plan::fld129_ipv6_rewrite_reaches_mihomo_plan` + `fld129_130_engine_wiring_builds_merged_mihomo_plan` | same | pass | `EnableIPv6` on → merged plan has `ipv6: true`; off → `ipv6: false`; secret stripped, unknown keys kept, no 10808; bad base YAML → `FIELD_FORMAT`. Engine smoke proves the `native_custom` merge branch (read from `SP-24/G-06-engine-wiring.md`) is reachable: mihomo `Custom` entry builds a merged plan (port rewrite + ipv6 + secret strip). |
| 130 | same wiring as 129 + new `wave_a_plan::fld130_mixin_merge_gated_by_switch` | same | pass | `EnableMixinContent` on → synthetic mixin key merged; off → same mixin ignored; bad mixin YAML → `FIELD_FORMAT` (no half-merge). |
| 159 | existing `t11_routing_dns.rs:239/545` (templates + common-wins order; `UseSystemHosts=true` previously only in lib unit tests `dns.rs:472/507/512`) + new `wave_a_plan::fld159_use_system_hosts_reaches_plan_after_reopen` | same | pass | Save `use_system_hosts=true` → reopen keeps it → `merge_hosts` plan branch admits synthetic system hosts while custom hosts still win; `false` → system hosts stay out. |

## Uncovered ids

None of the 11 assigned ids is uncovered at the Wave A plan level.
The following are explicitly NOT claimed (Wave B/C follow-ons per the
cited FLD-CFG docs, out of scope for this window):
real core start/session (023, 114/115, 123, 129/130), real-LAN firewall
observation (042), formal settings/DNS-window click-through (024),
real traffic acquisition + table-column linkage (057/058).

## Files created/changed

- Created: `crates/application/tests/wave_a_plan.rs` (11 tests).
- No production code changed; no existing test file edited; no docs,
  ledger, or manifest edited; no commit.

## Addendum 2026-10-07 — FLD-CFG-093/094 (CoreTypeItem binding)

`crates/application/tests/wave_a_core_binding.rs` closes the two instances the
codegen batch could not cover (the binding lives in
`AppEngine::resolve_target_core`, not in `config_codegen`):

- FLD-CFG-093: canonical `CoreTypeItem[].ConfigType -> CoreType` binding decides
  the resolved core when the row has no explicit core; an explicit node core
  wins over the binding; the binding survives reopen (save -> load).
- FLD-CFG-094: a config type without a binding keeps the upstream `Xray`
  default.

Command: `cargo test -p application --locked --test wave_a_core_binding` ->
ok, 2 passed / 0 failed. Synthetic rows only; no kernel, no OS state.
