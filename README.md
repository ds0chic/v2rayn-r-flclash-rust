# v2rayN-R

**v2rayN-R** is an in-progress, unofficial reimplementation of
[2dust/v2rayN](https://github.com/2dust/v2rayN) with a **Flutter** UI and a
**Rust** application backend. It targets the frozen upstream baseline
**v2rayN 7.25.4** (commit `7d6a967`) and keeps the original window structure,
menus, tables, and setting groups while improving visuals and performance.

> This is a **derived refactor**, not an official v2rayN release. See
> [`NOTICE.md`](NOTICE.md). All applicable upstream functionality, settings, and
> platform behavior belong to upstream; this project reimplements them under
> GPL-3.0.

## Status

Release-candidate stage. The application builds and runs on **Windows x64** and
has a verified packaged clean-environment smoke (startup, data-dir creation,
real UI → Rust → net-host → core apply). Other platforms are **not built or
verified**. See [`compat/platform-matrix.md`](compat/platform-matrix.md) and
[`docs/evidence/T20.md`](docs/evidence/T20.md).

### Release notes (RC)

- The portable package **bundles no proxy core**. The packaged smoke evidence
  used a developer-local `tools/cores/xray` binary; end users obtain cores at
  runtime via *Check updates* (see `dist/CORE-NOTES.txt`).
- Runtime target: **Windows 10/11 x64**. A clean machine may require the
  Microsoft Visual C++ Redistributable; dependency closure was not proven with
  Process Monitor (registered in `docs/evidence/T20.md`).
- Automation environment hooks are **compiled out of default release builds**.
  Evidence/benchmark drivers must build with
  `--dart-define=V2RAYN_R_SMOKE_ARMED=true`; unarmed builds ignore
  `V2RAYN_R_AUTO_SMOKE`/`V2RAYN_R_T18_BENCH` (ISSUE-08 guard).
- Real-machine verification status (Windows x64, see
  [`docs/evidence/T21-real-os.md`](docs/evidence/T21-real-os.md)):
  - **verified**: system-proxy write/read/restore (current user; registry
    authoritative), autostart Run-key roundtrip, privileged route add/remove
    (elevated), TUN adapter create/destroy (safe scope, default route
    untouched), PGP v5 release-asset signature verification, `.dgst` digests,
    Inno Setup install/uninstall, external self-update replace/rollback.
  - **not verified**: public distribution requires code signing; self-update
    assets are verified but unsigned; real remote servers/TLS handshakes;
    production TUN with automatic routes/global takeover (would alter the
    live network); non-Windows platforms.

## Architecture

```
apps/desktop/        Flutter shell (lib/app, lib/features/*, lib/shared, lib/bridge)
crates/              domain, application, persistence, subscriptions,
                     config_codegen, core_adapters, runtime, platform,
                     updater, bridge_api, ipc_contract
services/net_host/  普通权限运行管理（唯一内核进程持有者）
services/privileged_helper/  最小提权操作（TUN/路由）
compat/              四份台账 + upstream-lock + 平台矩阵
fixtures/            夹具（source/ 上游模板；其余为合成测试数据）
tests/system/        平台与故障测试
benchmarks/          测量数据
docs/tasks/          任务卡
docs/evidence/       每回合证据
docs/decisions/      架构决策记录（ADR）
tools/               开发/审计脚本（含 tools/release 打包与冒烟）
```

Dependency direction: `UI → bridge_api → application → domain / special
modules`. `domain` never depends on Flutter/window/platform APIs; `runtime`
never calls the UI; platform modules never own subscription or profile
business logic. Real proxy traffic never passes through Flutter/FRB; the UI does
not own core runtime state (`desired_revision` and `applied_runtime_revision`
are separate).

## Features (overview)

- 11 profile protocols with the upstream editor forms; Xray and sing-box config
  generators kept as independent chains (no forced Mihomo-YAML conversion).
- Three main layouts, menu/toolbar/status bar, virtualized profile table,
  multi-select, column settings, groups, templates, custom outbounds.
- Import/subscriptions, routing and DNS, settings, backup (local/WebDAV),
  in-app core/app updater, monitor (logs/connections/proxies), speed test.
- Runtime managed solely by `net_host`: one owner of core processes, Job-bound,
  lease reclaim on client loss, structured error codes, no fake "Running".

## Build

### Prerequisites (locked toolchain)

| Tool | Version | Path (this machine) |
|---|---|---|
| Flutter | 3.47.5 stable (Dart 3.13.4) | `C:\Users\Colby\toolchains\flutter\bin\flutter.bat` |
| Rust | 1.98.1 stable-x86_64-pc-windows-msvc | `C:\Users\Colby\.cargo\bin\cargo.exe` |
| VS BuildTools | 2022 17.14 (CMake / Ninja / Win10 SDK 10.0.26100) | VS bundled |
| flutter_rust_bridge | 2.13.0 (Dart/Rust/codegen) | pinned |
| OS | Windows 11 25H2 (26220) x64 | — |

### Rust workspace (repository root)

```powershell
cargo build --workspace --release --locked
```

### Flutter app (`apps/desktop`)

```powershell
flutter build windows --release
```

The Flutter build triggers the Rust `bridge_api` cdylib via `rust_builder` /
cargokit; `net_host.exe` and `privileged_helper.exe` must also be built (the
release script below does the whole sequence).

### Packaged release candidate

```powershell
pwsh -File tools/release/build_windows.ps1
```

Produces `dist/v2rayN-R-<version>-windows-x64/`, the matching
`.zip`, `dist/SHA256SUMS`, and `dist/build-info.json`. The script is
idempotent. **No proxy core binaries are bundled**; cores are downloaded at
runtime and pinned by [`tools/cores/cores.lock.json`](tools/cores/cores.lock.json).

## Test / gates

Rust workspace root:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Flutter app directory (`apps/desktop`):

```powershell
dart format --output=none --set-exit-if-changed lib test
flutter analyze
flutter test
flutter build windows --release
```

`flutter test` is run per file via `tools/flutter_test_retry.ps1 -PerFile`
because the locked Flutter 3.47.5 `flutter_tester` intermittently segfaults on
this host; per-file isolation is the established method and every file passes.

## Run

The portable package is self-contained for the app itself:

```powershell
# from an extracted dist/v2rayN-R-<version>-windows-x64/
$env:V2RAYN_R_DATA_DIR = "$env:TEMP\v2rayn-r-data"   # optional; defaults under %LOCALAPPDATA%
.\v2rayn_desktop.exe
```

Useful environment variables (all optional):

| Variable | Purpose |
|---|---|
| `V2RAYN_R_DATA_DIR` | application data directory (`guiNConfig.json`, `guiNNDB.db`) |
| `V2RAYN_R_CORES_ROOT` / `V2RAYN_R_XRAY_BIN` | point at an existing core install for offline runs |
| `V2RAYN_R_RUN_ROOT` | net-host session/journal directory |
| `V2RAYN_R_PIPE` | net-host named-pipe name |
| `V2RAYN_R_AUTO_SMOKE` | release smoke driver: apply the persisted plan on launch |

## Known limitations

- **Runtime settings apply chain**: the packaged apply path consumes the real
  persisted plan (verified in T20), but fields still marked `unwired` /
  `codegen_only` in `docs/evidence/T18.md` are not yet wired end-to-end.
- **Windows ARM64**: `flutter build windows --target-platform` is not supported
  by the pinned Flutter CLI (T20 ARM64 attempt failed at option parsing).
- **macOS / Linux**: not built or verified; no support is claimed.
- **Code signing / installer / auto-update execution**: not performed.
- Performance/stability numbers are short-sample, not a 24h soak
  (`docs/evidence/T18.md`).
- The `ui_state.json` draft sits next to the exe and is excluded from the
  package.

## Platform support

| Platform | Build | Smoke | Notes |
|---|---|---|---|
| Windows x64 | ✅ | ✅ packaged startup + data dir + window + real apply | verified scope in `compat/platform-matrix.md` |
| Windows ARM64 | ❌ unsupported by pinned CLI | — | see T20 evidence |
| macOS / Linux | ❌ not built | ❌ not verified | `tools/release/README-platforms.md` |

## Security & compliance

- No core binaries are bundled; third-party cores keep their own licenses.
- Secrets (subscriptions, credentials, cookies) are never written to logs,
  evidence, or fixtures.
- Tests never bind/alter `127.0.0.1:10808`, the host system proxy, or TUN.
- GPL-3.0 with upstream attribution in [`NOTICE.md`](NOTICE.md); full license in
  [`LICENSE`](LICENSE).

## Milestone evidence index

| Milestone | Evidence |
|---|---|
| T00 baseline & ledgers | `docs/evidence/T00*.md`, `compat/*` |
| T01 feasibility | `docs/evidence/T01.md` |
| T02 contracts/models | `docs/evidence/T02.md` |
| T03 runtime minimal loop | `docs/evidence/T03.md` |
| T04 storage/migration | `docs/evidence/T04*.md` |
| T05 main window/table | `docs/evidence/T05.md` |
| T06 profile edit & persistence | `docs/evidence/T06a.md`, `T06b-validation.md` |
| T07/T08 config generation | `docs/evidence/T07-T08-codegen.md` |
| T09 import/subscriptions | `docs/evidence/T09.md` |
| T10 custom/group/template | `docs/evidence/T10.md` |
| T11 routing/DNS | `docs/evidence/T11.md` |
| T12 settings | `docs/evidence/T12a.md` |
| T13 desktop integration | `docs/evidence/T13.md`, `T13-platform.md` |
| T14 TUN / privilege | `docs/evidence/T14-runtime.md`, `T14-helper.md` |
| T15 speed test / monitor | `docs/evidence/T15a.md`, `T15b.md` |
| T16 backup/update | `docs/evidence/T16.md`, `T16-updater.md` |
| T17 visual polish | `docs/evidence/T17.md` |
| T18 performance/stability | `docs/evidence/T18.md` |
| T18b runtime wiring | `docs/evidence/T18b.md` |
| T20 release candidate | `docs/evidence/T20.md` |
