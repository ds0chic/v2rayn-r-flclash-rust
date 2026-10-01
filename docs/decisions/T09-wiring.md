# ADR T09-wiring — 订阅接线层的架构决策

状态：accepted（implemented）。
关联：`docs/evidence/T09-wiring.md`、`docs/decisions/T09-fmt.md`、plan §15、AGENTS.md。

## 背景

T09 库（`crates/subscriptions`）已提供纯 `parse_content`/`refresh`/`download` 与 Fmt 编解码，
但尚未接入应用层（持久化、用例、UI）。本 ADR 记录把它接进
`application → bridge_api → Flutter` 的取舍。

## 决策

### D1. `SubItem` 归 application 层，桥接层只做 DTO 映射
`SubItem`（17 字段）落在 `crates/application/src/subs.rs`，不在 `domain`（它是订阅专属、
非节点业务），也不在 `subscriptions`（该 crate 保持纯编解码/解析/合并，不拥有持久化模型）。
理由：保持 `subscriptions` 无数据库耦合，同时让 application 复用其纯函数。

### D2. 持久化沿用既有 `Store`，不新增连接
`SqliteSubRepository` 与 `SqliteProfileRepository` 共享同一 `Store`（即同一 SQLite 连接与
单写者约束，plan §04）。`Store::read_rows("SubItem")`/`query_rows`/`upsert_row`/`execute`
已按列名映射，直接复用，避免第二条写路径。

### D3. 候选集先构建，失败/空保留旧节点
`refresh_one` 的顺序：下载（含 MoreUrl 合并）→ `build_candidates`（解析 + Filter 过滤 +
`KeepOlderDedupl` 去重）→ 仅当候选非空才 `replace_sub_profiles`（按 subid 先删后写）。
下载失败、解析为空、过滤后为空一律保留旧节点并返回结构化错误（F-SUB-011）。
与上游 `ProcessDownloadResult`/`AddBatchServers` 的“空结果提前返回、成功先删后写”一致。

### D4. 更新是 job，取消 token 贯通 subscriptions
`update_subscriptions` 在 application 的 `JobManager` 注册 job 并取其 `CancellationToken`，
贯通到 `download_all`/`parse_content`（库层每项安全点检查）。`cancel_job`（既有 API）对应用户
点击取消。返回的 `job_id` 暴露给 UI，状态经 `job_view` 读取。取消语义与 plan §8 一致：
安全点前可停，不伪造即时完成。

### D5. 经代理更新用显式本地端口，不读环境
`AppEngine` 持有 `local_proxy_port`（由运行会话通过 `set_local_proxy_port` 注入），
`local_proxy_url()` 生成显式 `http://127.0.0.1:PORT` 交给下载器；无运行代理时 `via_proxy`
退化为直连（下载器 `download_with_fallback` 的直连重试）。**不读取环境代理变量**（库层
`no_proxy()` 已保证），符合 plan §15 “不让网络库环境变量随机决定走向”。

### D6. 调度为独立可停止任务，不阻塞退出
`SubScheduler` 用 `tokio::spawn` 跑 detached 循环，`stop()` 置标志并 `notify` 唤醒立即退出。
`is_due` 复刻上游 `now - UpdateTime >= interval*60`（interval ≤ 0 或未启用则跳过）。
调度默认不在应用启动时拉起（见 evidence §6.6），此为有意的保守选择，直到端到端验证完成。

### D7. `to_inner_uri` 作为库层薄导出
`subscriptions::fmt::mod` 新增 `to_inner_uri(&[Profile]) -> Option<String>`，直接包装既有
`inner::emit`，不改行为。避免在桥接层复制 InnerFmt 逻辑。

### D8. 分享二维码新增 `qr_flutter`
`F-IMPORT-010` 的二维码渲染需要 QR 编码；选择 `qr_flutter`（纯 Dart、无原生依赖）。
扫码/图片导入（F-IMPORT-006/007）本轮不做，避免引入摄像头/图像管线依赖，登记为未决。

### D9. 测试用本地 loopback HTTP，端口 ≥11808
Rust 管线测试自建 `TcpListener`（11808 起），合成 Base64/URI 列表；绝不触碰 127.0.0.1:10808。
Flutter 测试用 `SyntheticBridgePort` 内存实现 + 注入剪贴板，不加载原生库。

## 影响

- 新增公共面：`application::subs::*`、`SubRepository`/`SubStore`、`AppEngine` 订阅用例；
  `bridge_api::api::subs::*`（FRB 生成 Dart 面）。
- 未改 `crates/subscriptions` 既有行为（仅加导出）、`config_codegen`、`persistence` 既有测试、
  `net_host`、`runtime`。
- 未 commit（按任务要求）。

## 未采纳的替代

- 把 `SubItem` 放 domain：拒绝，其非节点领域概念，会污染 domain 边界。
- 在桥接层重写解析/合并：拒绝，违背复用 T09 库与“不重复定义协议”的约束。
- 调度启动即自启：暂缓，避免未验证的自动网络行为进入 release 默认路径。
