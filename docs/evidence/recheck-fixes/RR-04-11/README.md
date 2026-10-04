# RR-04 evidence — app self-update actually launches the runner and exits

Status: `implemented` (Dart launch + exit handoff wired and covered by three/four
branch widget tests; release packaging assertions pass). End-to-end application
self-update stays `blocked` because no real release source/signature is
configured; no source was faked and PGP verification was not bypassed.

## Problem (from docs/evidence/parity-recheck-2026-10-04/runtime.md §RR-04)

`update_controller.dart` only stored the `ExternalSpecDto` and told the user
"exit and an external runner will replace and restart", but never launched the
runner and never exited. The runner (`v2rayN-upgrade.exe`, from
`services/upgrade_runner`) therefore never ran, so the visual flow and the real
one diverged.

## Fix

- `apps/desktop/lib/features/update/update_controller.dart`
  - New injectable seams `RunnerLauncher` (`Future<void> Function(helper, args,
    cwd)`) and `AppExit` (`void Function()`); the `UpdateController` constructor
    defaults them to a detached `Process.start` and a delayed `exit(0)`.
  - `stageAppUpdateSpec` replaced by `applyAppUpdate`: it fetches the spec, and
    on `ok=true` with a non-empty `helperExe` actually starts the helper with
    `ProcessStartMode.detached`, `workingDirectory = installRoot`, and the
    backend-provided `args` (which already carry `--plan/--result/--pid/
    --restart-exe/--restart-cwd`), sets the success status, then calls the exit
    handoff. Failures (`ok=false`, empty helper, spawn throw) are reported and
    never reported as success; the app does not exit on failure.
- `apps/desktop/lib/features/update/check_update_view.dart`
  - The "应用自身更新" button now calls `applyAppUpdate`; success text is
    "已启动更新程序，应用将退出" (no more "由外部 runner 执行" without launching).
- Tests: `test/recheck_rr04_app_update_test.dart` (success / launch failure /
  missing helper / spec unavailable) and `test/t16_update_test.dart` updated to
  inject a recording launcher + exit stub (no real process, no real exit).

## Release-packaging assertions

`tools/release/recheck_rr04_release_asserts.ps1` uses
`build_windows.ps1 -SkipBuild -SkipFlutter` with
`-ReleaseDirOverride/-TargetReleaseOverride/-OutRootOverride` pointed at a
synthetic stage and asserts:

- the dist stage contains `v2rayN-upgrade.exe` (and `v2rayn_desktop.exe`);
- `v2rayn-r.iss` `[UninstallDelete]` has no `filesandordirs` rule on bare
  `{app}` / `{app}\*` (no whole-directory recursive delete), while keeping
  `{app}\.staging`, `{app}\app.previous` and `dirifempty {app}`.

## Commands actually run

| Command | Result |
|---|---|
| `dart format` (changed files) | formatted |
| `flutter analyze lib/features/update test/recheck_rr04_app_update_test.dart test/t16_update_test.dart` | No issues |
| `flutter test test/recheck_rr04_app_update_test.dart test/t16_update_test.dart` | 11 passed (`app-update-tests.log`) |
| `pwsh tools/release/recheck_rr04_release_asserts.ps1` | `RR04_RELEASE_ASSERTS ok=true` (`release-asserts.log`) |
| `cargo test -p updater --locked` | 13 passed (`updater-tests.log`) |

## Verification detail

- Success branch: recorder receives `helper=/app/v2rayN-upgrade.exe`, args that
  contain `--plan`, cwd `/app`, and the exit stub is called exactly once; the
  window shows "已启动更新程序，应用将退出".
- Launch failure: launcher throws → no exit call, status
  "启动更新程序失败", no success text.
- Missing helper: `helperExe=null` → no launch, no exit, same error status.
- Spec unavailable: `error.update_app_source_unconfigured` → blocked message,
  no launch, no exit.

No port was touched; 127.0.0.1:10808 was never used; no host proxy/registry/route
change; no real install/uninstall/self-replace was performed.

## Residual / follow-up

- The real release source is unconfigured (`check_app_update` has `app_repo=None`,
  `update_service.rs:83-94,282`); `t16_apply_app_update_spec` returns
  `error.update_app_source_unconfigured`. End-to-end download → verify → replace
  → restart cannot be demonstrated and is registered `blocked`.
- `t16_apply_app_update_spec` fixes `prerelease=true, proxy=None` (RR-04 item 2):
  the window's prerelease/via-proxy toggles are not forwarded to this path.
