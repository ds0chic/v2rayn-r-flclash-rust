# FIX-10B — 节点表拖动/表头/结果排序写入原版排序字段并重开保持

状态：`implemented`（Dart UI 三种排序路径已接新桥接 `speedtestApplyProfileOrder` 写 `ProfileExItem.Sort`，Rust `ProfileExStore::apply_order` 步长 10 落库与读回有单测；真实窗口/Rust 全量并集未跑，保持 `implemented`）。

任务 ID：FIX-10B

来源：FIX-10 的 PR-15 接口缺口登记（`docs/tasks/FIX-10.md`「接口缺口（登记，PR-15/排序持久化）」；`docs/evidence/parity-review-2026-10-03/repair-queue.md` 第 35 行 FIX-10 行「拖动/表头/结果排序写原版排序字段，重开保持」）。

本次唯一用户流程：节点表拖动行重排 / 点击表头排序 / 按测试结果排序 → 三种排序结果都写入原版 `ProfileExItem.Sort`（步长 10）→ 重开后节点表顺序按 `Sort` 恢复。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `72e4c1e`（工作树含根代理已完成的 FRB 再生成，含 `speedtest_apply_profile_order`，直接调用，未改生成文件）。FIX-10 主链证据见 `docs/evidence/UX-PARITY-FIX-10/README.md`、`docs/tasks/FIX-10.md`。

对应 feature / field / action / layout ID：`F-PROFILE-008/011`（移动/排序）、`ACT-PROF-020`（按测试结果排序）、`ACT-PROF-031`（表头排序）、`ACT-PROF-033`（拖动排序）、`FLD-ENT-107`（ProfileExItem.Sort）。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的
- `ServiceLib/Handler/ConfigHandler.cs:1004`（`SortServers`：按列排序后对整表 `SetSort((i+1)*10)`；Delay/Speed 列把 `<=0` 行沉底为 `maxSort`）、`:476`（`MoveServer`：对可见列表 `SetSort((i+1)*10)` 再微调被移动行）；
- `ServiceLib/Manager/ProfileExManager.cs`（`SetSort/GetSort/GetMaxSort`）、`ServiceLib/Models/Entities/ProfileExItem.cs`（`Sort` 字段）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：当前可见行序列（当前组 + 文本过滤）。拖动＝移动 source 行到 target 行位置；表头排序＝`ProfilesController.sortBy`；结果排序＝`sortByResult`（delay 升序、失败/未知沉底，等价上游 DelayVal 列 maxSort 沉底语义）。
- 输出：`applyProfileOrder(ordered_ids)` 把 `ordered_ids[i]` 的 `ProfileExItem.Sort` 写为 `(i+1)*10` 并 flush 到 SQLite；Dart 侧只发有序 id 列表。
- 错误：桥接返回 `SimpleResult`；本卡空列表/单行不发请求（无排序信息、不报错、不伪造持久化）。
- 取消：无长任务；排序为同步一次写入。
- 权限：仅本机 UI + FRB/Rust/SQLite；不启动内核、不写系统代理/TUN、不监听端口、不碰 10808。
- 持久化：`ProfileExItem.Sort` 写 SQLite；重开后 `profile_ex_all` 读回同值（Rust 单测 `apply_order_writes_step_ten_and_reads_back_stably` 证明步长与读回序）。
- 生效：重开进程懒加载 `ProfileExItem`；Rust 侧读回排序字段恢复顺序。Dart 侧「按 Sort 重排节点表」受 DTO 缺口限制（见下）。

允许修改的模块：`apps/desktop/lib/features/profiles/**`（改 `profiles_controller.dart`）、`apps/desktop/lib/bridge/bridge_port.dart`（**经用户确认**：手写 seam，非生成文件；新增 `applyProfileOrder`，`FrbBridgePort` 转发到已生成的 `speedtest.speedtestApplyProfileOrder`，`SyntheticBridgePort` 记录调用）、`apps/desktop/test/**`（新增 `fix10b_profile_order_test.dart`）、`crates/application/src/speedtest.rs`（仅加读回单测）、`docs/evidence/UX-PARITY-FIX-10B/**`、本卡、`compat/features.yaml`（仅追加 evidence）。未改 `main_shell.dart`、`app.dart`、`frb_generated.*`、`lib/bridge/api/**`、`features/{settings,subs,runtime,monitor,update,backup,routing}/**`、`crates/application/src/engine.rs`、`crates/bridge_api/src/api/**`。

禁止改变的已有行为：FIX-10 已提交的选中集合语义/移除无效/去重语义、菜单结构、拖动反馈 UI、列排序稳定锚点；不删入口或降分母；不改生成文件；不伪造排序持久化。

测试夹具和原版预期：`SyntheticBridgePort` 合成节点（无原生库）驱动控制器；记录 `appliedProfileOrders`。原版预期：三种排序都对整表写 `(i+1)*10`；空/单行无动作。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed lib test`
- `flutter analyze`（本卡文件：`No issues found`）
- `flutter test test/fix10b_profile_order_test.dart`
- Rust：`cargo fmt -p application -- --check`、`cargo test -p application --lib speedtest --locked`、`cargo test -p application --test fix10_speedtest_result --locked`

证据文件位置：`docs/evidence/UX-PARITY-FIX-10B/`（`observations.json`、`README.md`）。

完成条件：拖动/表头/结果排序三种路径都经 `speedtestApplyProfileOrder` 写 `ProfileExItem.Sort`；Rust 单测证明步长 10 与重开读回；空/单行不写不报错；FIX-10 既有语义不变；门禁（本卡文件）通过；全 workspace 测试与真实窗口并集本轮未跑，故保持 `implemented`。

- 接口缺口（登记，Dart 侧按 Sort 重排节点表）：生成的 `SpeedTestResultDto` 无 `sort` 字段（`apps/desktop/lib/bridge/api/speedtest.dart:123`），`ProfileDto` 亦无 `Sort`（对比 `FLD-ENT-104..109`），Dart 无法读回 `Sort` 直接给节点表排序；`queryAllProfiles` 走 `ProfileSortDto.indexId`。本卡按任务「补 Rust 或 Dart 层测试证明读回」以 Rust 层证明持久化与读回；若后续要在 Dart 层按 `Sort` 重排节点表，需在共享桥接给结果 DTO 增 `sort` 字段（改生成文件，本卡禁改）。
- 未完成/范围：未跑全 workspace 测试、`flutter build windows --release` 与真实窗口 reopen 并集；`features/monitor/**` 当前被另一子代理并行改动，`flutter analyze` 全量报 `ClashModeDto` 等错误与本卡无关（本卡文件单独 analyze 干净）。

本轮实际结果：见 `docs/evidence/UX-PARITY-FIX-10B/README.md`。
