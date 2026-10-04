# R3-CORE-MATRIX 证据（2026-10-05）

基线：冻结 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `55e6d10`。
执行环境：Windows 11 x64 25H2，PowerShell 7.5.4；Rust `C:\Users\Colby\.cargo\bin\cargo.exe`。
边界：未占用/修改 `127.0.0.1:10808`（会话端口全部先探测，≥11808）；未改宿主系统代理/注册表/路由/TUN；未读用户凭据，会话配置全为合成回环数据；只停止本脚本启动并记录 PID 的进程（每核 `stopped=true`/`still_running=false`）。

## 交付范围

把其余 12 核（v2fly、v2fly_v5、mihomo、hysteria、naiveproxy、tuic、juicity、hysteria2、brook、overtls、shadowquic、mieru）按冻结 `CoreInfoManager.cs` 的清单，逐核选 windows-amd64 官方资产、钉版本/来源/哈希、跑 `version`/真实运行、修正 adapter 合同。共形成 14 核矩阵（含已有 xray、sing-box）。

## 逐核矩阵

| 核 | 版本 | sha256 校验 | version | 真实最小会话 | 结论 |
|---|---|---|---|---|---|
| xray | 26.3.27 | 上游 .dgst 一致 | ok | ok（监听 11808，已清理） | verified（既有） |
| sing-box | 1.14.2 | 本地计算 | ok | ok | verified（既有） |
| v2fly | 4.45.2 | 上游 .dgst 一致 | ok | ok（`-config`） | verified |
| v2fly_v5 | 5.53.0 | 上游 .dgst 一致 | ok | ok（`run -c`） | verified |
| mihomo | 1.19.32 | 本地计算 | ok | ok（`-f`/`-d`） | verified |
| hysteria (v1) | 1.3.5 | 本地计算 | ok | ok（`lazy_start:true`） | verified |
| naiveproxy | 154.0.8037.49-2 | 本地计算 | ok | ok（positional） | verified |
| tuic | 1.0.0 | 上游 .sha256sum 一致 | ok | ok（`-c`） | verified |
| juicity | 0.5.0 | 上游 .dgst 一致 | ok | ok（`run -c`） | verified |
| hysteria2 | 2.12.3 | 本地计算 | ok | ok（自签 TLS server+client，代理 GET=HY2-OK） | verified |
| brook | 20270101 | 本地计算 | ok | ok（配置=brook 命令脚本） | verified |
| overtls | 0.3.15 | 本地计算 | ok | ok（自签 TLS server+client，代理 GET=OV-OK） | verified |
| shadowquic | 0.4.0 | 本地计算 | ok | ok（YAML direct） | verified |
| mieru | 3.38.0 | 上游 .sha256.txt 一致 | ok | ok（`MIERU_CONFIG_JSON_FILE`） | verified |

会话汇总：14 核 version 全部 exit 0；14 核真实最小会话 **14 ok、0 blocked**；所有启动进程均已按 PID 停止，无残留（`stillRunning=false`）。

## 版本锁定

`tools/cores/cores.lock.json` 扩展为 14 条（schema 兼容，追加 xray/sing-box 之后的 12 条），逐条含 `release_tag`/`core_version`/`asset`/`asset_size_bytes`/`source`（官方 URL）/`sha256`/`sha256_verified`/`extracted_to`/`executable`/`executable_sha256`/`platform`。下载与哈希脚本：`tools/cores/fetch_cores.ps1`（生成可审阅片段 `tools/cores/cores.lock.generated.json`）。校验规则：有上游校验侧车（`.dgst`/`.sha256sum`/`.sha256.txt`）的 5 核（v2fly、v2fly_v5、juicity、tuic、mieru）`sha256_verified=true`；其余 7 核上游未发布校验资产，标 `sha256_verified=false` 并写明“本地计算”。

下载物位于 `tools/cores/<core>/<version>/`（`.exe`/`.zip` 已被 `.gitignore` 忽略）。

## 真实会话（脚本与结果）

脚本：`tools/cores/session_matrix.ps1`（`tools/cores/smoke_cores.ps1` 为 version 冒烟）。每核：先绑 127.0.0.1 探测 ≥11808 空闲端口 → 写合成回环配置 → 起真实进程（按 adapter 合同 args/env/cwd）→ 轮询监听 → 按 PID 停止 → 校验退出/无残留。
结果文件：`logs/smoke-results.json`、`logs/session-results.json`、`logs/cores.lock.snapshot.json`。

ceiling 与坑：
- **v2fly v4**：真实 `4.45.2` 忽略裸定位配置路径（上游 `Arguments="{0}"`），回退到 `<exe 目录>/config.json`；必须 `-config {0}`。
- **v2fly_v5**：真实 `5.53.0` 拒绝上游 `-format jsonv5`（切到 v5 原生加载器后报 `unknown field "type"/"loglevel"`）；`run -c`/`test -c` 能自动识别生成器产出的 v4 兼容 schema，故去掉该 flag。
- **hysteria/hysteria2**：上游 `Arguments=""`，核从“工作目录”解析 `config.json`；adapter 需 `working_dir=config 目录`。v1 加 `lazy_start:true` 可在无服务器时绑定 SOCKS5 监听；v2 **无子命令时恒为 client 模式**（不按配置自动判定），故 v2 server 必须用显式 `server -c`（仅 harness 使用，adapter 客户端合同仍是 `args=[]`+cwd）。v2 自签对起后可正常建连并转发。
- **hysteria2/overtls 自签对会话**：harness 用锁定 hysteria2 的 `cert` 子命令在 `$TEMP` 生成临时自签 PEM（只记录 cert 文件 sha256 与 hysteria pinSHA256，绝不记录私钥）。hysteria2 server 走 UDP（`Get-NetUDPEndpoint` 判定监听）；overtls server 走 TCP。两核均先跑“客户端指向关闭端口”的失败探针（hysteria2 QUIC 握手超时 exit 1；overtls `ConnectionRefused` exit 1），再跑成功对会话并经客户端 SOCKS5 发真实 HTTP GET 到本地目标（回体 `HY2-OK`/`OV-OK`）。
- **brook**：上游 `Arguments=" {0}"`（绝对路径）实测是把“配置文件当作 brook 命令脚本”读取；内容 `socks5 --listen 127.0.0.1:PORT` 即起监听，合同成立。
- **hysteria v1 误导日志**：运行期出现 `[version:app/v2.12.3] New version available ... HyNetworks/hysteria`，是 v1.3.5 自身的更新检查，不是二进制版本；实际 `-v` 与哈希均为 v1.3.5。
- **tuic**：relay `server`/`local server` 均须 `host:port`；relay 认证字段是 `uuid`+`password`（无 `token`）。
- **shadowquic**：配置为 YAML；`inbounds/outbounds` + `router.default-outbound: direct` 可无服务器起监听。

## adapter 修正（crates/runtime/src/adapter.rs）

- `V2flyAdapter`：`run_args` 裸定位 → `["-config", cfg]`；`test_args` 版本探针 → `["-test","-config",cfg]`。
- `V2flyV5Adapter`：`run_args` 去掉 `-format jsonv5` → `["run","-c",cfg]`；`test_args` → `["test","-c",cfg]`。
- `HysteriaAdapter` / `Hysteria2Adapter`：新增 `working_dir = config 父目录`。
- 未删除任何枚举、未降低分母；未改动 `install_layout.rs`（不在允许清单内）。
- 补测（`cargo test -p runtime`）：新增 `v2fly_uses_the_explicit_config_flag`、`v2fly_v5_uses_run_and_test_without_jsonv5_format`、`hysteria_cores_run_from_the_config_directory`；runtime 58 passed / 0 failed。

## 命令与结果

- `tools/cores/fetch_cores.ps1`：12 核官方资产下载+解压+哈希，全部成功。
- `tools/cores/smoke_cores.ps1`：14 核 version 探针，14/14 exit 0。
- `tools/cores/session_matrix.ps1`：14 核真实最小会话，14 ok / 0 blocked，逐核清理无残留；hysteria2/overtls 走自签 TLS 对会话（server+client+代理 GET）。
- `cargo fmt -p runtime -- --check`：exit 0。
- `cargo clippy -p runtime --all-targets --locked -- -D warnings`：exit 0，0 警告。
- `cargo test -p runtime --locked`：58 passed / 0 failed。
- `cargo test -p net_host --locked`：62 passed / 0 failed。

## 上游对照结论

- 上游仅对 v2rayN/Xray/mihomo/sing-box 定义 `DownloadUrl*` 并参与自动更新（`GetCheckUpdateCoreTypes`）；其余核只有 `Url`，需人工提供 Custom 配置，`CoreConfigHandler.cs:16-21/44-83` 将用户自定义文件复制为 `config.json`。
- 逐核 `CoreExes`/`Arguments`/`VersionArg`/`Environment`/`AbsolutePath` 与冻结 `CoreInfoManager.cs` 一致或已按真实二进制修正（见上）。
- 发现上游 v2fly/v2fly_v5 的 `Arguments` 在真实 4.45.2/5.53.0 上不可用（裸路径 / `-format jsonv5`），本实现以真实二进制为准做最小修正并补测。

## blocked 清单（已全部解除）

- hysteria2：**已解除**。用锁定 hysteria2 的 `cert` 在 `$TEMP` 生成临时自签证书，显式 `server -c` 起 UDP 监听，客户端按 adapter 合同（`args=[]`+cwd=配置目录）连接成功，SOCKS5 监听 11809，经其代理的 HTTP GET 回体 `HY2-OK`；失败探针（指向关闭端口）exit 1。进程全部按 PID 停止，无残留。
- overtls：**已解除**。`--help` 证实 `-r, --role <server|client>` 支持本地 server 模式；自签证书起 `-r server -c` TCP 监听，客户端 `-r client -c` 连接成功，mixed 监听 11809，经其代理的 HTTP GET 回体 `OV-OK`；失败探针 `ConnectionRefused` exit 1。进程全部按 PID 停止，无残留。

两核 version 探针均通过；此前“合成回环无服务器”所记 blocker 已由自签对会话解除，矩阵 14/14。

## 接口缺口（登记）

- `V2ray`/`Xray` 的 `V2RAY_LOCATION_ASSET`/`XRAY_LOCATION_ASSET`/`XRAY_LOCATION_CERT` 指向的是“可执行文件目录”（`Utils.GetBinPath("")`），而 `CoreAdapter::env_vars` 只能拿到配置路径，无法表达 exe 目录；当前未注入（本机 geo 资产与 exe 同目录，最小会话不受影响）。如需严格对齐，需要契约提供 exe 路径或工作目录注入。
- `run_config_check`（`services/net_host/src/session.rs`）执行 `test_args` 时不注入 `env_vars`/`working_dir`；对当前 v2fly/v2fly_v5 的 `-test`/`test` 无影响，但若后续某些核的校验命令依赖 env/cwd，需要该路径同步。

## 未完成 / 下一步前置

- hysteria2、overtls 的“含服务器最小会话”已完成（自签 TLS 对会话 + 代理 GET + 失败探针），矩阵 14/14。
- xray/sing-box 的 lock 条目沿用既有版本，未重新下载；其 sha256 校验状态维持原状。
- adapter 合同已按实测修正并补测；未跑全仓 `cargo test --workspace`（仅运行受影响的 runtime 与 net_host）。
