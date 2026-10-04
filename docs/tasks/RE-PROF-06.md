# RE-PROF-06 — 去重/移除无效范围按整组，结果清理按组且不误清失败证据

状态：`implemented`（Dart 合成桥 + Rust 单 crate 覆盖已通过；未做原版实机双窗口逐事件对照，未跑真实 Windows 窗口链路，故不写 `verified`）。

任务 ID：RE-PROF-06

本次唯一用户流程：节点表顶部选中有节点的订阅分组 A 后，用文本搜索把某些节点隐藏；执行右键「移除重复」或「按测试结果移除无效」时，对象仍是 A 的**整组**节点（被搜索隐藏的同组重复/失败节点也要处理），B 组的失败节点与其失败记录必须保留；删除失败或部分失败时，同组失败记录不得被清掉。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；复核结论见 `docs/evidence/parity-recheck-2026-10-04/profiles.md` 的 RE-PROF-06；上游定位 `ConfigHandler.DedupServerList:1156-1193`（取 `ProfileItems(subId)`，不带文本过滤）与 `ConfigHandler.RemoveInvalidServerResult:1595-1611`（取 `ProfileModels(subid, "")`，按 `Delay == -1` 删同组非复杂节点）。开始 HEAD `472ca54`，工作树干净；本卡只读 `main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**` 及禁止清单内文件。

对应 feature / field / action / layout ID：`F-PROFILE-005`(去重)、`F-PROFILE-010`(移除无效)、`ACT-PROF-003`(移除重复)、`ACT-PROF-021`(按测试结果移除无效)、`F-PROFILE-009`(命令上下文/当前组)、`FLG-ENT-005`(Subid)。

必读上游文件、符号和固定 commit：
- `ServiceLib/Handler/ConfigHandler.cs:1156-1193`（`DedupServerList`：`ProfileItems(subId)` → `CompareProfileItem` → `RemoveServers`）；
- `ServiceLib/Handler/ConfigHandler.cs:1595-1611`（`RemoveInvalidServerResult`：`ProfileModels(subid, "")`，剔复杂节点，join `ProfileExs` 取 `Delay == -1`，`RemoveServers`）；
- `ServiceLib/ViewModels/ProfilesViewModel.cs`（`RemoveDuplicateServerCmd` / `RemoveInvalidServerResultCmd` 使用当前 `SubIndexId`）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：去重/移除无效的对象=当前组 `state.groupSubId` 的整组 `ProfileDto`（`subid` 过滤），**不叠加** `state.filter` 文本筛选；`groupSubId == null` 时按全部已存节点。
- 输出：`removeDuplicateProfiles` 返回真实删除数；`removeInvalidResults` 返回真实删除的 `ProfileItem` 数。
- 错误：删除失败（`DeleteProfilesResult.ok == false`）时返回 0/保留节点，且**不清理结果行**；部分删除（`removed != targets.length`）同样不清理，保证同组失败证据不丢。
- 取消：无（同步动作）。
- 权限：仅本机 UI + FRB/Rust/SQLite；不启动内核、不监听端口、不写系统代理/TUN、不碰 10808。
- 持久化：真实删除经既有 `deleteProfiles` 落 SQLite；结果行经 `speedtest_remove_invalid` 持久化清理。
- 生效：`reload()` 后节点表刷新；重开数据一致。

允许修改的模块：`apps/desktop/lib/features/profiles/{profiles_controller.dart,profile_dedup.dart}`、`apps/desktop/lib/bridge/bridge_port.dart`（手写 seam）、`crates/application/src/speedtest.rs`、`crates/bridge_api/src/api/speedtest.rs`（可加函数，未改 `frb_generated`）、`apps/desktop/test/**`、本卡、`docs/evidence/recheck-fixes/RE-PROF-06/**`、`compat/` 台账（仅追加）。未改 `main_shell.dart`、`app.dart`、`frb_generated.*`、`lib/bridge/api/**`、`crates/application/src/engine.rs`/`backup_service.rs` 及其它禁止清单。

禁止改变的已有行为：FIX-10/10B 已提交的选中集合/排序语义、菜单结构/条目/顺序、`_recompute`/`reload` 的可见集合投影、去重比较函数 `compareProfileForDedup`、`speedtestApplyProfileOrder`。

接口缺口（登记，FRB 重生成）：按字面合同希望 `speedtest_remove_invalid` 带 `subid` 由 Rust 一次完成“按组移除无效”。`AppEngine::remove_invalid_profiles(subid)` 已存在且单测通过（`crates/application`），但它是 application 层 `pub fn`，无对应 FRB 生成函数；改桥接签名/新增 FRB 函数必须重跑 `flutter_rust_bridge_codegen`，本卡禁改 `frb_generated.*` 与 `lib/bridge/api/**`。因此本轮：
  - 新增 `crates/bridge_api/src/api/speedtest.rs::speedtest_remove_invalid_group(subid) -> i32`（`#[frb(sync)]`，内部复用 `AppEngine::remove_invalid_profiles`），作为重生成后的按组一次完成入口；
  - 现网 Dart 路径仍调用既有 `speedtest_remove_invalid()`，但其语义已改为**孤儿清理**（只删 `delay == -1 且 profile 已不存在` 的结果行），等价于“按组清理 + 保留他组/失败删除证据”，无签名变化、无重生成即可生效。
  - **根代理前置**：如需 Dart 直接调用 `speedtest_remove_invalid_group`，须先运行 FRB 重生成，再把 `FrbBridgePort.removeInvalidResults` 接到该函数；在此之前 Dart/`bridge_port` 不引用任何未生成符号。

测试夹具和原版预期：合成节点（RFC 5737 地址，`192.0.2.x:443`，不下载、不连接），合成组 `SUB-R06-A`/`SUB-R06-B`，合成 `ProfileExItem` `Delay == -1`。原版预期：去重按 `ProfileItems(subid)` 整组比较；移除无效按同组 `Delay == -1` 非复杂节点删除；他组失败节点/记录不受影响。

本次必须通过的命令/真实场景（实际运行）：
- `cargo fmt -p application -p bridge_api -- --check`：通过（无输出）。
- `cargo test -p application --lib re_prof_06_tests --locked`：2 passed。
- `cargo test -p application --test fix10_speedtest_result --locked`：4 passed（回归）。
- `cargo test -p bridge_api speedtest --locked`：5 passed（含 `result_overlay_and_invalid_cleanup` 孤儿语义回归）。
- `cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings`：通过。
- `dart format`（三文件）：通过。
- `flutter analyze`：No issues found。
- `flutter test test/re_prof_06_scope_test.dart test/t15b_speedtest_test.dart`：10 passed。
- `flutter test test/fix10_dedup_test.dart`：4 passed（回归）。

证据文件位置：`docs/evidence/recheck-fixes/RE-PROF-06/`（`observations.json`、`README.md`、`runs.txt`）。

完成条件：Dart 合成覆盖“filter 隐藏的同组重复/失败仍被处理、他组不受影响、删除失败保留”；Rust 覆盖“按组移除无效、跨组保留、删除失败不清结果”；回归 FIX-10 通过；门禁通过。未跑真实 Windows 窗口，保持 `implemented`。

本轮实际结果：见 `docs/evidence/recheck-fixes/RE-PROF-06/README.md`。
