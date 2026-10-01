# T15a 监控管线接线决策（统计 / 日志 / Clash API / 状态栏）

- 依据：`outputs/V2RAYN_FLUTTER_RUST_PLAN.md` §14（事件限流：流量 1Hz、日志 50–100ms 批、
  连接页可见时订阅）、§15（统计跨日/清空语义）。
- 台账：`compat/features.yaml` F-MONITOR-001..006；`compat/fields.entities.yaml`
  FLD-ENT-110..115（ServerStatItem）；`compat/actions.yaml` ACT-MAIN-030；
  `compat/layouts.yaml` LAY-MSG-001 / LAY-CLASHPROXY-001 / LAY-CLASHCN-001。
- 上游只读源码：`ServiceLib/Manager/{StatisticsManager,ClashApiManager}.cs`、
  `ServiceLib/Services/Statistics/*`、`ServiceLib/ViewModels/{MsgViewModel,ClashProxiesViewModel,ClashConnectionsViewModel,StatusBarViewModel}.cs`、
  `v2rayN/Views/{MsgView,ClashProxiesView,ClashConnectionsView,StatusBarView}.*`。

## D1 —— 累计字节永远不被 1Hz 节流丢弃

`StatsService::apply` 在**每次**收到快照时先按 tag 差分并累加到会话/节点计数，再调用
`StatsAggregator`（1Hz）决定是否刷新“速率/事件”。因此节流只影响 UI 刷新频率，不会少算字节。
代次（core 重启）变化时清空会话基线与会话计数，但保留已持久化的 `ServerStatItem` 累计值。
跨日（`DateNow` 变化）只把 `TodayUp/Down` 归零，`TotalUp/Down` 结转。

## D2 —— 日志环与控制事件分通道

复用 `core_adapters::log_stream`：`RingBuffer`（默认 10,000 行或 10MiB，先到为准）承载批量日志，
`LogBuffer` 内独立控制队列承载进程生命周期标记。日志洪峰只会淘汰旧日志并计数（`dropped_lines`/
`dropped_bytes`/`truncated_lines` 对 UI 可见），控制事件不被挤掉。采集暂停与滚动暂停分离。

## D3 —— 日志来源：net-host 事件而非文件 tail

net-host 的 stdout/stderr 读取器在写会话日志文件的同时，发 `log_line` 命名事件
（`services/net_host/src/{events,session}.rs` 的最小改动：`EventBus` 变为可克隆、读取器持有克隆）。
应用层 `ingest_runtime_event` 只挂在 engine 的单一事件 sink 上，随 `subscribe_events` 一并激活，
不新增第二条 IPC 订阅。已有 `log_batch` 载荷与 `log_line` 均可消费。

## D4 —— Clash API 只读 + 选择/关闭，非 mihomo/sing-box 明确“不适用”

`ClashApiService::supported` 仅对 `Mihomo` / `SingBox` 为真；其他内核返回
“当前内核不提供 Clash API”，不伪造空表。读/延迟/连接沿用 `core_adapters::clash_api`；
代理组选中需要 `PUT /proxies/{group}`，为此在 `core_adapters::clash_api::ClashApiClient`
**新增**一个 `select_proxy` 方法（纯增量，未改动既有读逻辑与既有测试）。
延迟探测只在用户动作时发起，单节点与整组（组内 provider 走 healthcheck）。

## D5 —— 桥接事件带 epoch/seq，页不可见不订阅

`crates/bridge_api/src/api/monitor.rs` 的 hub 为每次推送分配 `(epoch, seq)`；
`subscribe_traffic` / `subscribe_logs` 先回放头部（日志回放当前环，最多 2000 行）。
`set_page_visible` 记录页可见性，只有可见页保持订阅（连接页可见时才拉取连接）。
实时轮询（Xray `/debug/vars`、sing-box `/traffic`）由 `monitor_start_polling` 显式开启，
默认（测试/普通渲染）不建立任何 socket。

## D6 —— 边界

- 不实现测速（T15b）、TUN、系统代理；不占用 10808；所有测试端口 ≥11808。
- 真实数据不经过 Flutter/FRB；UI 只消费桥接 DTO。
- `core_adapters` 仅新增 `select_proxy`（增量），既有测试逻辑未改。
