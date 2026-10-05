# R4-09 真实数据分页与增量 — 证据

状态：implemented（本卡范围的可运行测试通过；`flutter build windows --release` 未运行，见阻塞）。

- 固定 commit：139fbea（执行时工作树含并行代理改动，见下）
- armed=false；未启动内核、未占用 10808、未改宿主代理/路由/TUN/自启；测试无真实端口/网络。
- 夹具：合成 ProfileSummary/ProfileDto（`test/r4_09_contract_test.dart`、`crates/application/src/synthetic.rs`），无用户凭据。
- 平台/DPI/内核：未涉及原生运行，未实测。

## 缺陷与修复

D09（`queryAllProfiles` 单页 pageSize=100000 丢弃 nextCursor，>10 万节点消失）
- `FrbBridgePort.queryAllProfiles` 改为跟随 Rust `query_profiles` 的真实 cursor，按 `kProfileQueryPageSize=500` 有界分页循环；不再请求 100000，不再丢弃 `nextCursor`。
- 新增 `BridgePort.querySummaryPage` / `queryAllSummaries`（真实 cursor 语义）与 `ProfileSummaryPage`。

D08（`bridge_port.dart:354-395` 同步全量读；`profiles_controller.dart:498-503` 再读一次；测速每 150ms reload）
- 新增 `BridgePort.fetchProfileSnapshot(count)`：一次 cursor 读返回“排序后基础行 + 完整 DTO”，`build()`/`reload()` 由“两次全量读”降为一次。
- 新增 `BridgePort.applyLiveOverlay(base)`：只把实时测速结果/流量统计叠加到缓存的基础行。
- `ProfilesController` 缓存 `_baseSummaries`；150ms 轮询改调 `_refreshLive()`（仅 overlay + 本地 recompute），不再重读节点表；切组/筛选/排序只做本地 recompute，不触发存储读。
- 新增 `search()` 查询代际守卫：新查询递增 `_queryGeneration`，旧的在途结果被丢弃，不会覆盖新结果。

## 命令与结果

- `flutter analyze`（全项目）：EXIT=1，仅 `lib/features/routing/routing_windows.dart:2061` unused_element 与 `:2070` use_null_aware_elements（R4-14 路由代理的文件，本卡禁改）。本卡 4 个改动文件单独 analyze：`No issues found!` EXIT=0。
- `flutter test test/r4_09_contract_test.dart`：7 passed / 0 failed。
- `cargo fmt --all -- --check`：无 diff（PASS）。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：Finished，无 error/warning（PASS）。
- `cargo test --workspace --locked`：除 `crates/application/tests/t18b_runtime_e2e.rs` 两个真实内核/本地服务 e2e（`routing_domain_rule_direct_vs_block_differs`、`xray_socks_and_http_outbounds_reach_local_service`）失败外全部通过；新增 `store_repo::tests::paging_follows_cursor_over_ten_thousand_rows` ok（10k 行 500/页，无截断、游标推进）。两个失败与 R4-09 无关（无真实 xray 环境）。
- `flutter build windows --release`：未运行。

## 未完成 / 接口缺口 / 阻塞

- `flutter analyze` 全项目 EXIT=1 由并行 R4-14 路由文件的 warning/info 造成；不在本卡所有权范围，未修改。
- `cargo test --workspace` 两个 t18b 运行时 e2e 失败，需真实内核/本地服务，非本卡改动。
- `flutter build windows --release` 未运行（需全项目 analyze 通过且无本项目实例锁 exe）。
- 真正的“行级虚拟化数据窗口”（仅加载可视区页面而非全量 visible 列表）未实现；当前 UI 渲染已虚拟化，内存受 `kProfileQueryPageSize` 分页读取与单份 visible 列表约束。切组释放/复用、结果稳定排序由现有 `_recompute`/`applySort` 保证。
- FRB：未新增 Rust `#[frb]` 函数/字段，未改 `frb_generated`，无需重生成。
