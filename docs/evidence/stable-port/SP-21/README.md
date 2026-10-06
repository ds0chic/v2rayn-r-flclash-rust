# SP-21 准备与数据层证据（ identifies / 部分 implemented，未完成 ）

卡状态：**identified**（数据层 + UI 消费层部分 implemented；完整卡未完成，不伪造完成）。
基线：`92d46dd`。本次不 commit（worktree 保持 uncommitted，与其它并行卡共存）。

## 2026-10-07 continuation（本次）

### 新增 Rust 真实 SQLite 规模证明（`crates/application/src/store_repo.rs`）

prep 测试走 `:memory:` 直调 `query_page`；本次新增 4 个文件型 SQLite
（WAL、合成行、无网络）经后台 `AsyncPageWorker` 连接的测试：

- `sp21_real_sqlite_10k_worker_walk_covers_every_row_once`：10k 行全量
  worker walk（500 行/页，20 页），断言有界页/cursor 推进/无重复遗漏。
- `sp21_real_sqlite_100k_worker_walk_covers_every_row_once`：100k 行全量
  worker walk（500 行/页，200 页），同上。
- `sp21_real_sqlite_id_tiebreak_is_stable_across_worker_pages`：5000 行同
  Remarks 经 worker 跨页，断言全程 `IndexId` 回退序。
- `sp21_real_sqlite_worker_stale_revision_and_cancel`：真实文件上首
  页→写提交→旧 revision 下一页得 `StaleCursor`；预取消 token 得
  `Cancelled`；`QueryHandle::cancel()` 同步 `<1s` 且迟到结果丢弃。
- 跨页全选 = walk 收集的 id 全集恰为库全集（10k/100k 两 walk 内断言，
  排序后逐项相等）。

### 新增 UI 消费接线（仅 `apps/desktop/lib/features/profiles/**`，未碰 FRB）

- `ProfilesController.reloadPaged({pageSize})`：经既有
  `BridgePort.querySummaryPage` 游标 API 的增量结构加载；页间让出事件循环
  （timer/输入/取消可插入）、generation 守卫（迟到页丢弃）、revision 变化
  重查（上限 5 次防活锁）、逐页发布 + `pagedLoading` 标志（owner generation
  持有，supersede 不串旗、cancel 无后继时自清）。
- `ProfilesController.selectAllAcrossPages({pageSize})`：游标走完后取
  walked ∩ 当前 view（与 Ctrl+A 同语义，窗口化后仍正确）；取消/过期保持原
  选择不动。
- 取消真接线：`setGroupSubId` / `resyncGroupFromSubs` / `setFilter` /
  `updateFilterInput`（clear）/ `submitFilter` / `sortBy` / `sortByResult` /
  `search` / `reload` 均 bump `_pagePager` generation —— 组切换/筛选/排序
  在同步语义不变下取消在途 paged walk（widget 无需改动，chips/filter/sort
  本就走这些入口）。
- `test/sp21_paged_load_test.dart`：6 用例（全量流/取消丢弃/世代取代/
  组切换取消/跨页全选/跨页全选取消保持）。
- 结构读本就走真实游标：`reload()` 经 `queryAllProfiles` 跟随
  `engine.queryProfiles` 游标到 exhaust（D09）；本次是其异步消费形态。

### 本次定向检查

| 命令 | 结果 |
|---|---|
| `cargo fmt -p persistence -p application -- --check` | 通过（0 diff） |
| `cargo clippy -p persistence -p application --all-targets --locked -- -D warnings` | 通过 exit 0 |
| `cargo test -p persistence -p application --locked` | 通过：application lib **346 passed / 0 failed**（含 12 个 `sp21_*`），persistence lib 74/0，全部集成 target 0 failed |
| `cargo test -p application --locked --lib sp21_ -- --nocapture` | 12/12 通过 |
| `dart format --output=none --set-exit-if-changed lib/features/profiles test/sp21_paged_load_test.dart test/sp21_async_page_test.dart` | 通过（0 changed） |
| `flutter analyze` | **No issues found** |
| `flutter test test/sp21_paged_load_test.dart test/sp21_async_page_test.dart` | **10/10 通过**（新 6 + 旧 4） |
| 回归（逐文件独立进程） | `profiles_filter`、`r4_09`、`recheck_r3_prof_controller`、`t05_profiles_ui`、`t06a_controller`、`profiles_keyboard`、`ux_space01_entries`、`r4_22`、`fix10b_profile_order`、`context_menu_move`、`t10_groups_panel`、`ux_space01_group_flow`、`t15b_speedtest`、`recheck05_hidden_selection`、`recheck_r3_prof10`、`re_prof_06_scope` 全绿（`profiles_filter`、`t10_groups_panel` 在多文件同进程批跑时各出现 1 次未完成，单文件重跑通过，系已知 flutter_tester 进程不稳定，非本卡回归） |

### 真实查询耗时（真实 SQLite 文件，合成行，无网络；debug 测试二进制）

walk = 按 `Remarks` 稳定排序、500 行/页、经 `AsyncPageWorker`
submit/wait 逐页跟到尾（无重复/遗漏已断言）。计时只含 walk（seed 在前，
单事务批量写入，不计入；DTO/overlay 不在 walk 内）。

| 规模 | 页大小 | 页数 | 全量 worker walk 耗时 | 折合单页 |
|---|---|---|---|---|
| 10k | 500 | 20 | **670ms** | ~34ms |
| 100k | 500 | 200 | **26636ms** | ~133ms |

命令：`cargo test -p application --locked --lib sp21_real_sqlite_ -- --nocapture`
（`SP21 real-sqlite …` 行）。本次走 worker 通道且为文件库，反而快于 prep
的 `:memory:` 直调（10k 1247ms / 100k 29423ms，主因是 prep 全量 walk  harness
写法与排序/COUNT 开销不同，见下）——两组数都是 debug harness 测量，不作
生产性能宣称。

对照与说明（沿用 prep 结论）：生产价值不在全量 walk 更快，而在调用方只
等待有界单页、查询跑在后台 worker 连接（WAL 并发读，不持 writer 连接）、
取消同步返回、迟到/过期结果丢弃。100k 全量 walk 的 OFFSET 线性增长 +
逐页全表排序仍是 O(pages × n log n) harness 写法；keyset 游标与 total
轻量化仍是后续优化项（已登记，不属本卡独立范围）。

## prep 记录（保留）

## 范围

完整 UI 接线依赖 SP-16（A06 在途）。本次只做可独立的数据层：

- 真实 SQLite 异步分页查询模块（后台 worker 线程 + 有界页）。
- 稳定排序（含 ID tiebreaker）。
- 游标在数据修订变化时失效（`datasetRevision`）。
- 取消语义（协同取消 + 新 generation 迟到丢弃；取消调用同步立即返回）。

## 改动文件（写锁内）

- `crates/application/src/store_repo.rs`：新增 `AsyncPageRequest` /
  `AsyncPage` / `AsyncPageError` / `ProfilePageQuery`
  （revision+generation 编排）/ `AsyncPageWorker`（自有只读 SQLite
  连接，后台线程）/ `QueryHandle`（同步 `cancel()` + 有界 `wait()`）；
  修复 `ProfileSort::IndexId` 为 `ORDER BY "IndexId" ASC`
 （此前 `ORDER BY rowid ASC`，插入顺序而非 ID 稳定序）。
- `apps/desktop/lib/features/profiles/profiles_controller.dart`：新增
  `ProfilePageRequest` / `ProfilePageResult` / `AsyncProfilePager`
 （generation 守卫、同步取消、迟到丢弃、游标失效判定）；
  `ProfilesController.cancelProfilePageQuery()` 同步取消入口。
- `apps/desktop/test/sp21_async_page_test.dart`：Dart 合同测试（4 用例）。
- Rust 合同测试：`store_repo.rs` 内 `sp21_*` 8 用例（含真实 SQLite
  全量 walk、无重复/遗漏、有界页、ID tiebreak、修订失效、取消、
  generation 丢弃、worker 线程隔离、页大小钳制）。

## 接口缺口登记（返回给 SP-00 整合者，不私改他人文件）

需新增 FRB 查询 API（签名/错误/取消/游标合同建议）：

```text
QueryProfilesPageAsync(filter, sort, cursor, datasetRevision,
                       requestGeneration, limit)
  -> { items, nextCursor, datasetRevision, total?, error }
```

- `filter/sort/cursor/limit` 在请求时冻结；`limit` 上限 2000。
- `datasetRevision` 不匹配 → `StaleCursor{expected, actual}`，调用方从
  cursor 0 用新 revision 重查；不悄悄跳行/重复。
- 取消：`CancellationToken` 协同检查（查询前后安全点）；调用方
  `cancel()` 同步返回，迟到结果丢弃（`Cancelled` / `Superseded`）。
- 错误映射：`AsyncPageError::to_domain_error()`（`E_CANCELLED` /
  `E_CONFLICT` / `E_REVISION_STALE` / `E_TIMEOUT` / 存储原错）。
- `ProfilePageQuery::notify_mutated()` 必须在每次 profile 写提交后调用
  （engine 接线时补；当前测试内手动调用）。
- 提供方：`application::store_repo`；调用方：`bridge_api` → FRB →
  `profiles_controller.dart`；版本/持久化/生效点待整合者定。

## 定向检查

| 命令 | 结果 |
|---|---|
| `cargo fmt -p application -- --check`（本文件） | 通过（本文件 0 diff；同包他人文件 diff 与本卡无关） |
| `cargo clippy -p application --all-targets --locked -- -D warnings` | **通过 exit 0**（中途修 1 处本卡 `collapsible_if`） |
| `cargo test -p application --locked` | **通过：lib 309 passed / 0 failed**，全部集成 target 0 failed（含 8 个 `sp21_*`） |
| `dart format`（本卡两 Dart 文件） | 通过 |
| `flutter analyze` | 本卡文件 0 issue（唯一 warning 在他人 `settings_controller.dart`） |
| `flutter test test/sp21_async_page_test.dart` | **4/4 通过**；另跑 `profiles_filter_test.dart` 回归通过 |

## 真实查询耗时（真实 SQLite，合成行，无网络；debug 测试二进制）

 walk = 按 `Remarks` 稳定排序、每页附带 `COUNT(*)` total、500 行/页，
 从 cursor 0 跟到尾（无重复/遗漏已断言）。计时只含 walk（seed 在前，
 单事务批量写入，不计入）。

| 规模 | 页大小 | 页数 | 全量 walk 耗时 | 折合单页 |
|---|---|---|---|---|
| 10k | 500 | 20 | **1247ms** | ~62ms |
| 100k | 500 | 200 | **29423ms** | ~147ms |

命令：`SP21_ROWS=10000|100000 cargo test -p application --locked
sp21_async_pages_cover_full_dataset -- --nocapture`（`SP21 walk …` 行）。

对照基线（CP-15）：旧同步分页 Dart 循环 100k 约 2982ms（release DLL，
`IndexId` 排序、无逐页 COUNT 断言、另一 harness）。本卡 debug walk
更慢的主因是逐页全表 `COUNT(*)` + 全量重排 + OFFSET 线性增长
（O(pages × n log n））——这是测量 harness 的写法，不是生产目标。
生产价值不在全量 walk 更快，而在：调用方只等待有界单页、查询跑在
后台 worker 连接（WAL 并发读，不持 writer 连接）、取消同步返回、
迟到/过期结果丢弃。keyset 游标与 total 轻量化是后续优化项（已登记，
不属本卡独立范围）。

单页有界性证据：`sp21_async_page_size_is_bounded`（`u32::MAX` 请求被钳
到 2000）；worker 线程隔离证据：`worker_thread != caller` 断言通过；
取消即时性证据：`cancel()` 同步调用 `<1s` 断言 + 迟到结果丢弃通过。

## 未完成 / 不宣称

- FRB `QueryProfilesPageAsync` 真实接线（SP-00 整合者；`bridge_api`/`FRB`
  文件本卡按约束未碰，缺口登记沿用 prep 的签名/错误/取消/游标合同建议）。
- DTO/编辑器侧仍走既有同步 `queryAllProfiles` 跟随读；`reloadPaged` 完成页
  后一次取全量 DTO（窗口化 + 后台 worker 直供 DTO 需上述 FRB 缺口）。
- 完整 UI 接线（SP-16 在途）：虚拟滚动只取可视窗、overlay 增量、真实
  100k 下 UI timer/交互可用采样（待 SP-31 release GUI 实测；本次只在合成
  桥上证明页间让出 + 取消语义）。
- 本卡状态保持 identified；待阻塞解除、FRB 接线、SP-16/SP-31 实测后复评。
