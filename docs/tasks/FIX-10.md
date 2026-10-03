# FIX-10 — 选中测速→结果→按结果处理→重开

状态：`implemented`（主链“选中测速→结果→移除无效→重开”已在本机真实 Windows 窗口 + 真实 FRB/Rust/SQLite 验证；排序/拖动/表头持久化与 UDP 因桥接函数缺口未接入，按 §19 登记接口缺口，保持未伪造。未做原版实机双窗口逐事件对照，故不写 `verified`）。

任务 ID：FIX-10

本次唯一用户流程：节点表顶部分组选中一个有效订阅分组 → 选中若干节点 → 运行测速（Tcping 主链，其余类型后续补测）→ 节点表显示每节点延迟结果 → 右键「按测试结果移除无效」真实删除当前组内失败 `ProfileItem` → 关闭重开进程，删除结果保持、有效节点保留。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `c1c9c77`（本卡只读，未回退前序修复）。审查结论见 `docs/evidence/parity-review-2026-10-03/README.md`、`repair-queue.md` FIX-10 行、`profiles-report.md` PR-11/12/15/16/17/28 与 PR-12、`all-items.json` 的 `F-PROFILE-005/008/010/011/015`、`F-TEST-001..007`、`ACT-PROF-003/009/010/011/012/014..021/031/033`、`LAY-PROFILES-002/004`、`ENUM-006/012`。

对应 feature / field / action / layout ID：`F-PROFILE-005`(去重)、`F-PROFILE-008/011`(移动/排序)、`F-PROFILE-010`(移除无效)、`F-PROFILE-015`/`F-TEST-001..007`(测速与结果)、`FLD-ENT-104..109`(ProfileExItem 字段)、`ACT-PROF-003`(移除重复)、`ACT-PROF-014..021`(6 类测速/排序/移除无效)、`ACT-PROF-031/033`(表头/拖动排序)、`LAY-PROFILES-002`、`ENUM-006`(EMove)、`ENUM-012`(ESpeedActionType)。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的
- `ServiceLib/ViewModels/ProfilesViewModel.cs:125/530/635/652/676/692/705/715`（`RemoveDuplicateServer`/`SortServer`/`RemoveInvalidServerResult`/`MoveServer`/`MoveServerTo`/`ServerSpeedtest`）；
- `ServiceLib/Handler/ConfigHandler.cs:476/1004/1156/1258/1595`（`MoveServer`/`SortServers`/`DedupServerList`/`CompareProfileItem`/`RemoveInvalidServerResult`）；
- `ServiceLib/Services/SpeedtestService.cs:88/705`（`RunAsync`/`ServerSpeedtest` 的 Mixed/Fast 取 `ProfileItems.OrderBy(Sort)`，其它取 `SelectedProfiles`）；
- `ServiceLib/Manager/ProfileExManager.cs`、`ServiceLib/Models/Entities/ProfileExItem.cs`（`IndexId/Delay/Speed/Sort/Message/IpInfo`，SQLite 读写在 `InitData`/`SaveQueueIndexIds`）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：测速对象=当前可见/当前组与选中集合。Mixed/Fast 取当前组 + 文本过滤后的可见列表（`ProfileItems` 语义），其它动作取 `SelectedProfiles` 选中集合；空集合=无可测节点，绝不回退全库。
- 输出：节点表延迟/速度/IP 列叠加真实 `ProfileExItem`；`removeInvalidResults` 返回真实删除的 `ProfileItem` 数。
- 错误：启动失败弹结构化 code；空选中提示「没有可测试节点」；移除无效无失败节点时删除 0，不假成功。
- 取消：Esc 停止运行中的测速作业（`SpeedtestingStop`）。
- 权限：仅本机 UI + FRB/Rust/SQLite；Tcping 为直连 TCP（≥11808 本地监听），不启动内核、不写系统代理/TUN、不碰 10808。
- 持久化：`ProfileItem`（删除）与 `ProfileExItem`（delay/speed/sort/message/ip）写 SQLite；重开一致。
- 生效：测速完成后 150 ms 轮询结果叠加；移除无效后 reload，重开数据仍在。

允许修改的模块：`apps/desktop/lib/features/profiles/{profiles_controller.dart,profiles_table.dart,context_menu.dart,table_actions.dart,profile_dedup.dart}`、`apps/desktop/test/**`、`apps/desktop/integration_test/**`、`crates/application/src/{speedtest.rs,engine.rs}`、`crates/bridge_api/src/api/speedtest.rs`、`crates/application/tests/fix10_speedtest_result.rs`、`docs/evidence/UX-PARITY-FIX-10/**`、本卡、`compat/features.yaml`/`actions.yaml`（仅 evidence/notes 追加）。未改 `main_shell.dart`、`frb_generated.*`、`features/subs/**`、`features/runtime/**`、`crates/application/src/subs.rs`。

禁止改变的已有行为：菜单结构/条目/根顺序/分隔/32px 高度、编辑器、列头与列宽、`main_shell`、成长条与关闭语义；不删入口或降分母；不伪造测速/删除结果；不改生成文件。

测试夹具和原版预期：合成订阅（URL 为 loopback，从未请求）+ 合成 `vless://` 节点指向本地 loopback 监听端口（≥11808，仅 accept）与一个已释放的死端口。原版预期：Tcping 直连测延迟，失败节点 `Delay==-1`；移除无效按当前组删除 `Delay==-1` 的非复杂节点；去重按 `CompareProfileItem` 保留较旧节点。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed lib test integration_test`
- `flutter analyze`
- `flutter test test/t15b_speedtest_test.dart test/fix10_dedup_test.dart`
- 真实窗口：`flutter test integration_test/ux_parity_fix10_speedtest_result_test.dart -d windows`，`V2RAYN_R_FIX10_MODE=run` 后同 data dir 再跑 `reopen`。
- Rust：`cargo fmt -p application -p bridge_api -- --check`、`cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings`、`cargo test -p application -p bridge_api --locked`。

证据文件位置：`docs/evidence/UX-PARITY-FIX-10/`（`observations.json`、`reopen-observations.json`、`01-results.png`、`02-after-remove-invalid.png`、`reopen-window.png`、`README.md`）。

完成条件：主链“选中测速→结果→移除无效→重开”真实窗口转绿并落库；空选中/取消/错误集合准确；去重入口恢复且真实去重（Dart 忠实移植 compare，Rust 同源单测）；ProfileEx 结果写 SQLite 并重开一致；6 类测速中本卡至少完成 Tcping 主链，其余按序补测并记录；门禁通过。未做原版实机双窗口逐事件对照、排序/UDP 持久化受桥接缺口限制，故保持 `implemented`。

- 接口缺口（登记，PR-15/排序持久化）：节点拖动、表头排序、按结果排序都需写上游 `ProfileExItem.Sort`（`ConfigHandler.SetSort`/`SortServers`/`MoveServer`）并重开保持。`crates/application` 已有 `AppEngine::set_profile_sort`/`ProfileExManager` 语义与 `ProfileExItem.Sort` 落库单测，但 `ProfileDto` 无 `Sort` 字段且桥接没有 `speedtest_sort`/`move_profiles` 类 FRB 函数；新增函数须改 `frb_generated.*`（本卡禁改）。建议后续在共享桥接新增一个携带 `subid + ordered_ids + 可选列名/方向` 的排序用例，再接 UI。排序重开持久化本轮为未完成。
- 接口缺口（登记，PR-11 的 Rust 端子集）：`AppEngine::remove_invalid_profiles(subid)` 已实现并单测（按组内 `Delay==-1` 删除非复杂节点），但桥接 `speedtest_remove_invalid()` 无 `subid` 参数保持结果行清理语义；真实 `ProfileItem` 删除本轮由 UI 经既有 `deleteProfiles(ids)` 完成。若后续要在 Rust 一次完成“按组移除无效”，需新增带 `subid` 的 FRB 函数。
- 未完成/平台差异（PR-28，UDP）：`SpeedTestSupport.udp=false` 保持诚实禁用；上游 `DoUdpTest` 需要测试内核承载 UDP 路由，受限测试会话尚未提供该路径。未恢复适用实现，如实标注平台/范围差异，不伪造 UDP 结果。

本轮实际结果：见 `docs/evidence/UX-PARITY-FIX-10/README.md`。
