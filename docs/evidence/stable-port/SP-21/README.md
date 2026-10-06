# SP-21 准备与数据层证据（ identifies / 部分 implemented，未完成 ）

卡状态：**identified**（准备与数据层部分 implemented；完整卡未完成，不伪造完成）。
基线：`92d46dd`。本次不 commit。

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

- FRB `QueryProfilesPageAsync` 真实接线（SP-00 整合者）。
- 完整 UI 接线（SP-16 在途：A06）：虚拟滚动只取可视页、overlay 增量、
  跨页全选、迟到丢弃的界面证据。
- 本卡状态保持 identified；待阻塞解除、10k/100k 实测、SP-16 接线后复评。
