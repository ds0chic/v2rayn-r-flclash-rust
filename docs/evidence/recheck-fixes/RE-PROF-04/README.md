# RE-PROF-04 证据 — Sort 读回 / 排序方向 / 写失败反馈

状态：`implemented`。日期 2026-10-04；开始 HEAD `e9b8e74`（开场工作树干净，执行期存在其它并行子代理的未提交改动）。冻结上游 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

本回合改 `apps/desktop/lib/features/profiles/{profiles_models,profiles_controller,profiles_page}.dart`、`apps/desktop/lib/bridge/bridge_port.dart`（手写 seam）、`apps/desktop/test/reprof04_sort_readback_test.dart`、`crates/application/src/speedtest.rs`、`crates/bridge_api/src/api/speedtest.rs`、本目录与任务卡、`compat/actions.yaml`（仅追加 notes）。未改生成桥接、`main_shell.dart`、`app.dart`、`engine.rs`、`updater/**`。测试只用 `SyntheticBridgePort`/内存 `ProfileExStore`，无内核、无监听端口、无系统代理/注册表/路由/TUN 改动。

## 上游对照

- `ServiceLib/ViewModels/ProfilesViewModel.cs:400-437`：`GetProfileItemsEx` 连接 `ProfileExItem` 后 `OrderBy(t => t.Sort)`；当前 `bridge_port.fetchSummaries` 曾固定走 `queryAllProfiles` 的 `ProfileSortDto.indexId`，读链缺口。
- `ProfilesViewModel.cs:635-650` + `ConfigHandler.cs:1004-1112`：`SortServer` 同列两向切换、`SortServers` 整表 `SetSort((i+1)*10)`，Delay/Speed 列 `<=0` 沉底 `maxSort`。
- `ProfilesViewModel.cs:197-199`：结果排序复用 `_dicHeaderSort` 方向。

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `dart format --output=none --set-exit-if-changed lib/features/profiles lib/bridge test/reprof04_sort_readback_test.dart` | exit 0，36 文件 0 改变（首轮 controller 被格式化一次） |
| `flutter analyze lib/features/profiles lib/bridge/bridge_port.dart test/reprof04_sort_readback_test.dart` | exit 0，`No issues found!` |
| `flutter test test/reprof04_sort_readback_test.dart` | exit 0，5/5 通过 |
| `flutter test test/fix10b_profile_order_test.dart` | exit 0，5/5 通过（写链回归） |
| `rustfmt --edition 2021 --check crates/application/src/speedtest.rs crates/bridge_api/src/api/speedtest.rs` | exit 0 |
| `cargo test -p application --lib speedtest --locked` | **未运行成功**：并行未提交的 `crates/updater/src/install.rs:191` E0308（`&String` vs `&Path`）使 `updater` 无法编译，`application` 依赖其而中止 |

未运行（任务约束）：`flutter build windows --release`、全仓 `flutter test`、全量 `cargo test --workspace`、真实 Windows 窗口。

## 改动要点

- 读回：`ProfileExStore::all()` 改为按 `(Sort, IndexId)` 排序；结果表 list 顺序即 `Sort`。`FrbBridgePort.fetchSummaries` 用 `orderByPersistedSort` 按该顺序重排 `queryAllProfiles` 的行后再叠测速 overlay，因此刷新、150ms 轮询、重开都不再回退 IndexId。`SyntheticBridgePort` 记录 `applyProfileOrder` 顺序并在 `fetchSummaries` 读回，模型化整条读链。
- 方向：`SortSpec.next` 改为两向（升↔降，不再第三击清空），与 `SortServer/_dicHeaderSort` 一致；`sortByResult` 用 `_resultSortAscending` 两向切换，失败/未知（delay<=0）在升/降两向都沉底。
- 失败反馈：`_persistOrder` 返回并检查 `SimpleResult`，失败写 `ProfilesState.orderMessage` 状态行（key `order-message`），成功清除；不再静默忽略。
- Rust：`bridge_api` 增 `results_follow_persisted_sort_order` 单测（`speedtest_results()` 按 Sort 输出、方向翻转读回）；`application::speedtest` 增 `all_reports_persisted_sort_order_and_direction` 单测。

## 边界与缺口

- 未给 `SpeedTestResultDto` 增 `sort` 字段：生成 Dart（`lib/bridge/api/**`）禁改，改 Rust 字段不重生成会造成 FFI 字段错位，故用 list 顺序承载；若需显式字段，需根代理 FRB 重新生成。
- Rust 单测因外部 `updater` 编译错误未实际运行，仅 rustfmt 解析通过。
- 未做原版实机双窗口逐事件对照；`queryAllProfiles` 的 `ProfileSortDto` 仍未含 Sort，未改动其契约。
