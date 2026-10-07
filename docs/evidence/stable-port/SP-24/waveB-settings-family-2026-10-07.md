# SP-24 Wave B settings-family evidence (2026-10-07)

Scope: FLD-CFG-036, 043, 044, 052, 053, 054, 055, 059, 060, 079, 110, 112 —
formal entry through `apps/desktop/lib/features/settings/option_setting_window.dart`
(single writer: this round only), synthetic fake-bridge/widget tests only.
No live core, no real network, no OS writes. 10808 untouched (no listener,
no proxy call in any new test). No cargo, no whole-suite gate, no release
build, no commit. Ledger CSV / FLD docs / manifest / bridge_api /
ipc_contract / services / workspace Cargo untouched.

Upstream baseline (read-only `work/`): frozen v2rayN 7.25.4
(`2dust-v2rayN-7d6a967`, commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`).

## Files changed

- `apps/desktop/lib/features/settings/option_setting_window.dart`
  - 036: new `settings-inbound-protocol` dropdown (frozen `EInboundProtocol`
    names/values 0,1,2,3,4,5,6,21) in 本地监听；unknown persisted int shown
    unset, kept on save (no silent remap).
  - 043/044: no logic change (already `_set('Inbound','User'/'Pass')` with the
    `NewPort4LAN` enable-gate); added `settings-newport4lan`,
    `settings-inbound-user`, `settings-inbound-pass` keys for tests.
  - 052-055: new 历史保留 gRPC section (`settings-grpc-idle-timeout`,
    `settings-grpc-health-timeout`, `settings-grpc-permit-without-stream`,
    `settings-grpc-initial-windows-size`); null = 缺省 → frozen defaults.
  - 059/060/079/110: no logic change (entries already wired); added keys
    `settings-keep-older-dedupl`, `settings-auto-update-interval`,
    `settings-hide2tray`, `settings-ipapi-url`.
  - 112: intentionally no control (see gap below).
- `apps/desktop/test/repair/wave_b_settings_036_044_test.dart` (new, 4 tests)
- `apps/desktop/test/repair/wave_b_grpc_052_test.dart` (new, 4 tests)
- `apps/desktop/test/repair/wave_b_settings_059_079_test.dart` (new, 5 tests)
- `apps/desktop/test/repair/wave_b_settings_110_112_test.dart` (new, 2 tests)

## Per-id status

| id | status | what was done | remaining gap |
|---|---|---|---|
| 036 | implemented | Protocol dropdown added; entry→save→reopen (mixed=6); unknown 99 preserved, dropdown unset | real socks/HTTP request对照 + mixed/分开入站验收未跑 (SP-34) |
| 043 | implemented | entry verified wired; gate-off disabled + gate-on edit→save→reopen; disabled-keeps-value | 真实认证成功/拒绝验收未跑 (SP-34) |
| 044 | implemented | same as 043 (paired) | 正确/错误认证 + 备份恢复验收未跑 (SP-34) |
| 052 | implemented | IdleTimeout editor; defaults 60 render; edit 11→save→reopen; blank→null→default display; invalid-port visible-error/rollback case | 真实 gRPC session 空闲行为未跑 (SP-34) |
| 053 | implemented | HealthCheckTimeout editor (same file/cases as 052, value 7) | 超时/取消真实会话未跑 (SP-34) |
| 054 | implemented | PermitWithoutStream checkbox (false→true→save→reopen) | 布尔组合真实会话未跑 (SP-34) |
| 055 | implemented | InitialWindowsSize editor (65535→save→reopen) | 发送路径窗口参数验收未跑 (SP-34) |
| 059 | implemented | KeepOlderDedupl toggle→save→reopen (existing wiring + new key/test) | 合成订阅去重身份/关联端到端未跑 (SP-34) |
| 060 | implemented | AutoUpdateInterval 6→save→reopen (existing wiring + new key/test) | 真实调度启停/改周期/退出停止未跑 (SP-34) |
| 079 | implemented (Windows half) | ruling recorded below; entry toggle→save→reopen; Windows-half truth table; independent-window close test | Linux 真机 X 双路径 + 托盘菜单真实验收未跑 (SP-16/SP-18, 隔离机) |
| 110 | implemented | IPAPIUrl entry verified present; URL→save→reopen; clear→null (empty-filter parity `speedtest.rs:419`) | 合成 IP 源真实请求→UI 显示未跑 (SP-29/SP-34) |
| 112 | gap registered, skipped | NO edit: FLD allows Rust-only modules; upstream `OptionSettingWindow.xaml` has no PageSize row; consumer `profiles_controller.dart:661` reads it; formal edit entry undecided | 正式编辑入口位置未定 + 多页/边界/空页/取消验收未跑 (SP-29/SP-34) |

No id is marked verified (all lack the SP-34 live-effect half by definition).

## 079 platform_scope ruling (verified from frozen upstream, read-only)

- `v2rayN/v2rayN/Views/MainWindow.xaml.cs:193-197` (WPF/Windows):
  `MainWindow_Closing` unconditionally `e.Cancel = true; ShowHideWindow(false);`.
  The close-box X on Windows ALWAYS hides to tray; `Hide2TrayWhenClose` has no
  observable effect on Windows. Real exit only via menu confirm
  (`MenuClose_Click` → `AppExitAsync`) / session ending.
- `v2rayN/v2rayN.Desktop/Views/MainWindow.axaml.cs:192-213,340-362` (Avalonia):
  `OnClosing` cancels + hides for WindowClosing/OwnerWindowClosing; and
  `ShowHideWindow(false)`: `if (Utils.IsLinux() && Hide2TrayWhenClose == false)
  { minimize; return; }`, else close owned windows + `Hide()`. So on Linux the
  switch decides minimize-vs-hide-to-tray (never quit); real quit only via
  `MenuClose_Click` confirm. Independent windows (Sub/Routing/Option) close
  with state save only, never exit the app.
- Ruling: ledger `platform_scope=['linux']` is CORRECT as "where the setting has
  an observable effect". Windows behavior = always-hide (already implemented in
  `desktop_integration.dart:156-158,192-198,470-484`, untouched this round as it
  is outside the allowed paths). This round implements/tests the Windows half
  only: `hideOnClose` truth table (Windows always true), `CloseBehavior`
  defaults, entry save→reopen, independent-window cancel→`host.close()` only.
  Linux close-box dual path + tray-menu observation remain SP-16/SP-18 work on
  an authorized isolated machine.

## Exact commands + results

Working dir `apps/desktop`, toolchain
`C:\Users\Colby\toolchains\flutter\bin\flutter.bat` (`dart.bat` same dir).
Targeted only (no cargo, no whole suite, no release build).

1. `dart.bat format lib\features\settings\option_setting_window.dart test\repair\wave_b_settings_036_044_test.dart test\repair\wave_b_grpc_052_test.dart test\repair\wave_b_settings_059_079_test.dart test\repair\wave_b_settings_110_112_test.dart`
   → 0 changed after formatting (second run `--output=none --set-exit-if-changed`, exit 0).
2. `flutter.bat analyze` → clean except 1 pre-existing info in
   `test\waveB_window_geometry_test.dart` (another agent's file, untouched).
3. `flutter.bat test test\repair\wave_b_settings_036_044_test.dart` → 4/4 passed.
4. `flutter.bat test test\repair\wave_b_grpc_052_test.dart` → 4/4 passed
   (after one legit fix: missed-tap on off-screen checkbox → added
   `ensureVisible`; re-ran full file → all passed).
5. `flutter.bat test test\repair\wave_b_settings_059_079_test.dart` → 5/5 passed.
   (First full-file attempt hit the known `flutter_tester` "did not complete"
   crash at varying positions; per instruction retried once — still flaky at
   5 widget tests/file; split the 110/112 groups into their own file, after
   which this file passed 5/5 cleanly. Each previously-hung test also passed
   individually via `--plain-name`, proving no logic hang.)
6. `flutter.bat test test\repair\wave_b_settings_110_112_test.dart` → 2/2 passed.
7. Regression spot-check (adjacent existing window tests, not a suite gate):
   `flutter.bat test test\fix08_option_apply_test.dart test\r4_13_s13_contract_test.dart` → 3/3 passed.

## Gaps (all recorded, none silently closed)

- 036/043/044/052-055/059/060/079/110: live-effect halves (real protocol
  traffic, real auth accept/reject, real gRPC session, dedup identity, real
  scheduler, Linux close-box/tray observation, synthetic IP-source request)
  all require isolated-run infra → SP-34 gate, not this window.
- 112: formal edit entry for `SpeedTestItem.SpeedTestPageSize` is undecided
  (correct consumer file `profiles_controller.dart:661` reads it; no upstream
  OptionSetting row; FLD allows Rust-only). Needs an SP-29 ruling on which
  window owns the entry before any control is added.
- Negative-number handling (`AutoUpdateInterval`, gRPC ints): UI round-trips
  the typed int (upstream WPF boxes have no range validation either); range
  rejection, if any, belongs to the Rust data-layer gate (Wave A cargo
  territory) — no UI validation was invented, partly because new localized
  validation strings live outside the allowed edit paths.
- `test\waveB_window_geometry_test.dart` (other agent's file) has a
  file-name info; left untouched.

## FLD-CFG-112 ruling (integrator, 2026-10-07)

Verified against the frozen upstream: there is no PageSize row in the settings
XAML, so no formal entry control exists to add. The canonical consumer is
`profiles_controller.dart:661` (page-size read). Ruling: treat 112 like the
other no-control rows (entry `preserved`; acceptance is the data-layer
consumer read), not a Wave B control item. Any future page-size editor is a
product decision, out of port scope.
