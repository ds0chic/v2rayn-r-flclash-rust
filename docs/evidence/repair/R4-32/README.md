# R4-32 Windows 未武装包交付 — 证据（2026-10-05）

状态：**本机可执行部分 verified；真实远端发行源/签名/SmartScreen/六平台 blocked**。

- 起点/重建 HEAD：`8651e19d1049d69e27951c98e4884560d56f4c97`（工作树起始干净）。
- 交付包：`dist/v2rayN-R-1.0.0+1-windows-x64.zip`
  - SHA256 `09ca02a5d4d50e05bf57f9aa3f3167a9a0ef3ae53e1943e297b57c147728af5d`
  - setup SHA256 `854fdfa9767bbc40a3882909ed108fa2673bdb5df2233eff42b3f9c442c40786`
  - `build-info.json`：`git_commit=8651e19…`、`git_dirty=false`、`smoke_armed=false`、
    `target_platform=windows-x64`、`version=1.0.0`。`dist/SHA256SUMS` 与实测 zip+setup 一致。
- armed=false；测试端口 ≥11808（实际用 21808+）；全程未占用/修改 127.0.0.1:10808；
  未改宿主系统代理/注册表/路由/TUN/自启；只停止本卡启动并记录的 PID。

## 关键说明：从纯净 HEAD worktree 构建

执行期间共享工作树被**并行子代理**改动（`crates/application/src/engine.rs`、
`services/net_host/src/{server,session}.rs`、`crates/ipc_contract/src/lib.rs`、
`apps/desktop/lib/features/profiles/profiles_controller.dart` 等为 M，另有 R4-25/31/33
未跟踪产物）。直接在主树构建会把未完成改动打入包且 `git_dirty=true`。
因此用 `git worktree add --detach <temp> HEAD` 建立纯净 HEAD 工作树，在其中
`cargo build --workspace --release`→`flutter build windows --release`→
`build_windows.ps1 -SkipBuild -SkipFlutter`（打包前把 Flutter 重生成的
`generated_plugin_*` 与 `dist/*` 还原到 HEAD 以获得 `git_dirty=false`），再把
stage/zip/build-info 同步回主 `dist`，并用 ISCC 重新编译 setup。未改动任何生产源码
（`crates/**`、`lib/**`、`services/**`）。`build-info.git_branch=HEAD`（detached 纯净树）。

## 逐项结果

| # | 项 | 状态 | 证据 |
|---|---|---|---|
| 1 | `cargo fmt --all -- --check` | 通过 | 主树，exit 0 |
| 2 | `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过 | exit 0，无 warning |
| 3 | `cargo test --workspace --locked` | 通过 | exit 0，全仓 `test result: ok` |
| 4 | `flutter analyze` | 通过 | 无 issue（含新增 `test/r4_32_contract_test.dart`） |
| 5 | `flutter build windows --release` | 通过 | `Built …\Release\v2rayn_desktop.exe` |
| 6 | `tools/release/build_windows.ps1` | 通过 | 纯 HEAD worktree，`ARMED False`，zip sha `09ca02a5…` |
| 7 | 包内 build-info `git_dirty=false`、`smoke_armed=false`、同 commit | 通过 | `build-info.json` |
| 8 | `SHA256SUMS`（zip+setup）一致 | 通过 | 实测两文件哈希与文件逐字节一致 |
| 9 | 真实入口：发布包首启 | 通过 | 窗口出现、`guiNDB.db`/`guiNConfig.json` 生成、`r4-32-first-run.png` |
| 10 | 真实入口：退出→立刻重开（同数据目录） | 通过 | 重开窗口出现、DB 持久、`r4-32-reopen.png`；见下“关闭到托盘”说明 |
| 11 | 真实入口：UI-only 导航 hook（更新/备份窗口） | 通过 | `r4-32-update-window.png`、`r4-32-backup-window.png` |
| 12 | 同 commit 真实 UI：导入合成节点+设为活动+reopen | 通过 | `integration-fix07-activate.log`、`integration-fix07-reopen.log`，`failures=[]` |
| 13 | 合成内核启动/停止（xray+mihomo，≥21808） | 通过 | `real-core-launch.log`：listened=true、proxied_ok=true body=`R4-21-OK`、stopped=true、10808 未用 |
| 14 | `negative_unarmed.ps1`（官方包忽略全部自动化 env） | 通过 | `negative-unarmed.json`：无 core、无 journal/core.log/bench、监听归属本进程树为 false |
| 15 | 安装/卸载规则静态断言 | 通过 | `recheck_rr04_release_asserts.log` `ok=true`；`recheck_r4_29_release_asserts.log` `ok=true` |
| 16 | `install_test.ps1 -CompileOnly`（不真实安装宿主） | 通过 | ISCC 编译成功、stage 扁平布局齐全、`T21_INSTALL_COMPILE_ONLY ok=True` |
| 17 | 自更新合成链（stub runner） | 通过 | `selfupdate-*.json` + `.log`：4/4（替换+回滚+摘要不符+崩溃恢复） |
| 18 | `flutter test test/r4_32_contract_test.dart` | 通过 | 5/5 |
| 19 | 真实远端发行源/公钥、签名、SmartScreen、干净机真实安装、六平台 | blocked | 无授权隔离机/无本项目真实发行源与信任公钥；未伪造 |

## 发现

1. **`t21e_import_test.dart` 在 HEAD 确定性失败（2/2）**：第一步“真实菜单剪贴板导入”
   未出现 `已从剪贴板导入` 状态（`TestFailure: no import status message appeared`）。
   日志 `integration-t21e-import.log`。未修复：属 `features/profiles|subs` 生产行为，
   本卡禁止改动；且可能为集成测试/剪贴板环境陈旧。**登记待根代理分诊**（若确认是
   生产缺陷即 P1，需按 IsSub/入口规则最小修复并补测）。同 commit 的 FIX-07 通过
   共享服务的真实导入+设为活动+reopen 链，说明导入服务本身可用。
2. **发布包关闭为“最小化到托盘”**：`CloseMainWindow` 不终止进程（上游默认行为），
   本卡脚本据此在记录 PID 树后强制停止再重开；重开后持久状态正确。非缺陷，已记录。
3. **并行子代理使共享工作树变脏**（见上）；未触碰其改动，改用纯净 worktree 构建。
4. `negative_unarmed.ps1` 原先把“任意进程占用 11808”误判为失败；已改为按监听
   **归属本进程树**判定（并发测试不再产生假阳性），主断言（无 core/journal/bench）不变。
5. `install_test.ps1` 新增 `-CompileOnly`：编译安装器并静态校验 stage 布局，**不做**
   宿主安装/卸载，满足本卡硬约束；原真实安装/卸载路径保留不变。

## 改动文件

- 新增 `tools/release/r4_32_real_entry.ps1`（发布包首启/重开/UI hook 回归）。
- 改 `tools/release/negative_unarmed.ps1`（11808 监听按本进程树归属判定）。
- 改 `tools/release/install_test.ps1`（新增 `-CompileOnly`，默认行为不变）。
- 新增 `apps/desktop/test/r4_32_contract_test.dart`（R4-32 交付契约 5 项）。
- 新增 `docs/evidence/repair/R4-32/**`（本目录）；复制 R4-21 内核 harness 用于本卡
  合成内核启动/停止复证（`run_local_install_launch.ps1`、`synthetic_http_server.ps1`）。
- 追加 `docs/repair/tasks/R4-32.md` 执行记录。
- 刷新 `dist/`（zip/setup/build-info/SHA256SUMS；禁改文件未动）。
- `docs/evidence/repair/R4-32/observations.json` 为发布包真实入口观测（脚本产出）。

## 未完成 / 下一步前置

- 根代理需在**干净提交树**上重跑 `build_windows.ps1 -SkipBuild -SkipFlutter` 以输出
  `git_branch` 为分支名（本卡为 detached `HEAD`）并最终刷新 `dist/`。
- 分诊 `t21e_import_test.dart`（真实菜单剪贴板导入状态）。
- 配置本项目真实发行源/信任公钥，并在授权隔离机执行真实安装/卸载、签名与
  SmartScreen、六平台矩阵，才能把第 19 项转 verified。
