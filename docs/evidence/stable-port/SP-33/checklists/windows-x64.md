# SP-33 实例 O01：Windows x64 —— 验收清单（available，本机可执行）

前置：pinned Flutter 3.47.5 / Rust 1.98.1 / FRB 2.13.0（AGENTS.md 锁定路径）；
隔离数据目录（`V2RAYN_R_DATA_DIR=%TEMP%\sp33_accept_<guid>`，空目录，与用户数据无关）；
14 核二进制在位（沿用 SP-28 core-matrix 14 条，不捆绑进包）。

| # | 动作 | 真实入口 | 通过判定 |
|---|---|---|---|
| P0 | 包身份 | `sp33_launch_prep.ps1 -Zip <基线393fafd干净重建包>` | exit 0：sha 记录；flat 四 exe 齐；无内核捆绑；`smoke_armed=false`；`git_dirty=false`；`commit==393fafd` |
| P1 | 端口探测 | `-ProbePorts` | 11808..11815 空闲；10808 未触碰 |
| P2 | 首次启动（普通入口） | 双击 `v2rayn_desktop.exe`（无 AUTO_SMOKE、无预置 active） | 主窗口出现；隔离目录生成 `guiNConfig.json`（惰性）+ `guiNNDB.db`；截图 |
| P3 | 合成导入 | 正式 UI 订阅入口导入 `fixtures/acceptance/sp33/synthetic-sub.txt` | 4 节点入库 remarks=`sp33-synth-*`；失败行不半写 |
| P4 | 选择/应用 | 正式 UI 设活动 → 真实 FRB apply（回环端口 ≥11808） | desired/applied revision 分离可查；journal `stage=applied`（pid/config_sha256/rev） |
| P5 | 托盘/热键/窗口/DPI | 正式托盘菜单、全局热键 pause、主窗口三种布局、DPI 缩放 | 与 WPF/Avalonia 基线行为一致（平台矩阵 §3）；记录差异不降低分母 |
| P6 | 更新/清理 | 更新检查（不真实升级）+ 退出后 owned PID/端口/目录清理 | 无残留；journal 无半写 |
| P7 | 备份/重开/恢复 | 同 SP-30 S5–S7（备份 zip 记 hash → 同目录重开一致 → 清空后恢复一致） | 配置 diff 为空 |

约束：合成数据；10808、宿主代理/路由/TUN/DNS/Run-key 零触碰；只停 owned PID；
签名/安装器/自动更新真实执行不在本轮（登记未验证）。
