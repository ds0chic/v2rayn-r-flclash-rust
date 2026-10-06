# SP-30 正式 GUI 完整流程：验收清单（准备，基线 3635392）

状态：identified（清单 + 夹具 + 只读脚本已建；真实 GUI 验收未运行）。
本次只写 `fixtures/acceptance/**`、`tools/acceptance/**`、本目录；
未改生产代码、tools/release、FRB、Cargo 锁；未 commit。

前置证据：SP-29 门禁 `../SP-29/runs/20261007-003811/gate-result.json`
verdict=`fail+incomplete`（7 fail + 1 incomplete，8 文件定向过滤运行，非全量）——
SP-30 真实验收开始前须先复核 SP-03/06/07/12–20/22/24/25/27/28/29 当前证据，
本文不宣称前置已完成。

## 通用记录格式（每步）

`{"step":"S1", "entry":"<真实入口>", "expected":"...", "actual":"...",
"exit":0, "evidence":"<截图/日志/文件hash路径>", "cleanup":"owned pid/目录已清"}`；
失败判定：任一 `actual != expected` 即停，保留现场（数据目录/日志路径），
记 `blocked` 并登记，不重放非幂等写。取消语义：已提交操作不假称未改变。

## 步骤

| # | 动作 | 真实入口 | 通过判定 | 失败判定/证据采集点 |
|---|---|---|---|---|
| S0 | 候选包身份核对 | `tools/acceptance/sp30_package_identity.ps1 -Zip <新包>`（只读消费） | exit 0：zip sha 与 SHA256SUMS 一致；`smoke_armed=false`；`git_dirty=false`；`git_commit==重建基线`；flat 四 exe 齐；无内核捆绑 | 任一断言失败即停：输出 JSON 全量存档；**历史 ZIP（dist/2026-10-05，commit 672e666/dirty=true）不得复用** |
| S1 | 隔离数据目录准备 | `tools/acceptance/sp30_prepare_datadir.ps1` → `V2RAYN_R_DATA_DIR=%TEMP%\sp30_accept_<guid>` 空目录 | 目录新建、为空、与用户真实数据目录无关 | 目录非空/路径指向用户目录即停 |
| S2 | 首次使用（普通入口） | 解压 S0 包到隔离目录，双击 **`v2rayn_desktop.exe`**（无 AUTO_SMOKE、无预置 active）；对照 `tools/release/r4_32_real_entry.ps1` 第 1 阶段 | 窗口出现；首跑 `guiNDB.db` 在隔离目录创建；截图 | 无窗口/库落错目录：取进程退出码 + 截图 + 数据目录清单存档 |
| S3 | 导入合成订阅 | 正式 UI：订阅分组入口（`lib/features/subs/subs_actions.dart`，订阅设置/编辑窗）从文件导入 `fixtures/acceptance/sp30/synthetic-sub.txt` | 4 节点入库，remarks=`sp30-synth-01/02/03/ss`；失败行不半写（事务语义） | 数量/名称不符、半写：导出 DB 节点清单 diff 存档 |
| S4 | 选择/应用 | 正式 UI：服务器列表设为活动（`lib/features/profiles/profile_actions.dart`、`table_actions.dart`）→ 真实 FRB 应用路径（`smoke_windows.ps1 -WithSeed` 同款 loopback apply，不碰 10808） | desired/applied revision 分离可查；回环端口 ≥11808 可探测 | revision 混淆/占用 10808：立即停，记 journal + 端口快照 |
| S5 | 备份 | 正式 UI：备份/恢复视图（`lib/features/backup/backup_and_restore_view.dart`）→ 备份 zip 写隔离目录外取证区，记 SHA256 | 备份文件生成且 hash 记录；含 S3/S4 状态 | 备份失败/内容缺节点：保留数据目录 + 错误弹窗截图 |
| S6 | 重开 | 仅停止本脚本启动的 owned PID 树；同 `V2RAYN_R_DATA_DIR` 重新双击普通入口 | 节点/活动选择/设置与 S5 前一致（配置 diff 为空） | 不一致：新旧 DB 节点清单 diff + 启动日志存档 |
| S7 | 恢复 | 清空数据目录 → 正式 UI 恢复视图导入 S5 备份 | 恢复后与 S5 备份内容一致；journal 无残留半写 | 恢复失败/半写：保留现场目录，blocked 登记 |

## 约束重申

合成数据；不触发真实网络/OS 副作用；10808、宿主代理、路由、TUN、Run-key
零触碰；只清理 owned 进程/目录；端口先探测且 ≥11808。

## 当前不可执行的前置（已登记，见 observations.json blocker）

1. 新候选包须从基线 3635392 干净重建（`tools/release/build_windows.ps1`），
   现场仍有运行中 `v2rayn_desktop.exe`(23292)/`net_host.exe`(46108)——重建前须先
   经 owner 正常退出（禁按名批杀），且重建需关闭正在运行的 exe。
2. S2–S7 真实 GUI/重开需授权隔离机（宿主为日常使用机，不做真实 OS 副作用）。
3. SP-29 门禁 fail+incomplete 未清：真实验收前复核前置卡当前证据。
