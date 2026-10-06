# SP-00 helper protocol v2: lease / observation ops (2026-10-07)

Integrator (main-control) closed the registered interface gaps N-H1/N-H2/N-H3
(see SP-08/SP-09/SP-10 notes) at the contract and helper-server layers.

## Interface (crates/ipc_contract/src/helper.rs)

`HELPER_PROTOCOL_VERSION` bumped 1 → 2. Policy: both planes ship together; a
v1 helper cannot decode the new ops, so `check_helper_session` rejects the
mismatch with a structured `VersionMismatch` before dispatch instead of a
decode error. `ElevationStatus.protocol_version` reports the helper's version
for the handshake.

New ops (all validated before dispatch, all bounded):

- `RenewLease { handle }` → `LeaseRenewed { handle, expires_at_ms }`.
  Unknown handle → `UnknownHandle`; zero handle → `Malformed`.
- `GetLeaseStatus { handle }` → `LeaseStatus { handle, state, pid?, expires_at_ms }`
  with `LeaseState::{Active, Exited, Released}`. Non-draining.
- `PollCoreExits { handles }` → `CoreExits { exits: Vec<CoreExitObservation> }`.
  Drain-once; every drained exit is remembered in the lease so
  `GetLeaseStatus` can report `Exited` even if the caller did not list the
  handle that round. Bounds: non-empty, ≤ `HELPER_MAX_POLL_HANDLES` (256),
  non-zero, no duplicates.

Cleanup idempotency: `HelperResult::AlreadyGone { resource }` — cleanup
confirmed the resource was already absent; not an error. Helper-side wiring of
this variant into `RemoveRoutes`/`ResetTunAddress` not-found paths is a
follow-up in the helper lane.

## Helper server (services/privileged_helper/src/server.rs)

- `ConnectionLease` now tracks per-handle `lease_expires_at_ms` (set at start
  from `HelperServerConfig.idle_timeout`, extended by `RenewLease`),
  `core_pids`, `released_cores` and `observed_exits`.
- `RunElevatedCore` records the lease; `StopElevatedCore` and cleanup
  (`release_cores`) mark the handle `Released`; `PollCoreExits` drains
  `HelperBackend::poll_core_exits` (existing SP-06 backend surface) and records
  observations.

## Verification

- `cargo test -p ipc_contract --locked` — 39/39 (new roundtrip/timeout/
  validation tests).
- `cargo test -p privileged_helper --locked` — all suites green; new dispatch
  tests `sp09_renew_lease_extends_owned_handle_only`,
  `sp10_poll_core_exits_drains_once_and_status_reflects_it`,
  `sp10_poll_handles_validate_before_backend`.
- `cargo clippy -p ipc_contract -p privileged_helper -p net_host --all-targets
  --locked -- -D warnings` — 0.

## Follow-ups (not verified)

- net_host adoption (renew loop + PollCoreExits → elevated-exit observation)
  is the next lane; real elevated-helper runs remain unverified on this host.
