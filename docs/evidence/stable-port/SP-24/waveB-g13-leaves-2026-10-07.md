# Wave B G-13 TUN leaves (2026-10-07): FLD-CFG-100/102/103/105 plan/desired-actual closure, application level

Status: implemented (plan/desired-actual contract green at application level;
formal UI->FRB->save->restart_core->real-TUN-session/route/adapter acceptance
unrun, not verified; ledger CSV / FLD docs / manifest / bridge_api /
ipc_contract / services / workspace Cargo / FRB untouched; no commit).

Scope: offline verification/wiring only. NO real TUN session, NO routes, NO
adapter changes, NO OS calls. Real TUN acceptance remains user/external
(unverified). 10808 untouched (no listener, no proxy call in any new test).

Baseline: working tree has parallel in-flight batches; this card edits ONE
production file (`crates/application/src/tun_plan.rs`), adds ONE test file,
and edits no existing tests. `engine.rs` untouched.

Upstream baseline (read-only `work/`): frozen v2rayN 7.25.4
(commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`).

## Per-id status (plan/desired-actual level)

| id | leaf | status | evidence |
|---|---|---|---|
| 100 | `EnableIPv6Address` (bool, default false) | wired (+1 minimal wiring: on-without-address now warns) | `tun_plan.rs:230-238` gates v6 inclusion; new tests `g13_100_*` (3); new warning `tun_ipv6_address_missing` / field `IPv6Address` for on-without-address (was silently IPv4-only) |
| 102 | `EnableLegacyProtect` (bool, default true) | wired, no production change | `engine.rs::pre_socks_of` (`:4842-4886`) writes the pre-socks sidecar topology; TUN-A02 suppresses the duplicate main tun inbound (`:4962-4964`); new tests `g13_102_*` (on->sidecar, off->absent) |
| 103 | `RouteExcludeAddress` (list, default null) | wired, no production change | `filter_route_exclude` + `_with_warnings` variants + engine attach site (`engine.rs:5171-5187`) already extends `generated.diagnostics`; new test `g13_103_*` proves engine-level diagnostics visibility |
| 105 | `IPv6Address` (string, upstream default `fc00::172:18:0:1/126`) | wired, no production change | `tun_address("IPv6Address", ..)` (`tun_plan.rs:146-158` via `:231-237`); bad CIDR fails closed with field `IPv6Address`; new test `g13_105_*` |

Plus cross-cutting: `g13_master_gate_tun_off_suppresses_all_leaves`
(`enable_tun=false` + all leaves set -> no `tun`/`tun-deferred`/sidecar node,
`tun_enabled=false`, `tun_spec_from_plan` None) and
`g13_actual_state_fail_closed_without_adapter` (index 0 refused with field
`interface_index`; deferred plan present but strict resolver errs; tampered
flag-on/node-stripped plan errs `E_INVALID_PLAN`).

## Files changed (this card only)

- `crates/application/src/tun_plan.rs` (sole production edit):
  - New `TUN_IPV6_ADDRESS_MISSING_CODE = "tun_ipv6_address_missing"` +
    `TUN_IPV6_ADDRESS_FIELD = "IPv6Address"` (snake_case code + upstream
    symbol field, same style as the 103 pair).
  - `build_tun_spec_fields_with_warnings` emits one warning when
    `enable_ipv6_address=true` but no address is set; the plan still builds
    IPv4-only (no rejection, no silent pure-IPv4). Resolved + deferred paths
    share the builder, so both warn. Plain entry points keep their
    signatures and Ok/Err behavior (warnings dropped there as before);
    `engine.rs` already extends `generated.diagnostics` with the `_with_warnings`
    output, so the warning surfaces with zero `engine.rs` changes.
- `crates/application/tests/wave_b_g13_tun_leaves.rs` (new, 8 tests):
  synthetic vless leaf (`192.0.2.10`), explicit `TunPlanHints`
  (`g13-synth-tun`/29 — distinctive values asserted back, proving hint
  origin instead of OS discovery); `tun_hints_from_env` /
  `discover_interface_index` never called; ports pre-probed `>= 11808`
  (11928..12028 range, 10808 skipped, no listener held).

## Exact commands + results (workspace root)

| Command | Exit | Result |
|---|---|---|
| `rustfmt --edition 2021 --check crates/application/src/tun_plan.rs crates/application/tests/wave_b_g13_tun_leaves.rs` | 0 | green |
| `cargo fmt -p application -- --check` | 1 (pre-existing, out of scope) | my 2 files clean; remaining diffs only in others' in-flight `cert_chain_roundtrip.rs`, `zz_probe_yaml.rs` (untouched) |
| `cargo clippy -p application --all-targets --locked -- -D warnings` | 1 (pre-existing, out of scope) | single error `config_codegen/src/util.rs:546` `while_let_loop` (another batch's file, untouched); application lints never reached |
| `cargo test -p application --locked --test wave_b_g13_tun_leaves` | 0 | 8/8 pass |
| `cargo test -p application --locked --lib tun_plan` | 0 | 24/24 pass (existing `ipv6_included_only_when_enabled_with_address` still green: plain entry behavior unchanged) |
| `cargo test -p application --locked --test fix13_tun_presocks_plan --test sp24_tun_route_exclude_warn_filter --test t18b_runtime_plan --test r3_02_presocks_sidecar` | 0 | 6 + 3 + 16 + 2 pass, 0 fail |

No workspace-wide run, no release build, no commit.

## Gaps (recorded, none silently closed)

1. Real TUN acceptance (all four leaves): formal TUN window -> FRB -> save ->
   same-revision plan -> core validation -> real session/adapter/route effects
   -> reopen; needs UI/runtime link + authorized isolated machine (SP-34 gate).
2. 100/105 global IPv6 context: `resolve_ipv6_for_plan` / `TunProbeContext`
   (SP-10) still has no production consumer — platform probe (RT-12) + SP-00
   integrator assembly. This card's missing-address warning is the
   settings-level diagnostic only, not probe coverage.
3. 102 production core path: pre-socks sidecar topology is plan-correct, but
   the real core-executable protection effect is unverified (CSV current_gap).
4. Clippy gate for `-p application` is currently red via the in-flight
   `config_codegen` lint above (not this card); integrator to confirm after
   the owning batch lands.
