# SP-17 integrator addendum: actual descriptor exposed end-to-end (2026-10-07)

The SP-17 Dart-side work (see README.md) was blocked on `SnapshotDto` not
exposing the actual-runtime descriptor. The integrator (main-control, shared
IPC/DTO/FRB ownership) has now wired it through every layer:

## Data path

1. `services/net_host/src/session.rs` — `inner.detail` (`runtime::RuntimeDetail`)
   now mirrors the SP-06 facts on every transition:
   - unsolicited main exit: `actual_generation`, `last_exit` (`pid/exit_code/at_ms`),
     `last_exit_sidecar=None`;
   - sidecar exit: same plus `last_exit_sidecar=Some(id)`;
   - fresh session: all three cleared, `core_version` set from `plan.target.version`.
2. `crates/runtime/src/wire.rs` — `RuntimeDetail` gained
   `actual_generation`, `core_version`, `last_exit` (`RuntimeExitFact`),
   `last_exit_sidecar` (all serde-default; old peers still decode).
3. `crates/application/src/runtime_client.rs` — `RuntimeSnapshot` gained the
   same fields (`ExitFact` is the application-local type; the wire stays
   decoupled). `net_host_client::map_snapshot` maps them untouched.
4. `crates/application/src/snapshot.rs` — new `ActualRuntimeView` /
   `ExitFactView`; `assemble` takes it as an explicit part.
   `engine.rs::actual_runtime_view` composes it from the submit-time
   `FrozenAppliedTarget` (target_profile_id/plan_hash/target_core) plus the
   live net-host facts. `None` when no actual fact exists at all.
5. `crates/bridge_api/src/api/contract.rs` — `SnapshotDto.actual:
   Option<ActualRuntimeDto>` + `ExitFactDto`; FRB bindings regenerated
   (`frb_generated.rs`, `lib/bridge/**`).
6. `apps/desktop/lib/features/runtime/runtime_bridge.dart` — `RuntimeActualView`
   + `RuntimeView.actual`, mapped from the DTO; redacted `exitFactLabel`
   getter (`pid=.. code=..`).

## Verification (targeted, per VALIDATION_POLICY)

- `cargo test -p runtime -p net_host -p bridge_api --locked` — green
  (bridge_api 81/81 incl. updated snapshot contract test).
- `cargo test -p application --locked` — green (lib 342 + integration;
  `sp05_applied_target` 8/8 incl. new `sp17_actual_view_is_absent_without_facts`
  and the extended applied-A/B assertions).
- `cargo fmt --all -- --check` — 0; `cargo clippy -p runtime -p application
  -p net_host -p bridge_api --all-targets --locked -- -D warnings` — 0.
- `dart format lib test` — 0 changed; `flutter analyze` — no issues.
- `flutter test` sp_17 four files — 12/12; `t06a_frb_bridge_test` (real FRB
  round-trip against the rebuilt debug DLL) — pass.

## Not verified

- Real GUI candidate trace of the descriptor on a running core (needs a clean
  candidate build; the running dist instance blocks `flutter build windows
  --release`). Status stays `implemented`, never `verified`.
