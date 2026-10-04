# RR-01 evidence — managed cores root is the single source of truth

Status: `implemented` (Rust-side full chain verified; no Flutter GUI build, per task boundary).

## Problem (from docs/evidence/parity-recheck-2026-10-04/runtime.md §RR-01)

Install root was `V2RAYN_R_CORES_DIR` or `engine.data_dir()/cores` (= `%LOCALAPPDATA%/v2rayn-r/data/cores`),
but net-host's `CoreLocator::from_env` only looked at `V2RAYN_R_CORES_ROOT`,
`%LOCALAPPDATA%/v2rayn-r/cores` and the dev `tools/cores`, and `NetHostClient`
never forwarded the engine data/cores root. A user who downloaded a core could
still get `core_not_found`, or silently run a stale dev-tree binary.

## Fix

Contract: the engine cores root `<data_dir>/cores` (`V2RAYN_R_CORES_DIR`
override) is authoritative; it is forwarded to the managed net-host child as
`V2RAYN_R_CORES_ROOT`, which net-host's locator honours with absolute priority.

- `crates/runtime/src/adapter.rs`
  - `CoreLocator::from_env` now puts an explicit `V2RAYN_R_CORES_ROOT` first and
    that root is the *only* root when set — the `tools/cores` dev fallback is
    disabled so a development tree can never mask production.
  - Without an explicit root it defaults to `default_managed_cores_root()`
    (`V2RAYN_R_DATA_DIR` or `%LOCALAPPDATA%/v2rayn-r/data/cores`), which now
    matches `AppEngine::default_data_dir()/cores`; `tools/cores` remains a
    lowest-priority dev fallback.
- `crates/application/src/engine.rs`
  - New `AppEngine::cores_root()` and free `managed_cores_root(data_dir)` — the
    single definition of the managed root (`V2RAYN_R_CORES_DIR` override).
  - `AppEngine::open` builds the production `NetHostClient` with that root.
- `crates/application/src/net_host_client.rs`
  - `NetHostClient::with_cores_root`/`set_cores_root`/`cores_root`.
  - `apply_launch_env` sets `V2RAYN_R_CORES_ROOT` on the launched net-host
    (explicit process env wins over the client field).
- `crates/bridge_api/src/api/t16.rs`
  - `cores_root()` now delegates to `engine().cores_root()` (one definition for
    install and run).

Not changed: FIX-12 `CoreInstallLayout` semantics, PGP verification,
`frb_generated`, Dart, net_host source.

## Commands actually run

| Command | Result |
|---|---|
| `cargo build -p runtime -p application --locked` | exit 0 |
| `cargo build -p bridge_api --locked` | exit 0 (after a peer session's unrelated `bridge_api/src/api/subs.rs` edit was fixed by them) |
| `cargo build -p net_host --locked` | exit 0 |
| `cargo test -p runtime --lib --locked adapter::tests` | 7 passed |
| `cargo test -p runtime --lib --locked install_layout` | 4 passed |
| `cargo test -p application --lib --locked net_host_client` | 4 passed |
| `cargo test -p application --test rr01_cores_root --locked` | 2 passed (real-spawn skipped by default) |
| `$env:V2RAYN_R_RR01_SPAWN=1; cargo test -p application --test rr01_cores_root --locked real_spawn -- --nocapture` | 1 passed (real xray spawn) |
| `cargo test -p application --test t16_update --locked` | 6 passed |
| `cargo fmt -p runtime -p application -p bridge_api -- --check` | clean |
| `cargo clippy -p runtime -p application --all-targets --locked -- -D warnings` | clean |
| `cargo clippy -p bridge_api --all-targets --locked -- -D warnings` | clean |

## Verification detail

- `install_and_run_share_engine_cores_root`: installs a synthetic core with
  `UpdateService::install_core_from_dir` into a temp data dir, asserts
  `managed_cores_root` == `<data>/cores`, then `CoreLocator::with_roots([that
  root]).resolve` returns exactly the just-installed `xray/26.3.27/xray.exe`;
  `AppEngine::open(data).cores_root()` equals the same root;
  `NetHostClient::with_cores_root` reports the same root.
- `launch_env_forwards_configured_cores_root` / `launch_env_without_root_...`:
  capture the child `Command` env and assert `V2RAYN_R_CORES_ROOT` is present
  with the configured value, and absent when unconfigured. No core is started.
- `explicit_cores_root_disables_dev_fallback`: with `V2RAYN_R_CORES_ROOT` set,
  the locator has exactly one root and no `tools/cores` fallback.
- `default_root_matches_engine_data_cores`: with `V2RAYN_R_DATA_DIR` set and no
  explicit root, the first root is `<data>/cores`.
- `real_spawn_proves_temporary_root` (opt-in): copies
  `tools/cores/xray/v26.3.27` into a temporary cores root, resolves through
  `CoreLocator`, starts it on a probed port in `[11808,11900)` with a synthetic
  SOCKS+freedom config, confirms it stayed alive, and stops only that PID. The
  resolved exe path `starts_with` the temporary root, proving the process exe
  came from the temporary root and not the development tree.

No port below 11808 was touched; 127.0.0.1:10808 was never used; the dev core
copy target is a throwaway temp dir; only the spawned temp-core PID is killed.

## Residual / follow-up

- Flutter-side build/analyze and a GUI end-to-end download→run were not run
  (out of the RR-01 Rust scope and prohibited by the task boundary).
- SSH/`tools/cores` fallback is dev-only; a clean off-repo RC should now never
  need `V2RAYN_R_CORES_ROOT` set by the user because `NetHostClient` always
  forwards it.
