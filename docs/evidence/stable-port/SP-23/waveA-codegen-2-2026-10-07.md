# Wave A codegen emit-2 evidence — SP-23 FLD-CFG (2026-10-07)

Scope: `config_codegen` emit coverage for the Wave A instances that had no
emit test: 121, 122, 125, 151, 152, 154, 155, 160, 162–175, 178, 179, 180,
plus the G-02 unify case (150/153, MaxSplit "1-3" parse parity) and the G-01
gate cases (176/177/178/179/180). Existing cited tests for 120, 123, 150,
159, 161, 177 were read to confirm coverage; missing assertions were added
in the new file only.

No existing test file, shared helper, FLD doc, ledger CSV, or manifest was
edited. No production code was changed. No commit. Synthetic fixtures only
(`192.0.2.x`, TEST-UUID-style passwords); ports >= 11808; 10808 never
referenced (guarded by `assert_no_live_port` + a serialize-time check).

## New file

- `crates/config_codegen/tests/wave_a_emit_2.rs` — 33 tests, all passing.
  (Sibling files `wave_a_emit_1.rs`, `wave_a_plan.rs`, `wave_a_groups.rs` and
  their evidence docs are other agents' work; untouched.)

## Command + result

- Exact command: `cargo test -p config_codegen --locked`
  (run from the workspace root; no other package, no workspace-wide run).
- Result: **153 passed, 0 failed** across all 20 targets, including the new
  `wave_a_emit_2` target (**33 passed, 0 failed**). One iteration was needed:
  `fld175` singbox assertion first expected a raw `geoip` key, but the
  generator rewrites geoip into an srs `rule_set` ref (`singbox/ruleset.rs`);
  the test was corrected to assert the real emit (`rule_set ["geoip-cn"]` +
  `route.rule_set` entry). No production change; nothing else touched.

## Per-instance results (id -> test, assertion)

| id | test in `wave_a_emit_2.rs` | assertion summary |
|---|---|---|
| 120 | `fld120_mux_concurrency_custom_and_off` | Existing `xray_protocols.rs:66` covers default-8 on-path only (verified by read). New: concurrency=4 reaches `mux.concurrency`; mux-off emits `{enabled:false, concurrency:-1}`; reopen roundtrip keeps 4. |
| 121 | `fld121_xudp_concurrency_emit_and_reopen` | vless+flow+`mux_enabled` emits `xudpConcurrency=9`; mux-off emits base object with no xudp keys; JSON reopen regenerates 9. Note: codegen `Mux4Ray` struct default for xudp is 8; the frozen 16 default lives at the application projection (`codegen.rs:383 unwrap_or(16)` per FLD doc) — not asserted here. |
| 122 | `fld122_xudp_proxy_udp443_emit_and_reopen` | `"skip"` reaches `xudpProxyUDP443`; TCP-mux path carries no xudp443 key; reopen keeps `"skip"`. |
| 123 | `fld123_mux_sbox_protocol_emit_and_reopen` | Existing `t18_settings_chain.rs:77/139` covers plan-level (verified by read; `-p application` not run per scope). New: sing-box `multiplex.protocol=smux` on-path; mux-off and empty-protocol negatives emit no `multiplex`; reopen keeps smux. |
| 125 | `fld125_mux_sbox_padding_emit_and_reopen` | `padding=true` emitted; `None` omits the key (block still present); reopen keeps true. |
| 150 | `fld150_fragment_packets_leaf_and_reopen` | Existing `xray_global.rs:140` sets only max_split — does NOT cover this leaf (as the FLD doc itself notes). New: custom packets string reaches wire; unset falls back to `tlshello`; reopen keeps custom value. |
| 151 | `fld151_fragment_lengths_leaf_and_reopen` | `lengths=["20-40","60-80"]` + first-value `length="20-40"`; empty falls back to `["50-100"]`; reopen keeps list. |
| 152 | `fld152_fragment_delays_leaf_and_reopen` | `delays=["1-5","6-9"]` + first-value `delay="1-5"`; empty falls back to `["10-20"]`; reopen keeps list. |
| 150/153 G-02 | `fld150_153_maxsplit_parse_parity_and_reopen` | Wire first-int parity (`1-3`->1, `0`->0, `10000`->10000, `0-10000`->0, `" 3 "`->3, `abc`->0) through a reopen each time; raw `"1-3"` preserved verbatim (never stored truncated). Validator unify itself lives in `application/src/settings.rs:93/110` (verified present, G-02 fixed per SP-24 doc) — asserted here only at the wire. |
| 154 | `fld154_legacy_length_target_reaches_wire` | Emit half: post-migration `lengths=["77-88"]` reaches wire + `length` first value. Migration backfill itself is domain-owned (`domain/src/settings.rs:1114-1120`, `cargo test -p domain` per FLD doc) — out of this package's scope, recorded as gap below. |
| 155 | `fld155_legacy_interval_target_reaches_wire` | Same split for `delays=["9-19"]` -> wire + `delay` first value. |
| 159 | `fld159_use_system_hosts_emit` | Existing `t11_routing_dns.rs:239` covers template level only (verified by read). New codegen complement: system host reaches xray `dns.hosts` and sing-box `hosts-dns.predefined`; off -> absent in both. |
| 160 | `fld160_add_common_hosts_emit_and_reopen` | On: predefined `dns.google` set in xray `dns.hosts` and sing-box predefined; off+empty: no `hosts` key (xray), `{}` predefined (sing-box). |
| 161 | `fld161_fakeip_off_absent_both_cores` | Existing `xray_routing_dns.rs:15` + `fixtures_samples.rs:95` + persistence `:113` cover the on-path (verified by read). New complement: `fake_ip=false` emits no `fakedns` (xray) and no `fake-dns` server (sing-box). |
| 162 | `fld162_global_fakeip_branch_and_reopen` | Global default: fakedns server covers proxy+direct domains; sing-box global logical fake rule. `Some(false)`: direct domains excluded (xray), no logical rule but per-rule fake entries (sing-box). Direct-only + non-global: no `fakedns` at all. |
| 163 | `fld163_fakeip_range_pool_and_reopen` | Custom `10.9.0.0/16` -> xray `ipPool` + `poolSize 65535`, sing-box `inet4_range`; unset -> frozen `198.18.0.0/15` default in both. |
| 164 | `fld164_block_binding_query_emit` | Sing-box-only leaf (no xray counterpart branch in-tree): `[64,65]` predefined filter rule on/off; xray still generates cleanly. |
| 165 | `fld165_block_aaaa_query_emit_and_reopen` | xray `queryStrategy=UseIPv4` on/absent off; sing-box `[28]` filter rule on/absent off. |
| 166 | `fld166_direct_dns_value_and_reopen` | Custom `9.9.9.9` reaches xray tagged `direct-dns-1` server and sing-box `direct-dns-1`; unset -> `119.29.29.29` default (sing-box asserted; xray default shares the same `parse_dns_addresses` fallback). |
| 167 | `fld167_remote_dns_value_and_reopen` | Custom `8.8.8.8` in xray servers + sing-box `remote-dns-1` (detour proxy); unset -> cloudflare DoH default in both. |
| 168 | `fld168_bootstrap_dns_value_and_reopen` | Custom `9.9.9.9` reaches sing-box `local-local` and the xray bootstrap server for domain-form DoH remotes; unset -> `119.29.29.29`. |
| 169 | `fld169_strategy4_freedom_emit_and_reopen` | `UseIP` -> freedom `sockopt.domainStrategy`; `None`/`AsIs` -> absent; no happy block without the master switch. |
| 170 | `fld170_strategy4_proxy_emit_and_reopen` | `UseIP` -> proxy `targetStrategy`; `None`/`AsIs` -> absent. |
| 171 | `fld171_strategy4_proxy_dial_emit_and_reopen` | `UseIP` -> xray dial `domainStrategy` + sing-box `default_domain_resolver.strategy=prefer_ipv4`; unset -> absent in both. |
| 172 | `fld172_serve_stale_emit_and_reopen` | xray `serveStale` always written (true/false); sing-box `optimistic` only when on. |
| 173 | `fld173_parallel_query_emit_and_reopen` | xray `enableParallelQuery` only when on; sing-box `race:true` respond fan-out across two synthetic remotes when on, no `race` key when off. |
| 174 | `fld174_custom_hosts_merge_and_reopen` | `example.test 93.184.216.34` merged into xray `dns.hosts` and sing-box predefined; empty -> merge skipped in both. |
| 175 | `fld175_direct_expected_ips_emit_and_reopen` | xray `expectedIPs=["geoip:cn"]` server; sing-box `rule_set ["geoip-cn"]` respond rule + `route.rule_set` entry (geoip is rewritten to an srs ref by `ruleset.rs`); unset -> no `geoip-cn` anywhere. |
| 176 G-01 | `fld176_happy_gate_on_freedom_emits_block` | Gate fix exists in-tree (`xray/dns.rs:383`, SP-24 doc; no production change here). OFF both sites already covered by `sp24_happy_off_*`. New ON complement at freedom site: full params block emitted. |
| 177 G-01 | `fld177_try_delay_single_param_gate` | Existing `xray_routing_dns.rs:247` proves the on-path only (verified by read). New: isolated `tryDelayMs=300` block on, domainStrategy-without-block off. |
| 178 G-01 | `fld178_prioritize_ipv6_single_param_gate` | Isolated `prioritizeIPv6=true` block on, hidden off. |
| 179 G-01 | `fld179_interleave_single_param_gate` | Isolated `interleave=2` block on, hidden off. |
| 180 G-01 | `fld180_max_concurrent_try_single_param_gate` | Isolated `maxConcurrentTry=3` block on, hidden off. |

## Uncovered ids + reasons (no production fix made)

- None of the assigned codegen ids is left without an emit test: every id in
  {121, 122, 125, 151, 152, 154, 155, 160, 162–175, 178, 179, 180} plus G-02
  (150/153) and G-01 (176–180) has a dedicated test above.
- Partial-by-design, recorded as follow-on gaps (not fixed — production code
  untouched per instructions):
  - G-01 (176–180): the gate production fix pre-exists (`xray/dns.rs:383`,
    `sp24_happy_*` green); remaining acceptance is the formal
    DNS-window->FRB->save->restart_core->xray-binary-check->reopen loop
    (Wave B / SP-34 gate).
  - G-02 (150/153): validator unify pre-exists
    (`application/src/settings.rs:93/110`, SP-24 doc); remaining acceptance
    is the formal window save path + `cargo test -p application` MaxSplit
    matrix (different package, not run here).
  - 154/155: legacy Length/Interval backfill is domain-owned
    (`domain/src/settings.rs:1114-1120`); its migration test belongs to
    `cargo test -p domain` (different package, not run here). This file pins
    the emit half (migrated values reach the wire).
  - 159/123/120-plan halves: application-level chain tests (`t11`, `t18`)
    cited from their FLD docs by read-only verification; `-p application`
    was deliberately not executed (package scope constraint).
- Real-session / real-core validation halves in every FLD doc remain open by
  the docs' own completion criteria (formal entry + core binary + reopen);
  data-layer emit + reopen roundtrip is what this file proves.
