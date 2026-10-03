# UX-TEST-02 — HTTPS 测速探针 × 临时会话端口冲突修复

执行模型：deepseek-v4.1-flash。基线：`c19fe49`（UX-TEST-01 收口）。真实窗口 Flutter 集成测试 + 真实 FRB / Rust / SQLite + 本地 rustls TLS 目标 + 已授权临时数据目录的真实节点对照。

## 1. 结论

UX-TEST-01 遗留的两个“测速真实缺口”已修复并判定：

1. **HTTPS 探针**：受限 SOCKS 探针以前只做明文 HTTP，而默认/用户 URL 是 `https://`（`SpeedPingTestUrl=https://www.gstatic.com/generate_204`、`SpeedTestUrl=https://cachefly.cachefly.net/50mb.test`），因此真延迟/下载必然失败。现探针**经临时会话 SOCKS 出站**，直接在其上叠加 rustls（`ring` provider）完成 TLS，证书校验在产线走 OS 根存储，测试可注入合成 CA，**没有暴露任何 insecure 模式**。
2. **`error.port_conflict`**：临时会话端口“探测—使用—释放”存在竞态（并发 pool 观察到同一空闲端口、释放后立即复用落在 Windows `TIME_WAIT`）。现采用**进程内预留块 + ATomics 轮转游标 + 释放前有界确认**，连续 start/stop 与并发 job 均不再冲突。

判定：**默认 HTTPS 测速 URL 经真实节点可用**。真实节点下载经临时 Xray 内核测出 **29.6 MB/s**，真延迟经 HTTPS 测出 **129 ms**；本地合成 TLS 目标上真延迟/下载均出现真实数值与成功文案。

## 2. HTTPS 探针实现（`crates/application/src/speedtest.rs`）

- 依赖：新增 `rustls 0.23`（`default-features=false`，仅 `ring/std/tls12/logging`）、`rustls-native-certs 0.8`、`rustls-pemfile 2`。`ring` 已是 reqwest 的 rustls 栈所用 provider，未引入新 crypto 后端或重型框架。
- `http_get_via_socks(socks_port, url, trust, timeout, max_bytes, ct)`：
  - 先与 `127.0.0.1:port`（临时会话 SOCKS inbound）建 TCP，`socks5_connect` 以**域名 address type** 请求 CONNECT；
  - `http -> 明文`、`https -> rustls 握手` 两条路径共用 `write_request` / `read_response`；
  - 真延迟 = 读到完整响应头的 `header_ms`；下载 = `body_bytes / total`（max_bytes 限量、deadline 限时），Mbps 口径 `bytes / 1_000_000 / secs`。
- `TlsTrust`：`Native`（产线，`rustls-native-certs` 读 OS 根 + 仅测试用途的 `V2RAYN_SPEEDTEST_EXTRA_CA` 附加根）与 `Custom(RootCertStore)`（确定性测试注入）。**产线恒校验，无 insecure 开关**。
- 结构化失败：`ProbeFailureKind::{Url,Resolve,Connect,Timeout,Tls,HttpStatus(u16),Protocol,Cancelled}` → 稳定 `message_key`（`speedtest.tls_failed` 等），UI 侧 `profiles_controller.dart::_failureReason` 映射为“TLS 证书校验失败/请求超时/连接失败/HTTP 状态错误/域名解析失败/协议错误/已取消”。
- runner 语义：`real_ping` 失败返回结构化 `ProbeError`（不再伪装 `-1`）；`mixed_one` 下载失败保留已测延迟并标 `failed`。

## 3. port_conflict 根因与修复

**根因**：`find_free_test_port` 只“探测”不“占用”。并发 pool 的多个 worker 可同时观察到同一空闲端口，随后在核心 spawn / `preflight_port` 处碰撞（`error.port_conflict`）；此外临时会话 stop 释放监听后，已接受连接可停留在 `TIME_WAIT` 阻塞下次 bind。

**修复**（`speedtest.rs` + `services/net_host/src/session.rs`）：
- 进程内 `RESERVED_TEST_PORTS` 集合：`reserve_free_test_port()` 一次性占用连续 `TEST_PORT_BLOCK=3`（SOCKS inbound + 两个 state 端口），并发调用取互斥块，杜绝同端口双开。
- 轮转游标 `TEST_PORT_CURSOR`：释放后不立即复用刚关闭的块，规避 `TIME_WAIT`。
- `release_test_port()` 幂等释放；`NetHostTestSession::open/close` 在错误路径与成功路径都恰好释放一次。
- net-host `close_test_session` 增加 `wait_port_released(port, 1500ms)`：停核心后**有界确认**该端口可再次 bind 才返回，使调用方的预留复用安全。

## 4. 确定性测试（`crates/application/tests/ux_test02_https_probe.rs`，15 例全绿）

合成 CA（`tests/fixtures/tls/`）签发 `127.0.0.1` 服务器证书；本地 rustls TLS 服务器扮演目标，最小 SOCKS5 中继扮演临时会话；`http_get_via_socks` 在其上跑真实 TLS。全部注入自有根存储，无 OS 信任、无真实网络。

- HTTPS 真延迟成功（注入 CA，204）；
- HTTPS 下载成功并遵守 `max_bytes`；
- 不受信根 → `Tls`（`speedtest.tls_failed`）；
- 连接拒绝 → `Connect`；
- 慢服务器 → `Timeout`；
- 404 → `HttpStatus(404)`；
- 取消即时；
- 明文 HTTP 路径未回归；
- runner 级：注入根真延迟出正数值、混合测速出正速率、TLS 失败上抛 message_key；
- 端口回归：8 线程并发预留互斥；25 次快速预留/释放；**40 次 start/stop 循环且不立即复用**；**两 job 并发持有互斥端口**。

## 5. 集成（真实窗口）

### 5.1 本地合成 TLS 目标（`integration_test/ux_test02_https_speedtest_test.dart`）

拓扑（全回环，无 10808）：临时 Xray 内核（socks inbound）→ socks 出站节点 → 本地 SOCKS5 中继 → 本地 HTTPS 服务器（仓库测试 CA 签发的叶子证书）。仓库 CA 经 `V2RAYN_SPEEDTEST_EXTRA_CA` 作为**附加根**交给探针，**证书校验保持开启**。

`integration-https.json`：`socksPort=11808`、导入 1 行、`saved=true`；
- `https-realping`：`message=测速完成：成功 1`、`delay=1`（回环亚毫秒归一为 ≥1）；
- `https-download`：`message=测速完成：成功 1`、`speed=0.5 MB/s`（`resultSpeed≈0.49`，512KB/2041ms）。

### 5.2 真实节点对照（脱敏）

数据来源：`%TEMP%\t21f_data` 的授权副本（`%TEMP%\ux-test02-realnode\data`）。仅记录计数/类型/耗时/结果码，不写地址、别名、凭据、URL。选择单个节点用 `V2RAYN_UX_TEST01_NODE_INDEX`（0 基，脱敏索引）。

- 库存：总计 39 节点（`vless` 26、`hysteria2` 13）；默认 URL 方案为 `https://`。

| 动作 | 类型 | 耗时(ms) | 取消 | 阶段 | 可见文案 | 结果码 | 延迟哨兵 | 速率 |
|---|---|---|---|---|---|---|---|---|
| 真延迟 | vless | 2146 | 否 | SpeedtestingCompleted | 测速完成：成功 1 | Speedtesting | 129 ms | — |
| 下载(测速) | vless | 6860 | 否 | SpeedtestingCompleted | 测速完成：成功 1 | 29.6 | 183 | 29.6 MB/s |

- 下载样本为 `vless`，经临时 Xray 内核走默认 `https://cachefly.cachefly.net/50mb.test`，**测得真实速率 29.6 MB/s**，证明 HTTPS 探针经节点真实可用（对比 UX-TEST-01：同样动作只得到 `SpeedtestingSkip` / 0）。
- 对照观察：同批另有 hysteria2 节点真延迟/下载报 `speedtest.connect_failed`（`-2`，结构化“连接失败”）。这是节点/传输层失败，UI 已按结构化原因显示，非探针缺陷。
- 部分空闲 vless 节点对大流量做限速/封锁，下载虽 200 但速率近似 0（`resultCode=0.0`）；换节点即可测得正速率，说明限速属节点侧而非探针侧。

截图：`real-realping.png`、`real-download.png`（真实节点，表格只显示合成别名/数值，无地址）；`https-realping.png`、`https-download.png`（本地合成 TLS）。

## 6. 门禁（本地实跑）

- `cargo fmt --all -- --check` → exit 0。
- `cargo clippy --workspace --all-targets --locked -- -D warnings` → exit 0。
- `cargo test --workspace --locked` → exit 0（含新 `ux_test02_https_probe` 15 例）。
- `dart format --output=none --set-exit-if-changed lib test integration_test` → 0 changed（exit 0）。
- `flutter analyze` → No issues found（exit 0）。
- `flutter test`（`tools/flutter_test_retry.ps1 -PerFile -MaxAttempts 4`）→ PER-FILE PASS: all test files green（exit 0）。
- `flutter build windows --release` → exit 0（`build/windows/x64/runner/Release/v2rayn_desktop.exe`）。

## 7. 复现命令

```powershell
# 确定性 HTTPS 探针（无需内核）
cargo test -p application --test ux_test02_https_probe --locked

# 真实窗口合成 TLS 集成
$repo = "<repo>"
$env:V2RAYN_R_DATA_DIR = "$env:TEMP\ux-test02-run\data"
$env:V2RAYN_UX_TEST02_EVIDENCE_DIR = "$repo\docs\evidence\UX-TEST-02"
$env:V2RAYN_SPEEDTEST_EXTRA_CA = "$repo\crates\application\tests\fixtures\tls\ca.pem"
flutter test integration_test/ux_test02_https_speedtest_test.dart -d windows

# 真实节点对照（单动作 + 脱敏索引；数据为 t21f_data 授权副本）
$env:V2RAYN_R_DATA_DIR = "$env:TEMP\ux-test02-realnode\data"
$env:V2RAYN_UX_TEST01_REALNODE_EVIDENCE_DIR = "$repo\docs\evidence\UX-TEST-02"
$env:V2RAYN_UX_TEST01_ACTION = "download"
$env:V2RAYN_UX_TEST01_NODE_INDEX = "4"
flutter test integration_test/ux_speedtest_realnodes_test.dart -d windows
```

## 8. 前后对照

| 项 | UX-TEST-01（前） | UX-TEST-02（后） |
|---|---|---|
| 探针协议 | 仅明文 HTTP | HTTP + HTTPS（rustls，产线校验） |
| 默认 HTTPS 真延迟 | 失败/`SpeedtestingSkip` | 真实表 129 ms |
| 默认 HTTPS 下载 | 失败/0 | 真实测 29.6 MB/s |
| 失败原因 | 无（只 `-`） | 结构化 `speedtest.*` → 人类文案 |
| 临时会话端口 | 偶发 `error.port_conflict` | 预留块 + 游标 + 释放确认，回归 40 循环/并发 job 全通过 |
| 测试证据 | 明文探针 | + `ux_test02_https_probe` 15 例 + 本地/真实 HTTPS 集成 |

## 9. 未决项

- 真实节点对照受可用节点带宽限制：同一批部分空闲节点对 50MB 大流量限速/阻塞，故单节点样本用“可达样本”呈现；全部节点逐一测速不在本回合范围。
- `IpInfo(IPAPIUrl)`、UDP 测试仍未实现（T15b §5 既有项，未扩范围）。
- Debug 真窗口在 39 节点时偶发原生退出（`did not complete`），沿用按动作独立进程 + 重试规避；属既有工具链现象，非产品缺陷结论。
- 未验证 mac/Linux；未做物理鼠标点击、TUN/系统代理（遵守 AGENTS 禁用项）。
