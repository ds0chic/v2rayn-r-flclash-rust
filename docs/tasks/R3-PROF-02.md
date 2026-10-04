# R3-PROF-02 — 命令目标不漂移与隐藏对象合同

状态：`implemented`（合成 controller/widget 回归本轮已跑并绿；未做真实 Windows 窗口 + FRB 端到端，故不写 `verified`）。

任务 ID：R3-PROF-02

本次唯一用户流程：菜单在 A 打开后，即使刷新/选择漂移、或同组文字过滤把 A 隐藏，菜单命令仍只作用于打开时捕获的对象；隐藏对象一律拒绝并提示，绝不改作用 live 选择；完整配置导出也读取捕获目标而非 live 选择。

前置任务及已验证证据：第三轮复核 `docs/evidence/parity-recheck-2026-10-04/round3-profiles.md` 的 R3-PROF-02 行；FIX-01 已引入不可变 `CommandContext`（对象=打开时目标）。当前缺口为 `profiles_table.dart` 完整导出走 live 选择、`restoreContextTargets` 只查 `state.all`（隐藏行仍可恢复）。

对应 ID：`ACT-PROF-001/005/006/009-013/022/023`、`F-PROFILE-*`、`LAY-PROFILES-004`。

必读上游文件、符号和固定 commit：UP 冻结 `7d6a967`；`UP/v2rayN/Views/ProfilesView.xaml.cs` 菜单命令取 `SelectedProfile`/`SelectedProfiles`；`ProfilesViewModel` 各命令在命令时机读取对象。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：菜单打开时捕获的不可变 `CommandContext{targetIds, primaryId, groupSubId, viewContext}`。
- 输出：批量命令作用于捕获的 `targetIds`；单对象命令作用于捕获 `primaryId`；不与 live 选择混用。
- 错误：目标已被过滤/删除/换组 → 关闭菜单并提示「操作目标已失效，请重新选择节点」，不执行。
- 取消：菜单外点/Esc 关闭只清 UI 会话，不销毁已捕获命令。
- 权限：仅本机 UI + FRB/SQLite；无内核/系统代理/TUN。
- 持久化/生效：沿用既有命令路径；不新增 IPC。

允许修改的模块：`apps/desktop/lib/features/profiles/{profiles_controller.dart,profiles_table.dart,profile_actions.dart,command_context.dart,table_actions.dart,profiles_models.dart}`、`apps/desktop/test/**`、本卡、`docs/evidence/recheck-fixes/R3-PROF-01-04/**`、`compat/actions.yaml`（仅追加）。
禁止改变：同 R3-PROF-01 的模块禁令；不回归 RE-PROF-13/14、不改 FIX-10/10B 排序写链。

测试夹具和原版预期：合成 `SyntheticBridgePort`；在菜单开/执行之间程序化改变选择或加文字过滤，记录 bridge 请求 id。原版预期：命令只作用于打开时对象；不可见对象不执行。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed lib test`
- `flutter analyze`
- `flutter test test/recheck_r3_prof_controller_test.dart test/recheck_r3_prof_menu_target_test.dart`
- 去重确认期间换组/过滤的真窗对照（本轮未做，登记）。

证据文件位置：`docs/evidence/recheck-fixes/R3-PROF-01-04/`。

完成条件：controller 断言隐藏目标 `restoreContextTargets` 返回 false；widget 断言完整配置导出使用捕获目标、隐藏目标拒绝；analyze/format/测试通过。未真窗对照，保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：去重确认对话框在弹出后到确认之间可能发生换组/过滤，当前 `_removeDuplicate` 确认后仍按 `removeDuplicateProfilesDetailed` 读当前组；如需“确认期间快照组”，需要确认框携带捕获上下文的接口，本卡仅保证菜单命令上下文不漂移。
- 接口缺口（登记）：同组过滤隐藏判定的权威是 `ProfilesState.visible`；跨窗口（托盘等）没有共享可见集通道。

本轮实际结果：`restoreContextTargets` 改为要求目标仍在 `state.visible`（不可见即拒绝）；`ProfilesState` 主行随 `_recompute` 隐藏时清理；`_onContextAction` 对消费节点主对象的命令（编辑/分享/设活动/完整配置导出）增加可见性校验，`_usesPrimaryTarget` 精确限定范围以免误伤整组生成等命令。命令：`flutter analyze` → No issues；`flutter test test/recheck_r3_prof_controller_test.dart`（含隐藏拒绝）→ 8/8；`flutter test test/recheck_r3_prof_menu_target_test.dart` → 1/1。回归：`flutter test test/ux_parity_fix01_gen_group_guard_test.dart` → 1/1（确认未误伤一键生成策略组的组上下文）。
