# R3-PROF-04 — 去重设置、活动回退与失败提示

状态：`implemented`（合成 controller/widget 回归本轮已跑并绿；未做真实 Windows 窗口 + FRB 端到端，故不写 `verified`）。

任务 ID：R3-PROF-04

本次唯一用户流程：「移除重复」读取 `GuiItem.KeepOlderDedupl`（false 时反转，保留较新项）；若删除包含活动重复节点，删除后 Reload 并回退到可用的活动节点；删除失败显示真实错误，而不是「没有重复节点」。

前置任务及已验证证据：第三轮复核 `docs/evidence/parity-recheck-2026-10-04/round3-profiles.md` 的 R3-PROF-04 行与 RE-PROF-06；冻结 `UP/ServiceLib/Handler/ConfigHandler.cs:1156-1180`（`DedupServerList`，`:1166-1168` 在 `!KeepOlderDedupl` 时 reverse）、`UP/ServiceLib/ViewModels/ProfilesViewModel.cs:537-541`（删除后 Refresh + Reload）。既有 `re_prof_06_scope_test.dart`、`fix10_dedup_test.dart` 覆盖范围与纯函数（不回归）。

对应 ID：`ACT-PROF-003`、`FLD-CFG-*`（`KeepOlderDedupl`）、`F-PROFILE-*`。

必读上游文件、符号和固定 commit：UP `7d6a967`；`UP/ServiceLib/Handler/ConfigHandler.cs:1004-1111`（Sort，仅作旁证）、`:1156-1180`；`UP/ServiceLib/Models/Configs/ConfigItems.cs:72`（`KeepOlderDedupl` 默认 false）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`GuiItem.KeepOlderDedupl`（缺失/读取失败时保守回退 true）。
- 输出：`DedupOutcome{ok, removed, hadDuplicates, activeRemoved, errorCode/messageKey}`。
- 错误：删除失败返回结构化错误码并在状态栏显示「移除重复失败：<code>」；不再冒充「没有重复节点」。
- 取消：确认框取消不删除。
- 权限：仅本机 UI + FRB/SQLite；无内核/系统代理。
- 持久化：真实 `deleteProfiles`；删除含活动对象后回退。
- 生效：活动重复被删后复用 RE-PROF-02 回退（`reconcileActiveAfterRemoval`）。

允许修改的模块：`apps/desktop/lib/features/profiles/{profiles_controller.dart,profiles_table.dart,profile_actions.dart,profile_dedup.dart(只读语义不变)}`、`apps/desktop/test/**`、本卡、`docs/evidence/recheck-fixes/R3-PROF-01-04/**`、`compat/actions.yaml`（仅追加）。
禁止改变：不改 `crates/**`（`engine.delete_profiles` 不处理 active 的现状不在本卡改）；不改 `features/**`（除 profiles 目录）；不回归 RE-PROF-06 范围规则。

测试夹具和原版预期：合成两条相同传输身份节点（RFC 5737 地址）；设置 true/false 分别保留旧/新；活动节点为被删对象时断言回退；`failDeleteProfiles` 注入断言真实失败信息。原版预期：false→reverse；删除即 Refresh+Reload；失败不报“无重复”。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed lib test`
- `flutter analyze`
- `flutter test test/recheck_r3_prof_controller_test.dart test/recheck_r3_prof04_dedup_widget_test.dart test/re_prof_06_scope_test.dart test/fix10_dedup_test.dart`
- 真窗 + 真 SQLite 重开对照（本轮未做，登记）。

证据文件位置：`docs/evidence/recheck-fixes/R3-PROF-01-04/`。

完成条件：设置读取生效；活动回退；失败提示真实；既有 `re_prof_06_scope_test`/`fix10_dedup_test` 绿。未真窗对照，保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：`engine.delete_profiles` 不清理 active，删除含活动对象后的 DB 级一致性依赖 UI 回退；如要后端原子清理，需要新用例契约，本卡不造新 IPC。
- 接口缺口（登记）：`removeDuplicateProfiles` int 包装保留给直接调用方；真实菜单走 `removeDuplicateProfilesDetailed`，二者语义需在后续统一时收口。

本轮实际结果：`profiles_controller.dart` 新增 `readKeepOlderDedupl`、`DedupOutcome`、`removeDuplicateProfilesDetailed`（读取设置、记录 `activeRemoved`、区分失败与无重复），保留 int 包装；`profiles_table.dart` 的 `_removeDuplicate` 使用 detailed，失败显示真实错误码，`activeRemoved` 时调用 `reconcileActiveAfterRemoval`。命令：`flutter analyze` → No issues；`flutter test test/recheck_r3_prof_controller_test.dart`（3 项去重）→ 8/8；`flutter test test/recheck_r3_prof04_dedup_widget_test.dart` → 1/1；`flutter test test/re_prof_06_scope_test.dart` → 3/3；`flutter test test/fix10_dedup_test.dart` → 4/4。
