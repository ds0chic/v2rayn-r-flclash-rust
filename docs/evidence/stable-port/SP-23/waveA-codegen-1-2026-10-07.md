# Wave A codegen emit batch 1 — SP-23 FLD-CFG-025..051, 093/094 (2026-10-07)

Scope: `config_codegen` emit instances from Wave A list only. New file
`crates/config_codegen/tests/wave_a_emit_1.rs` (20 tests, standalone cases;
no existing test file or shared helper edited). Synthetic fixtures only;
ports >= 11808; 10808 guarded by `assert_no_live_port` in every emit test.
Reopen proxy at this layer = `CodegenSettings` serde roundtrip (persistence
reopen belongs to the persistence/domain gate, not this crate).

Command (exact): `cargo test -p config_codegen --locked`
Result: ok. 120 passed, 0 failed (18 targets + doc-tests).
New target `wave_a_emit_1`: 20 passed, 0 failed. Nothing introduced needed fixing.

## Per-instance results (new tests)

| id | test name | result | assertion summary |
|---|---|---|---|
| 026 | `wave_a_026_log_enabled_gates_log_files` | pass | off: xray no access/error, sing-box no output; on: Vaccess_/Verror_/sbox_ paths present; reopen keeps switch |
| 027 | `wave_a_027_loglevel_reaches_both_cores` | pass | debug reaches xray loglevel + sing-box level; default warning maps to warn on sing-box; reopen keeps value |
| 028 | `wave_a_028_def_fingerprint_default_and_override` | pass | empty node + chrome default stamps xray realitySettings + sing-box utls; explicit firefox wins; empty default stamps nothing; reopen keeps default |
| 029 | `wave_a_029_empty_user_agent_injects_nothing` | pass | empty default: xray wsSettings has no headers, sing-box transport has no User-Agent; reopen keeps None |
| 030 | `wave_a_030_send_through_effect_and_empty` | pass | Some reaches xray sendThrough + sing-box inet4_bind_address; None emits neither; reopen keeps value |
| 031 | `wave_a_031_bind_interface_effect_and_empty` | pass | Some reaches xray sockopt.interface + sing-box bind_interface; None emits neither; reopen keeps value |
| 032 | `wave_a_032_fragment_switch_and_reopen` | pass | off emits no finalmask; reopen keeps switch (on-path covered by existing `xray_fragment_passes` / `singbox_tls_reality_fragment_ech` / `sp24_fragment_*`) |
| 033 | `wave_a_033_singbox_final_fragment_rule_both_ways` | pass | on: route-options tls_record_fragment rule present; off: absent; reopen keeps switch (xray on-path covered by existing `xray_global.rs:137-138`) |
| 034 | `wave_a_034_cache_file_off_emits_nothing` | pass | off: no experimental.cache_file; on: enabled=true; reopen keeps switch (on-path covered by existing `singbox_dns_and_experimental`) |
| 035 | `wave_a_035_local_port_reaches_both_cores` | pass | 11821 reaches xray inbounds/0/port + sing-box inbounds/0/listen_port; reopen keeps 11821 (range rejection is application `settings.rs` gate, follow-on) |
| 037 | `wave_a_037_udp_enabled_reaches_xray` | pass | true/false reaches xray settings.udp; reopen keeps value; sing-box inbound carries no udp flag (N/A, documented) |
| 038 | `wave_a_038_sniffing_enabled_both_cores` | pass | off: xray sniffing.enabled=false (stored destOverride kept) + no sing-box sniff rule; on: both present; reopen keeps switch |
| 039 | `wave_a_039_dest_override_list_reaches_xray` | pass | ["tls"] lands verbatim; default ["http","tls"] order preserved; reopen keeps list (sing-box per-inbound destOverride N/A, sniff-only) |
| 040 | `wave_a_040_route_only_reaches_xray` | pass | true/false reaches xray sniffing.routeOnly; reopen keeps value (sing-box routeOnly N/A) |
| 041 | `wave_a_041_allow_lan_conn_listen_both_cores` | pass | on: second inbound listen 0.0.0.0 (xray + sing-box); off: single 127.0.0.1 listener; reopen keeps switch |
| 042 | `wave_a_042_new_port4_lan_off_keeps_single_listener` | pass | allow_lan on + new_port4_lan off: single 0.0.0.0 listener both cores; reopen keeps flag (on-path covered by existing `xray_inbound_ports_lan_log_stat` / `singbox_log_and_inbound_ports`) |
| 045 | `wave_a_045_second_port_off_keeps_single_inbound` | pass | off: single inbound both cores; on: socks2 tag at local+1; reopen keeps flag (on-path covered by existing `xray_global.rs:14` / `singbox_global.rs:80`) |
| 046 | `wave_a_046_047_kcp_mtu_tti_reach_emit` | pass | mtu=1400, tti=40 reach kcpSettings; reopen keeps both (node kcp_mtu override + default fallthrough covered by existing `xray_kcp_header_and_seed_order`) |
| 047 | `wave_a_046_047_kcp_mtu_tti_reach_emit` | pass | same test, tti=40 asserted (existing test only pins default tti=20) |
| 048 | `wave_a_048_049_kcp_capacities_reach_emit` | pass | uplink=15 / downlink=90 reach kcpSettings unswapped; reopen keeps both |
| 049 | `wave_a_048_049_kcp_capacities_reach_emit` | pass | same test, downlink=90 asserted |
| 050 | `wave_a_050_051_kcp_window_reaches_emit` | pass | cwndMultiplier=4 reaches kcpSettings; reopen keeps it (<=0 clamp lives in application projection `codegen.rs:396`, not this crate) |
| 051 | `wave_a_050_051_kcp_window_reaches_emit` | pass | maxSendingWindow=1048576 reaches kcpSettings; reopen keeps it (<=0 backfill lives in domain load defaults, not this crate) |

## Existing-coverage verification (no edit; effect assertion confirmed present)

- 025: `sp24_happy_fragment.rs` covers on+params block (`sp24_happy_on_emits_params`),
  off hides block (freedom + dial), defaults emit empty block. Emit gate covered;
  group reopen belongs to persistence gate (out of scope here).
- 029 positive: `xray_transport_security.rs:24` (chrome UA mapping) + `:73`
  (ws curl UA) + `singbox_transport_security.rs:21` (chrome UA). Missing empty
  negative added above.
- 030/031 positive: `xray_global.rs:188` + `singbox_global.rs:271` assert the
  field effect both cores. Missing None negatives added above.
- 032: `xray_global.rs:131` + `singbox_transport_security.rs:157` (on) +
  `sp24_happy_fragment.rs:163` (params) / `:208` (off, no mask). Covered.
- 033: `singbox_global.rs:271` (on rule) + `xray_global.rs:137-138` (on combo).
  Missing sing-box off added above.
- 034 positive: `singbox_global.rs:157` asserts enabled/path/store_fakeip.
  Missing off added above.
- 042 positive: `xray_global.rs:9` + `singbox_global.rs:80` assert LAN inbound
  on-case. Missing off added above.
- 045 positive: `xray_global.rs:14` + `singbox_global.rs:80` assert second
  inbound on-case. Missing off added above.
- 046/047 partial: `xray_transport_security.rs:160-172` pins node kcp_mtu
  override (1350) + default tti fallthrough (20). Missing settings-level custom
  values added above.

## Uncovered ids with reasons

- 093 (CoreTypeItem[].ConfigType), 094 (CoreTypeItem[].CoreType): NOT coverable
  in `config_codegen` — `src/input.rs` has no core-type binding field and no
  generator reads it (grep: zero `core_type` hits in crate). Effect lives in
  `application` (`engine.rs` `resolve_target_core` + availability check), whose
  allowed modules per FLD-CFG-093/094 are application-side. Needs an
  application-layer test (`cargo test -p application --locked`), out of scope
  for this batch; no emit test written rather than a vacuous one.

## Full package result detail

`cargo test -p config_codegen --locked`: lib 0, diff_semantics 8,
fixtures_samples 5, r4_19_template_merge 6, r4_20_groups 3, singbox_errors 11,
singbox_global 6, singbox_groups 3, singbox_protocols 6,
singbox_template_custom 4, singbox_transport_security 4, sp24_happy_fragment 7,
wave_a_emit_1 20, xray_errors 9, xray_global 4, xray_protocols 5,
xray_routing_dns 6, xray_template_custom 6, xray_transport_security 7,
doc-tests 0. Total 120 passed, 0 failed.
