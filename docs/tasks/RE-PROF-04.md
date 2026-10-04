# RE-PROF-04 — 节点表按持久化 Sort 读回、排序方向两向切换、写失败可见

状态：`implemented`（Dart 读回/方向/失败反馈有合成 bridge 测试；Rust Sort 输出/读回改动已就位且 rustfmt 通过，但 `cargo test` 被并行未提交的 `crates/updater/src/install.rs` 编译错误阻塞，未运行，故不写 `verified`）。

任务 ID：RE-PROF-04

来源：`docs/evidence/parity-recheck-2026-10-04/profiles.md` 的 RE-PROF-04 全节（P1），承接 FIX-10B 登记的 DTO 读回缺口。

本次唯一用户流程：节点表拖动行重排 / 点击表头排序 / 「按测试结果排序」→ 顺序写入 `ProfileExItem.Sort` → 刷新、测速轮询或关闭重开后节点表仍按该顺序显示；同一列再次点击在升/降序间切换；排序写库失败时用户可见提示。

上游对照：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
- `ServiceLib/ViewModels/ProfilesViewModel.cs:400-437`：`GetProfileItemsEx` 连接 `ProfileExItem` 后 `OrderBy(t => t.Sort)`，节点表顺序以持久化 Sort 为准。
- `ProfilesViewModel.cs:635-650` `SortServer` + `_dicHeaderSort`：同列第二次点击翻转 asc/desc（两向，无第三态）；`SortServers` 把列序整表 `SetSort((i+1)*10)` 落库（`ConfigHandler.cs:1004-1112`，Delay/Speed 列把 `<=0` 沉底为 `maxSort`）。
- `ProfilesViewModel.cs:197-199` `SortServerResultCmd → SortServer("DelayVal")`：结果排序复用同一方向状态。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：当前可见行序列（当前组 + 文本过滤）；拖动/表头/结果三条排序路径。
- 输出：`applyProfileOrder(ordered_ids)` 写 `ProfileExItem.Sort=(i+1)*10`（FIX-10B 既有，未重复造）；读端 `ProfileExStore::all()` 改为按 `(Sort, IndexId)` 排序，结果表 list 顺序即 Sort，`fetchSummaries` 依据该顺序重排节点行。
- 错误：`_persistOrder` 桥接返回 `SimpleResult`；失败时状态字段 `orderMessage` 显示「排序保存失败：<code>」，下一次成功写入清除；空/单行不写不报错。
- 取消：无长任务；写为同步一次。
- 权限：仅本机 UI + FRB/Rust/SQLite；不启动内核、不改系统代理/TUN、不监听端口、不碰 10808。
- 持久化：`ProfileExItem.Sort` 写 SQLite；`profile_ex_all` 重开读回。
- 生效：重开/刷新/150ms 测速轮询后 `fetchSummaries` 用读回顺序，不再被 `queryAllProfiles` 的 `ProfileSortDto.indexId` 覆盖。

允许修改的模块：`apps/desktop/lib/bridge/bridge_port.dart`（手写 seam）、`apps/desktop/lib/features/profiles/**`、`apps/desktop/test/**`、`crates/application/src/speedtest.rs`、`crates/bridge_api/src/api/speedtest.rs`、本卡、`docs/evidence/recheck-fixes/RE-PROF-04/**`、`compat/*.yaml`（仅追加 notes）。未改 `main_shell.dart`、`app.dart`、生成桥接（`frb_generated.*`、`lib/bridge/api/**`）、`features/{subs,runtime,settings,monitor,update,backup,routing}/**`、`crates/application/src/engine.rs`、`crates/updater/**`、`crates/subscriptions/**`。

禁止改变的已有行为：FIX-10/10B 已提交的选中集合、移除无效、去重语义与 `applyProfileOrder` 写链；不删入口或降分母；不伪造持久化成功。

测试夹具和原版预期：`SyntheticBridgePort` 合成节点（无原生库）+ `ProfileExItem.Sort` 顺序记录；原版预期：三条排序路径都写 Sort，刷新/重开按 Sort 恢复，同列两向切换，失败可见。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed lib test`
- `flutter analyze lib/features/profiles lib/bridge/bridge_port.dart test/reprof04_sort_readback_test.dart`
- `flutter test test/reprof04_sort_readback_test.dart`
- `flutter test test/fix10b_profile_order_test.dart`（回归写链）
- Rust：`rustfmt --edition 2021 --check crates/application/src/speedtest.rs crates/bridge_api/src/api/speedtest.rs`；`cargo test -p application --lib speedtest --locked`、`cargo test -p bridge_api --lib -- speedtest::tests`（本轮被并行 `updater` 编译错误阻塞）。

证据文件位置：`docs/evidence/recheck-fixes/RE-PROF-04/`（`observations.json`、`README.md`）。

完成条件：Dart 读回/方向/失败反馈测试通过；Rust Sort 输出与 `all()` 读回单测写好；FIX-10/10B 语义不变；Rust 全量测试因外部并行编译错误未运行，保持 `implemented`。

- 接口缺口（登记，非阻塞）：未给 `SpeedTestResultDto` 增 `sort` 字段（`apps/desktop/lib/bridge/api/speedtest.dart:123` 无该字段）。因为生成 Dart 侧禁改，增 Rust 字段而不重生成会造成 FFI 解码错位，故改用「结果表 list 顺序即 Sort」传递，零 ABI 变更。若后续要显式字段，需根代理 FRB 重新生成后再在 Dart 消费。
- 阻塞（登记）：`crates/updater/src/install.rs:191` 已存在的类型错误（`normalize_lexically(&self.keep_name)` 期望 `&Path` 实参 `&String`，并行子代理未提交改动）导致 `application`/`bridge_api` 无法编译，Rust 测试本轮未运行。
