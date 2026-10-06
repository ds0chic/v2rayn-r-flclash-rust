# SP-34 登记：固定身份最终发布包（准备，基线 3635392）

状态：identified（仅登记；包未重建；门禁未跑）。

## 当前 dist 盘点（只读观察，未改动）

- `dist/v2rayN-R-1.0.0+1-windows-x64.zip`（2026-10-05）内 `build-info.json`：
  `git_commit=672e666`，`git_dirty=true`，`smoke_armed=false`。
- 结论：该包**过期且 dirty**，相对基线 3635392 已漂移，**不得复用于 SP-30/S-34
  验收**（SP-34：历史 ZIP 不得复用）。

## SP-34 放行条件（重建后由整合者执行）

1. 从固定提交干净重建（`tools/release/build_windows.ps1`），新 ZIP/SHA/build-info
   满足 `armed=false`/`dirty=false`，commit == 固定提交。
2. 身份核对：`tools/acceptance/sp30_package_identity.ps1 -Zip <新包>` exit 0。
3. 完整 AGENTS 门禁（`tools/gates/run_all.ps1` 无 filter）+ 普通包/license 验收。
4. 所有适用实例 verified 后方可报告稳定完成；定向结果不冒充完整门禁。

## 阻塞

与 SP-30 observations.json B1/B2 相同：运行中 exe 须先正常退出；真实 OS
副作用仅授权隔离机。
