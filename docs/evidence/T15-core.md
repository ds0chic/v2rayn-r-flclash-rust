# T15 内核观测适配证据（crates/core_adapters）

- 任务：T15-core（统计 / Clash API 客户端 / 日志流）
- 范围：仅 `crates/core_adapters/**` + `docs/evidence/T15-core.md` + `docs/decisions/T15-core.md`；
  未改 apps、compat、work/、outputs/，未改其他 crate 源码。
- 例外声明：`Cargo.lock` 被 `cargo` 自动更新（新增 tokio-tungstenite/tiny_http/urlencoding/sha1 等条目），
  这是 `--locked` 门禁能通过的前提；除此之外未手工改动锁文件。用户已授权。
- 环境：Windows 11 x64，Rust 1.98.1（MSVC）。本机有用户正在运行的 xray（`Desktop\v2rayN-windows-64\bin\xray\xray.exe`，PID 4072）
  与代理端口 10808；本回合所有 mock/真实内核端口均 ≥ 11808，未触碰 10808。

## 1) API 概览

### 1.1 `core_adapters::stats`

- `CounterSample { tag: String, up: u64, down: u64 }` —— 统一的**累计**字节快照（u64 宽计数）。
- `TagClass { Proxy, Direct, Other }` + `classify_tag()`：对齐 `StatisticsXrayService`
  的 `key.StartsWith(ProxyTag)/key==DirectTag`。
- `trait StatsSource: Send`
  - `poll() -> BoxFuture<Result<Vec<CounterSample>, StatsError>>`（对象安全的轮询快照）
  - `generation() -> u64`：底层计数被「重置/回绕」时递增；调用方据此丢弃基线。
- `XrayStatsSource`
  - `new(port, timeout)` / `from_url(url, timeout)` / `with_secret(secret)`（`Authorization: Bearer`）
  - `GET http://127.0.0.1:{StatePort}/debug/vars`，解析 `stats.outbound[tag].{uplink,downlink}`
  - 观测到同 tag 计数下降 → `generation += 1` 并清基线（进程重启计数清零语义）
  - `parse_xray_vars(&str)` 独立可测；`to_display_units()` 复刻 `/linkBase(1024)`
- `SingboxTrafficSource`
  - 构造于 `tokio::runtime::Handle::current()`；`Drop` 自动取消；`cancel()` 幂等
  - `ws://127.0.0.1:{StatePort2}/traffic` 流式增量，内部累加成累计量；暴露 `proxy` 桶
  - 断流→指数退避重连（默认 3s 起、封顶 30s，对齐上游 3s）；`is_closed()`/`snapshot()`
  - `parse_traffic()` 接受 `{"up":..,"down":..}` 与上游 C# 大小写变体 `{"Up":..,"Down":..}`
- `StatsAggregator`
  - `new(min_interval)` / `one_hz()`；`apply(&samples, generation, now) -> bool`（false = 被限流）
  - 每 tag 输出 `TagStat { tag, up_total, down_total, up_bps, down_bps }`
  - 限流不丢量：速率基于「上次**应用**的基线」计算；generation 变更立即应用并重置
  - `applied_count()` / `throttled_count()` / `stats()`
- `StatsError { Http(..), WebSocket(..), Decode(..), Cancelled }`

### 1.2 `core_adapters::clash_api`

- `ClashApiClient::new(port, secret, timeout)` / `from_base(base, secret, timeout)`
- 方法：
  - `get_proxies()`：并发 `GET /proxies` + `GET /providers/proxies` 并合并（provider 条目回填 `provider_index_map`）
  - `get_proxies_with_retry(attempts, delay)`：对齐上游 3 次重试/2s
  - `get_proxy(name)`：`GET /proxies/{name}`（`urlencoding` 编码段）
  - `get_proxy_delay(name, timeout_ms, url) -> i32`（失败返回 -1）、`try_get_proxy_delay()`（带错误）
  - `try_get_provider_proxy_delay(provider, name, timeout_ms, url)` → `/providers/proxies/{p}/{n}/healthcheck`
  - `get_connections()`：`GET /connections`
  - `close_connection(id)`：`DELETE /connections/{id}`；`close_all_connections()`：`DELETE /connections`
  - `get_config()`：`GET /configs` 原样 `serde_json::Value`
- 模型（对照上游 ClashApiManager/ClashItem/ClashConnections/ClashConnectionModel）：
  `ClashProxy`、`HistoryItem`、`ClashProvider`、`ClashProxies`、`ClashProviders`、`ClashItem`、
  `ClashConnections`、`ConnectionItem`、`MetadataItem`。
- 错误分类：`ClashError::Http(HttpError)`，`HttpError { Connect, Timeout, Unauthorized, Status(u16), Decode }`。

### 1.3 `core_adapters::log_stream`

- `LogLevel { Trace..Fatal, Unknown }`（`Ord`；Unknown 永不被过滤隐藏）；`detect_level()` 支持
  `[Info]`、`level=debug`、`{"level":"fatal"}`、首 token 等。
- `LineReader<R: AsyncRead>` + `next_line()`：按行（LF/CRLF），`max_line_bytes` 硬上限 + `truncated_lines()` 计数。
- `RingBuffer`：行数/字节双上限、`dropped_lines()`/`dropped_bytes()` 溢出可见；单行超总预算即丢弃并计数。
- `LogFilter { min_level, include, exclude }` + `accept()`：exclude 优先，include 任一命中，级别过滤保留 Unknown。
- `ControlEvent { ProcessStarted, ProcessExit(i32), StreamError(String) }`、`LogEvent`
- `LogBuffer`：`RingBuffer`（可丢行）+ 独立控制事件队列（`with_control_cap`，溢出记 `control_dropped()`）。

## 2) 测试统计（`cargo test -p core_adapters`）

真实运行（mock HTTP 用 `tiny_http`、mock WS 用 `tokio_tungstenite::accept_async`，全部绑定 `127.0.0.1` 且端口 ≥ 11808）：

| 测试文件 | 用例数 |
|---|---|
| `tests/stats_xray.rs` | 13 |
| `tests/stats_singbox.rs` | 11 |
| `tests/stats_aggregator.rs` | 8 |
| `tests/clash_api.rs` | 17 |
| `tests/log_stream.rs` | 17 |
| `tests/xray_smoke.rs`（`#[ignore]`，手动跑） | 1（ignored） |
| **合计（常规）** | **66 通过 / 0 失败**；另有 1 个 ignored 冒烟 |

覆盖点：解析、401/状态码/超时/连接失败/解析失败分类、Xray 计数回退→generation、sing-box 增量累加/坏帧容忍/
断流重连/取消（活动流与待连状态）/落盘、限流不丢量、u64 宽计数、ring 行/字节上限与溢出计数、
控制事件在日志洪泛下不丢、日志级别/关键字过滤。

## 3) 与上游字段对照要点

- 见 `docs/decisions/T15-core.md` §1（字段路径逐条对照）。
- 关键差异均在上游同构范围内，并在决策文档中标注（`uid: Value`、`start: String`、`history.delay: i32`）。

## 4) 真实 Xray 冒烟结果（已跑）

- 命令：`cargo test -p core_adapters --test xray_smoke -- --ignored --nocapture`
- 内核：`tools/cores/xray/v26.3.27/xray.exe`（`Xray 26.3.27 ... d2758a0`）
- 启动命令：`xray.exe run -c <tempdir>\xray-config.json`
- 生成流量：最小 SOCKS 入站 `127.0.0.1:<socks>` → `freedom` 出站 `proxy`；脚本经 SOCKS5 连到 metrics 端口拉 `/debug/vars`。
- 结果：`smoke samples: [CounterSample { tag: "direct", up: 0, down: 0 }, CounterSample { tag: "proxy", up: 210, down: 13318 }]`，

- 机器产物（2026-10-01 实跑归档）：`docs/evidence/T15.runs/`——`stdout.log`（完整输出，PID 38920，proxy up=210/down=13402，3.54s 通过）、`manifest.json`（命令/时间/PID/端口/版本/清理）、`xray-config.template.json`（配置模板，端口运行时填入）。两端均为 127.0.0.1 且端口 ≥11808； spawning 的 xray 进程均已 kill/wait 回收，仅用户自有 PID 4072 存续（未触碰）。
  `source.generation() == 0`，测试通过；子进程 PID（如 40728）由 `kill_on_drop` + 显式 `kill` 清理。
- 清理核验：测试后无本项目 xray/sing-box 残留进程（仅剩用户自己的 PID 4072，未触碰）。
- 超时：整段包在 `tokio::time::timeout(60s)`。
- 另做两次人工抓形：确认空闲时 `stats.outbound` 仍列出 `direct`/`proxy`（计数 0），
  与上游 `ParseOutput` 假设一致（先读源码再实现）。

## 5) 未决项

- sing-box 真实冒烟未跑：真实 `/traffic` WS 抓包列为可选，本回合只做 mock WS；未验证真实 sing-box 1.14.2 的
  帧结构/连接行为（mock 依据上游 `StatisticsSingboxService` 与 `TrafficItem` 建模）。→ 记为 identified/未验证。
- sing-box `/traffic` 上游未传 secret；若某构建要求 secret 头，当前实现未提供（Xray 路径有 `with_secret`）。
- `get_proxies_with_retry` 的「空但成功」策略：当前「成功即返回（含空）」；上游是「两侧都为 null 才重试」。
  若后续需要严格对齐空列表重试语义，需接线层传入预期。
- `XrayStatsSource` 与 `SingboxTrafficSource` 均以 `proxy`/`direct` 归类为主；每 tag 明细通过 `CounterSample.tag` 暴露，
  但聚合器未按 `TagClass` 分桶（UI 若要 proxy/direct 汇总需自行按 `classify_tag` 聚合）。
- 本 crate 未接入任何进程管理/IPC；真实内核的启动/停止由 `net_host`/T03 负责，T15 仅消费端口。
