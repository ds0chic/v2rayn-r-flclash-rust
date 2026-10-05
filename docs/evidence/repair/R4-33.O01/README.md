# R4-33.O01 Windows x64 / WPF — 证据

- 实例：Windows x64，上游界面 WPF（`v2rayN/v2rayN/v2rayN.csproj`）。
- 固定 HEAD：`8651e19d1049d69e27951c98e4884560d56f4c97`；应用基线 `77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`。
- `armed=false`；本卡无生产改动、未启核、未写宿主系统代理/TUN/路由/注册表、未占用 `127.0.0.1:10808`。本实例证据全部引用既有真机/打包记录，未新跑破坏性 OS 操作。

## 结论

状态：**blocked**（部分 implemented）。

适用流程清单与逐项状态（完成条件 = GUI 实际构建/安装/运行/核/代理/TUN 权限/窗口/热键/流量全适用项 verified）：

| 适用流程 | 状态 | 证据 |
|---|---|---|
| GUI 实际构建 | verified（构建范围） | `docs/evidence/T20.md`：`cargo build --workspace --release --locked` + `flutter build windows --release` 均 exit 0 |
| 打包产物启动/数据目录/窗口 | verified（打包冒烟范围） | `docs/evidence/T20.md`：干净临时目录解压启动，生成 `guiNConfig.json`/`guiNNDB.db`，退出清理 |
| 代理（核心 apply 链） | verified（AUTO_SMOKE 等价路径） | `docs/evidence/T20.md`：种子 + `V2RAYN_R_AUTO_SMOKE=1` → net_host 写 `config.json` → 打包 Xray 启动 → ready → 11808 监听 → journal `stage=applied` |
| 核 | verified | `docs/evidence/recheck-fixes/R3-CORE-MATRIX/`：14 核 version + 真实最小会话（hysteria2/overtls 会话已解除） |
| 系统代理 / 自启 / helper 路由 / TUN 适配器创建销毁 | verified（当前用户/管理员会话，安全范围） | `docs/evidence/T21-real-os.md`：注册表读写复原、Run 键增删、helper 路由增删、sing-box `auto_route=false` 适配器创建销毁 |
| 真实 auto-route TUN / 全局接管 | **blocked** | 需授权隔离 VM；`compat/platform-matrix.md` §6.2“未验证” |
| GUI 鼠标点击驱动的 apply 时序 | 未验证 | T20 由 `V2RAYN_R_AUTO_SMOKE` 等价路径驱动，非鼠标点击自动化 |
| R4-32 未武装包交付回归（同 commit/hash、P0/P1 零、依赖闭包/升级卸载、首次启动核下载真实请求） | **blocked（交付未完成）** | `docs/evidence/repair/R4-32/` 仅见 `build-windows.log`、`flutter-build-windows-release.log`、`pre-cleanup-orphans.json`，缺 README/observations/门禁结论/固定哈希；`docs/repair/tasks/R4-32.md` 仍 identified（非完成报告） |
| 代码签名 / 安装器 / 自动更新真跑 / 升级卸载 / DPI 矩阵逐项 | 未运行 | `docs/evidence/T20.md` 明确排除 |

## 与 R4-32 的合并口径

- R4-32 是 Windows 阶段包交付卡（全部 Rust/Flutter 门禁、FRB no-diff、同 commit/hash、P0/P1 零、不 AUTO_SMOKE 绕按钮）。其证据目录仅含部分构建日志、缺完成性材料与门禁结论，故本实例不能提升 `verified`。
- 本实例引用的 Windows 真机证据均早于/独立于 R4-32，只证明对应子流程，不替代 R4-32 完整交付回归。
- 合并判据：R4-32 产出证据后，本实例可直接复用其哈希与门禁结果；在此之前维持 blocked。

## 解除条件（最小验收步骤）

1. 交付 R4-32：跑通全 Rust/Flutter 门禁，产出 `docs/evidence/repair/R4-32/`（同 commit/hash、P0/P1 零）。
2. 授权隔离 VM：快照基线 → 从真实 UI 开关触发 TUN（非 `V2RAYN_R_AUTO_SMOKE`）→ 记录设备出现/helper 地址路由/`ready`/默认路由与连通性 → 关闭断言适配器+路由+lease+journal 全回收 → 崩溃/睡眠唤醒恢复 → 复位快照。
3. 补 GUI 鼠标点击驱动的首次使用/失败恢复/订阅更新回归。
