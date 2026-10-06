# SP-07 事件落后与重连对账 — 证据

基线 `92d46dd`。缺陷依据 CP-17（`docs/evidence/complete-port-audit-2026-10-06/runtime/README.md` §RUN-06）：
`server.rs` 转发循环把可恢复 `Lagged` 当 EOF 静默结束订阅，且 `EventStreamOpened` 原样回显客户端传来的
`epoch`/`from_seq`；客户端断连才重连，无对账、无快照、无订阅起点更新。

## 改动（仅文件锁内）

- `services/net_host/src/events.rs`：`RESYNC_REQUIRED_EVENT`（`resync_required`，控制通道，
  无 IPC/DTO 变更）、`control_payload`/`parse_event_identity`（payload 内 `operation_id`/`generation`
  身份，envelope `epoch`/`seq` 仍为排序权威）、`emit_control`（生产者合同，见下阻塞）、
  `resync_notice`/`resync_notice_attributed`（携带 `reason`/`epoch`/`last_seq` + 最后已知控制身份）。
- `services/net_host/src/server.rs`：`resync_needed`（epoch 失配/`from_seq` 越过权威 tip 即判
  ResyncRequired）、`classify_recv`（Lagged→Resync，永不作 EOF）、forwarder 在 Lagged 后发
  attributed resync 通知并**关闭该订阅**（无静默半开）、`EventStreamOpened` 返回权威
  `(epoch, last_seq+1)` 不再回显、`SubscribeEvents` 请求 stale 时先发 resync 通知。
- `crates/application/src/net_host_client.rs`：`EventCursor`（gap/epoch 变更/resync 信号→
  `ResyncAsk`，游标保持到快照到达，旧 generation 永不覆盖）、`LifecycleLatch`（仅控制事件推进
  保留结果，遥测只计数）、`parse_event_identity`（镜像解析）、`event_loop` 接线：
  每次（重）订阅采用权威起点，Resync→`GetSnapshot`→`adopt_origin` 续订，读循环结束（断连或
  服务端关闭 resync 订阅）同样先快照再重连。

## 先红后修

红：新测试引用尚不存在的 API，`cargo test -p net_host` 编译失败（`emit_control`/
`resync_notice`/`resync_needed`/`classify_recv`/`EventCursor` 等未找到）。
修后绿（见下）。另发现并修正了测试自身对 broadcast 语义的误设：Lagged 后继续写入会再次
挤掉旧游标，慢订阅者须用容忍重复 Lagged 的排空循环——此即生产饥饿语义，也是关闭订阅+快照
兜底的依据。

## 定向检查（干净 worktree `92d46dd` + 仅本卡 patch，主仓他人改动未混入）

| 命令 | exit |
|---|---|
| `cargo test -p net_host --locked`（90 passed，含 9 个 SP-07 新测试） | 0 |
| `cargo test -p application --locked --lib`（295 passed，含 6 个 SP-07 新测试） | 0 |
| `cargo clippy -p net_host -p application --all-targets --locked -- -D warnings` | 0 |
| `cargo fmt --all -- --check`（本卡 3 文件无 diff） | 0 |

`flutter analyze`：未动 Dart，不适用。跨命名管道压力实测：未运行（见阻塞）。
端口：11808 探测 FREE（本卡测试全内存 broadcast/纯逻辑，无 socket 绑定）；
10808 未触碰；宿主代理/路由/TUN/DNS/Run-key 无写入；合成数据 only。

## 未验证 / 阻塞

1. 主仓工作树内另一 agent（SP-12/SP-15）在 `crates/platform/src/sysproxy/mod.rs`
   的未提交改动 currently 不编译（`applied_content_hash` 作用域错误），导致主仓内无法直接
   `cargo test -p application`；本卡 application 验证在干净 worktree 完成，主仓文件经
   SHA256 确认为同一验证态。需该文件 owner 修复，不在本卡锁内。
2. `emit_control` 的生产发射器（`session.rs` reconcile 路径）需该文件 owner（SP-06）接入；
   精确签名需求：`EventBus::emit_control(kind: EventKind, operation_id: Option<&str>,
   generation: u64, body: Value) -> EventEnvelope` 已存在可直接调用，无 DTO/bridge 变更。
3. 真实跨 pipe 慢订阅 burst 与 GUI 消费者对账（lag 通知→snapshot→续订端到端）未实测；
   本卡状态为 implemented，不标 verified。
