# R3-CORE-MATRIX — 其余 12 核版本锁定与真实运行矩阵

状态：`verified`（14 核 version 探针全部通过；14 核真实最小会话 **14 `verified`、0 `blocked`**——hysteria2/overtls 已用自签 TLS 服务器对 + 客户端最小会话 + 代理 GET 与失败探针解除 blocker；adapter 合同已按真实二进制修正并补测）。

任务 ID：R3-CORE-MATRIX（对应 `docs/evidence/recheck-fixes/remaining-boundaries-2026-10-05.md` 第 3 项）。

本次唯一用户流程：应用需能发现/启动冻结 `CoreInfoManager` 清单中的每个代理核；因此逐核选官方 windows-amd64 资产、钉版本/来源/哈希，并按 adapter 合同跑 `version` 与真实最小回环会话（先探测 ≥11808 端口），确认真实二进制被正确调用、监听与清理。

前置任务及已验证证据：冻结 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始 HEAD `55e6d10`。`docs/tasks/R3-03.md` 完成 14 核 adapter 候选与 env/args 合同；本卡在其基础上做真实资产运行验收。

上游对照：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `ServiceLib/Manager/CoreInfoManager.cs`（逐核 `CoreExes`/`Arguments`/`VersionArg`/`Environment`/`AbsolutePath`、`GetCoreUrl`）、`ServiceLib/Global.cs:649-665`（`CoreUrls`）、`ServiceLib/Manager/CoreManager.cs:334-350`（工作目录=`binConfigs`、env `string.Format`）、`ServiceLib/Handler/CoreConfigHandler.cs:10-42`（仅 v2ray/singbox/mihomo 生成配置，其余核为 Custom 复制）、`ServiceLib/Common/Utils.cs:1272-1284`（`GetBinConfigPath`）。

对应 feature / field / action：RR-06、RR-07；`CoreType` 14 代理核；`tools/cores/cores.lock.json`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`tools/cores/cores.lock.json` 逐核资产定义；合成回环配置（不接真实节点）。
- 输出：`tools/cores/<core>/<version>/` 下的官方可执行文件；锁文件逐条 hash/来源；adapter 的 exe/args/env/workdir。
- 错误：版本探针超时或非零退出 → 该核 `blocked`；无服务器最小会话失败 → 会话 `blocked`（version 仍可 `verified`）。
- 取消：单条下载/命令超 15 分钟终止；连续两次异常输出 blocker。
- 权限：普通权限；不写系统代理/注册表/路由/TUN；只停本脚本启动的 PID。
- 持久化：锁文件（可审阅 diff）；下载物 gitignore。
- 生效：`CoreLocator` 通过 `CoreInstallLayout` 的 `<root>/<dir>/<version>/<exe>` 解析到钉版本；net-host 按 adapter 合同 spawn。

允许修改的模块：`tools/cores/cores.lock.json`、`tools/cores/**`、`tools/**`（辅助脚本）、`crates/runtime/src/adapter.rs`、`crates/runtime/tests/**`、本卡、`docs/evidence/recheck-fixes/R3-CORE-MATRIX/**`、`compat/` 台账（仅追加）。

禁止改变的已有行为：不删枚举、不降分母、不伪造可用性；`main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`apps/desktop/**`、`features/**`、`crates/application/**`、`crates/updater/**`、`services/**`、`work/**`、`outputs/**` 不改；不跑全仓 fmt / `flutter build windows`。

测试夹具和原版预期：每核合成配置仅用回环地址与 `127.0.0.1:1`/测试端口，不含任何真实节点或凭据。原版预期：核心清单、exe 名、args/env 与冻结 `CoreInfoManager` 一致；版本可执行。

本次必须通过的命令/真实场景：
- `tools/cores/fetch_cores.ps1`（12 核官方资产下载/解压/哈希）
- `tools/cores/smoke_cores.ps1`（14 核 version）：14/14 exit 0
- `tools/cores/session_matrix.ps1`（14 核真实最小会话）：14 ok / 0 blocked（hysteria2/overtls 走自签 TLS 对会话 + 代理 GET），逐核清理无残留
- `cargo fmt -p runtime -- --check`、`cargo clippy -p runtime --all-targets --locked -- -D warnings`
- `cargo test -p runtime --locked`、`cargo test -p net_host --locked`

证据文件位置：`docs/evidence/recheck-fixes/R3-CORE-MATRIX/`（`README.md`、`per-core.json`、`logs/smoke-results.json`、`logs/session-results.json`、`logs/cores.lock.snapshot.json`）。

完成条件：14 核官方资产钉版本/来源/哈希并可审阅；version 全通过；真实最小会话可跑的核均 ok 且清理正常，不可跑的核如实 `blocked` 并给原因；adapter 合同以真实二进制为准修正且补测；不删枚举/不降分母。

接口缺口（登记）：
- `V2RAY_LOCATION_ASSET`/`XRAY_LOCATION_ASSET`/`XRAY_LOCATION_CERT` 上游指向 exe 目录，而 `CoreAdapter::env_vars` 仅接收配置路径，无法表达 exe 目录；当前未注入（本机 geo 资产与 exe 同目录）。
- `run_config_check` 执行 `test_args` 时不注入 `env_vars`/`working_dir`；当前受影响核无碍，后续依赖 env/cwd 的校验命令需同步。

本轮实际结果：`cores.lock.json` 扩为 14 条（追加 12 条；5 核上游校验一致、7 核本地计算）；`fetch_cores.ps1`/`smoke_cores.ps1`/`session_matrix.ps1` 新增；`adapter.rs` 修正 v2fly（`-config`/`-test -config`）、v2fly_v5（去 `-format jsonv5`，`run -c`/`test -c`）、hysteria/hysteria2（`working_dir`=配置目录）并新增 3 个单测。runtime 58 passed、net_host 62 passed；fmt/clippy 通过。

hysteria2/overtls 补测（`session_matrix.ps1` 新增 pair 会话路径）：在 `$TEMP` 用锁定 hysteria2 的 `cert` 生成临时自签 PEM（只记 cert sha256 / pin，不落私钥），`server` 显式起 TLS 监听（hysteria2 UDP，overtls TCP），客户端按 adapter 合同连接并代理真实 HTTP GET 到本地目标（回体 `HY2-OK`/`OV-OK`），同时跑“指向关闭端口”的失败探针（hysteria2 QUIC 握手超时 exit 1、overtls `ConnectionRefused` exit 1）。14 核会话结果：14 ok / 0 blocked；全部进程按 PID 停止、`still_running=false`、临时物仅存 `$TEMP`。overtls `--help` 证实本地 `-r server` 模式存在；adapter 只实现 client 角色，未改。

blocked（已解除）：hysteria2、overtls 的“含本地自签服务器最小会话”均已完成，矩阵 14/14。

未完成 / 下一步前置：未跑全仓 `cargo test --workspace`（仅运行受影响的 runtime 与 net_host）；xray/sing-box lock 未重下。
