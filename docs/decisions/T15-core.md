# T15 内核观测适配决策记录（字段对照与限流）

- 关联方案：`outputs/V2RAYN_FLUTTER_RUST_PLAN.md`（观测/统计/Clash 面板）。
- 关联台账：`compat/features.yaml` F-MONITOR-001..006（日志、统计、历史、Clash 代理/连接、实时速度）、
  F-TEST-*（结果模型）；`compat/fields.yaml` SpeedTestItem、ClashUIItem。
- 上游只读来源（commit 7d6a967）：
  - `ServiceLib/Services/Statistics/StatisticsXrayService.cs`
  - `ServiceLib/Services/Statistics/StatisticsSingboxService.cs`
  - `ServiceLib/Manager/StatisticsManager.cs`
  - `ServiceLib/Manager/ClashApiManager.cs`
  - `ServiceLib/Models/CoreConfigs/V2rayMetricsVars.cs`
  - `ServiceLib/Models/Dto/{ServerSpeedItem,ClashItem,ClashConnections,ClashConnectionModel,SpeedTestResult}.cs`
  - `ServiceLib/Helper/HttpClientHelper.cs`
  - 生成侧：`Services/CoreConfig/V2ray/V2rayStatisticService.cs`、`.../Singbox/SingboxStatisticService.cs`

## 1) 字段路径逐条对照

| 上游 | 本 crate | 说明/差异 |
|---|---|---|
| `StatisticsXrayService.Url = http://127.0.0.1:{StatePort}/debug/vars` | `XrayStatsSource::new(port, ..)` | 同路径；`StatePort` 由调用方解析后传入 |
| `V2rayMetricsVars.stats.outbound`（`Hashtable`） | `parse_xray_vars` 读 `stats.outbound` 对象 | 上游用 Hashtable 容忍任意键；本实现读为 `HashMap<String, XrayLink>` |
| `V2rayMetricsVarsLink.downlink/uplink`（`long`） | `XrayLink { downlink: i64, uplink: i64 }` | 同 JSON 小写键名；负值 clamp 到 0 再转 u64 |
| `state.uplink / linkBase`（`linkBase=1024`） | `LINK_BASE`、`to_display_units()` | 原始 u64 保留字节；显示单位由调用方按需调用 |
| `key.StartsWith(Global.ProxyTag="proxy")` → ProxyUp/Down | `classify_tag()` → `TagClass::Proxy` | 前缀匹配一致 |
| `key == Global.DirectTag="direct"` → DirectUp/Down | `classify_tag()` → `TagClass::Direct` | 精确匹配一致 |
| `PeriodicTimer(1s)` | `StatsAggregator::one_hz()` | 1 Hz 限流 |
| `_serverSpeedItem` 差值（本次累计 - 上次累计） | `apply()` 的 baseline 差值 + bps | 语义等价；用 u64 防回绕 |
| `if (server.DirectDown < prev \|\| server.ProxyDown < prev) { 重置; return null }` | 观测下降 → `generation += 1` 清基线 | 上游静默重置，本实现显式暴露 generation |
| `StatisticsSingboxService.Url = ws://127.0.0.1:{StatePort2}/traffic` | `SingboxTrafficSource` + `TrafficConfig::new(port)` | 同路径 |
| `TrafficItem.Up/Down`（`ulong`） | `parse_traffic` → `Option<(u64,u64)>` | 同时接受 `up/down` 与上游 C# 的 `Up/Down` |
| `ProxyUp = (long)(up / 1000)`（SI 单位） | 内部累加原始字节，`CounterSample.up` 暴露字节 | 除以 1000 的显示换算留给调用方（Xray 却用 1024，故不统一换算） |
| 断流后 `Task.Delay(3000)` 重连 | `TrafficConfig.initial_backoff=3s`，指数退避封顶 30s | 上游固定 3s；本实现可退避，更省资源 |

Clash API（`ClashApiManager`）：

| 上游 | 本 crate |
|---|---|
| `GET {ApiUrl}/proxies` + `/providers/proxies` 合并 | `get_proxies()`（并发 + 合并 + provider 索引） |
| `ClashItem.ProviderIndexMap` | `ClashItem.provider_index_map` |
| `ClashProxy { all, history, name, type, udp, now, delay }` | `ClashProxy`（`type`→`proxy_type`，其余同名/同义） |
| `ClashProxy.HistoryItem { time, delay }` | `HistoryItem { time: Option<String>, delay: i32 }` |
| `GET /proxies/{name}/delay?timeout=10000&url=` | `get_proxy_delay/try_get_proxy_delay`（`DEFAULT_DELAY_TIMEOUT_MS=10000`） |
| `GET /providers/proxies/{p}/{n}/healthcheck?timeout=&url=` | `try_get_provider_proxy_delay` |
| `delay` 非数字 → -1 | `delay_value()` 用 `Value::as_i64` + `i32::try_from`，否则 -1 |
| `GET /connections` | `get_connections()` |
| `DELETE /connections/{id}`、`DELETE /connections` | `close_connection`、`close_all_connections` |
| `ClashConnections { downloadTotal, uploadTotal, connections }` | `ClashConnections`（同 camelCase 重命名） |
| `ConnectionItem { id, metadata, upload, download, start, chains, rule, rulePayload }` | 同名；`start` 上游 `DateTime` → 本 crate `Option<String>`（避免引入时间库） |
| `MetadataItem.uid: object?`（实测 number 或 string） | `uid: Option<Value>` |
| `HttpClientHelper` 全局缓存 token/查询地址符号 | 不使用 create 默认头；仅 secret 用 `Authorization: Bearer`（见 §3） |

结果模型（`SpeedTestResult` / `ServerSpeedItem`）：

- `SpeedTestItem.*` 归 `config_codegen`/domain 的配置模型，T15 不落地。
- `ServerSpeedItem { ProxyUp/Down, DirectUp/Down, Today*, Total*, IndexId }` 属于持久化统计（F-MONITOR-002/003），
  由 `StatisticsManager` + `ServerStatItem` 承担，本 crate 只提供 `CounterSample`/`TagStat` 上游数据；
  `SpeedTestResult { IndexId, Delay, Speed, IpInfo }` 是测速结果模型，非观测适配范围。

## 2) 决策 D1 —— Xray 用增量，sing-box 用累计归一

Xray `/debug/vars` 给的是自进程启动以来的**累计**计数；sing-box `/traffic` 给的是**每周期增量**。
两者都归一为 `CounterSample`（累计语义），因此：

- Xray：直接读累计值，差值即速率。
- sing-box：在采集器内把增量**累加**成累计量（`saturating_add`），再走同一条聚合路径。

好处：UI 只需一条读取路径；sing-box 的进程内累加天然规避了「上游清零」的歧义。

## 3) 决策 D2 —— secret 头

- Xray metrics 端点无鉴权；上游 `StatisticsXrayService` 也不带 secret。
- Clash 控制器（sing-box `clash_api`/mihomo）以 `secret` 鉴权；上游 `HttpClientHelper` 未做统一 secret，
  但面板依赖控制器可访问。
- 本实现统一约定 `Authorization: Bearer <secret>`（`ClashApiClient::new(port, secret, ..)`），
  并对 401 单独归为 `HttpError::Unauthorized`（连接失败/超时/解析各自独立）。

## 4) 决策 D3 —— 1 Hz 限流与「不丢量」

上游每 tick 计算「本次累计 − 上次累计」并写入持久化。若 UI 因节流丢弃中间 tick，直接丢弃样本会**丢字节**。

本实现的 `apply()`：

1. 以「上次**被应用**的样本」为基线，计算 delta；即使中间样本被限流，只要下一被应用样本到达，
   delta 仍覆盖整段窗口（`throttle_does_not_lose_accumulated_bytes` 用例验证）。
2. 速率 = delta / 距离上次应用的真实时间窗（纳秒级整除，`u128` 中间量避免大计数溢出）。
3. `generation` 变化总是立即应用并重置基线，避免重启后上一个大累计值造成假的大 delta。

限流计数 `throttled_count()` 可观测；`applied_count()` 统计真正更新 UI 的次数。

## 5) 决策 D4 —— 日志环：控制事件与日志行分离

`F-MONITOR-001` 要求“不丢控制事件”。单环无法同时保证“日志有界”与“控制不丢”，故：

- `RingBuffer` 只放日志行，行数/字节双上限，溢出显式计数。
- `LogBuffer` 另用一条有界的控制事件队列（默认 256）承载 `ProcessStarted/ProcessExit/StreamError`。
  日志洪泛只能挤掉日志行，**托不掉**控制事件；控制队列自身溢出也计数 `control_dropped()`，不静默丢弃。

## 6) 决策 D5 —— 全部客户端 `no_proxy`

本机宿主配置了系统代理（用户正在运行 10808）。若 `reqwest` 走默认代理检测，环回控制请求可能被系统代理截获，
既不安全也不确定。因此 `XrayStatsSource`/`ClashApiClient` 一律 `reqwest::Client::builder().no_proxy()`；
真实 Xray 冒烟中另观察到 `NO_PROXY`/`no_proxy` 环境变量也会影响 `curl`/`Invoke-WebRequest`，测试仅用环回直连。

## 7) 决策 D6 —— 连接失败分类的长超时

本机 Windows 环回对关闭端口返回 RST 前有约 2s 的 SYN 重试（实测），因此「连接失败」用例的 client 超时设为 5s，
以区分 `Connect` 与 `Timeout`。生产默认超时按调用方传入，保持可控。
