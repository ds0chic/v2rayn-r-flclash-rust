# R4-21 十四内核普通入口矩阵 — 证据

任务：`docs/repair/tasks/R4-21.md`
执行前 HEAD：`a95897f`；上游冻结 `2dust/v2rayN@7d6a967`（7.25.4）。
armed=false；未占用/修改 `127.0.0.1:10808`；未改宿主系统代理/注册表/路由/TUN/自启；
未读用户凭据（合成配置/合成 HTTP 回显）；只停止本脚本自己启动并记录 PID 的进程。
执行环境：Windows 11 x64 25H2，Flutter 3.47.5 / Dart 3.13.4，Rust 1.98.1。

## 结论（本卡合同）

1. **普通入口 14 核矩阵完整，不静默遗漏。** `application::ui_targets()` 由
   `CoreType::PROXY_CORES` 派生（`runtime::adapter::update_core_key`），
   `builtin_targets(false)` 返回 15 行（应用 + 14 核）。分类：`Auto`=xray/mihomo/sing_box（+应用），
   `Manual`=其余 11 核（上游无 `DownloadUrl*`，`error.update_manual`），14 核无 `Blocked`。
   逐核矩阵见 [core-entry-matrix.md](core-entry-matrix.md)。
   修复前 `UI_TARGETS` 缺 `hysteria`/`v2fly_v5`，且把所有非自动核标为 `error.update_unsupported`。
2. **普通 UI 安装一个核（缺核→更新窗口→安装→重试）。** `seedMissingCores` →
   `installMissingCores` 驱动既有 T16 `check_updates`/`apply_core_update`；成功后清空缺核横幅，
   可再次启动。UI 把 `Manual` 核渲染为“需手动安装”，不再混同“不支持更新”。
   失败（下载/验签/坏核）逐字可见并可重试；修好后重试成功。
3. **真实内核端到端（xray + mihomo）。** 合成安装到临时受管根
   （`<root>/<dir>/<version>/<exe>`，与 `CoreInstallLayout` 一致）→ 按 adapter 契约真实启动
   → 经本机 SOCKS/HTTP 代理请求本机合成 HTTP 服务，返回 `R4-21-OK`（HTTP 200）。
   端口从 ≥11808 先探测（本机 21808-22999），未使用 10808；仅停止本脚本 PID。
4. **先建失败断言。** `apps/desktop/test/repair/r4_21_repro_test.dart` 在修复前失败
   （`需手动安装` 找不到，实际渲染 `不支持更新`），修复后转绿。

## 改动文件

- `crates/runtime/src/adapter.rs`：`update_core_key` / `core_type_for_update_key`（矩阵权威）+ 测试。
- `crates/application/src/update_service.rs`：`proxy_update_cores`/`ui_targets`/`CoreEntryKind`/
  `core_entry_kind`/`core_entry_note`；`builtin_targets` 覆盖 14 核；manual 行与 `check_core` 标注
  `error.update_manual`；`installed_cores` 覆盖 14 核；测试。
- `crates/bridge_api/src/api/t16.rs`：target 列表测试改为断言 14 核 + manual 标注。
- `apps/desktop/lib/features/update/check_update_view.dart`：`Manual` 行渲染“需手动安装”。
- `apps/desktop/test/r4_21_contract_test.dart`（新）、
  `apps/desktop/test/repair/r4_21_repro_test.dart`（新）。
- `docs/evidence/repair/R4-21/**`、`docs/repair/tasks/R4-21.md`、`compat/features.yaml`（仅追加）。

未改动：`main_shell.dart`、`app.dart`、`frb_generated`、`lib/bridge/api/**`、`lib/bridge/bridge_port.dart`、
`features/{profiles,subs,runtime,settings,routing,monitor,backup}/**`、
`crates/application/src/{engine,subs,dns,routing,monitor,speedtest,backup_service}.rs`、
`crates/updater/**`、`services/**`、`tools/**`。

## 上游对照

- `CoreInfoManager.cs:104-292`：15 个 `CoreInfo`；仅 v2rayN/Xray/mihomo/sing_box 定义
  `DownloadUrl*` → 其余 11 核为手动安装。
- `CoreInfoManager.GetCheckUpdateCoreTypes():53-73` / `IsCheckUpdateSupported():75-85`：
  自动更新集合与此实现 `BUILTIN_TARGETS` 一致。
- `CheckUpdateViewModel.cs:320-330 UpdateFinishedResult`：装好后发布 `ReloadRequested`，
  本实现 UI 安装成功后清空缺核横幅提示“可再次启动”。
- adapter 契约（args/cwd/env）与 `crates/runtime/src/adapter.rs` 及
  `docs/evidence/recheck-fixes/R3-CORE-MATRIX/` 的 14 核实测一致。

## 失败断言复现

- 修复前（临时回退 UI 标签逻辑）：`flutter test test/repair/r4_21_repro_test.dart` → exit 1，
  `Expected: at least one matching candidate / Found 0 widgets with text "需手动安装"`。
- 修复后：exit 0，1 passed。

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | exit 0 |
| `cargo test --workspace --locked` | exit 0（全部 ok） |
| `cargo test -p application --locked --lib update_service` | 10 passed |
| `cargo test -p runtime --locked --lib` | 62 passed |
| `cargo test -p bridge_api --locked --lib t16` | 4 passed |
| `flutter analyze` | exit 0，No issues found |
| `flutter test test/r4_21_contract_test.dart` | 3 passed |
| `flutter test test/repair/r4_21_repro_test.dart` | 1 passed（修复前 exit 1） |
| `flutter build windows --release` | exit 0，Built v2rayn_desktop.exe |
| `run_local_install_launch.ps1` | xray/mihomo listened=true proxied_ok=true body=R4-21-OK；全部 PID stopped=true；10808 未使用 |

## 未完成 / blocked（honest）

- **未跑武装版界面的整链点击**（缺核→装→同一 GUI 重试）：无 armed 打包实例；本机运行中的
  仓库 `dist\...\net_host.exe` 与用户桌面 `v2rayN/xray` 非本卡启动，按约束未触碰。
  该链在 widget 级（`flutter test`）与 真实内核 脚本级（`local-install-launch.json`）分别验证。
- 其余 12 核仅核对其 adapter 契约与矩阵可用性；真实启动+合成 HTTP 采用 R3-CORE-MATRIX 既有
  14/14 会话证据，未在本卡重跑。
- 未实测 OS 级效果（系统代理/TUN/路由），本卡不涉及。
