# R3-PROF-01 — 多选下的独立“主行”命令目标

状态：`implemented`（合成 controller/widget 回归本轮已跑并绿；未做真实 Windows 窗口 + FRB 端到端，故不写 `verified`）。

任务 ID：R3-PROF-01

本次唯一用户流程：Ctrl/Shift 多选 A+B 后，右键其中一行打开菜单，「编辑 / 分享 / 设为活动 / 导出所选完整配置」作用于指针所在的主行，而不是要求 `selected.length == 1`；批量选择集合保持不变。

前置任务及已验证证据：第三轮复核 `docs/evidence/parity-recheck-2026-10-04/round3-profiles.md` 的 R3-PROF-01 行与优先表；冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`（下称 UP）。`profiles_table.dart:807-816` 已在菜单打开时捕获 `CommandContext.primaryId` 却未被消费；`profile_actions.dart` 的编辑/分享/设活动/导出均要求单选。

对应 feature / field / action / layout ID：`ACT-PROF-001`（编辑）、`ACT-PROF-005`（设为活动）、`ACT-PROF-006`（分享）、`ACT-PROF-022/023`（导出完整配置）、`F-PROFILE-*`、`LAY-PROFILES-004`。上游对象为 `ProfilesViewModel.SelectedProfile` / DataGrid 当前行。

必读上游文件、符号和固定 commit：`UP/v2rayN/Views/ProfilesView.xaml.cs`（分享/编辑入口）、`UP/ServiceLib/ViewModels/ProfilesViewModel.cs:494-500`（编辑活动节点后 Reload）、`UP/ServiceLib/ViewModels/ProfilesViewModel.cs:593-607`（`ShareServerAsync` 取 `SelectedProfile`）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：控制器维护的独立 `ProfilesState.primaryId`（当前行/anchor，独立于批量 `selected`）；菜单打开时捕获的不可变 `CommandContext.primaryId`。
- 输出：单对象命令以解析出的目标 id 执行（显式捕获目标 > `primaryId` > 单选）。
- 错误：无任何可用目标时按原有文案提示；不伪造成功。
- 取消：编辑器/QR 窗口取消不落库。
- 权限：仅本机 UI + FRB/SQLite；不启动内核、不写系统代理/TUN、不监听端口。
- 持久化：沿用既有 `saveProfile`/`setActiveProfile` 路径。
- 生效：编辑活动节点后沿用 `applyAfterEditIfActive`；设为活动沿用 `activateProfileDetailed`。

允许修改的模块：`apps/desktop/lib/features/profiles/{profiles_controller.dart,profiles_table.dart,table_actions.dart,profile_actions.dart,profiles_models.dart,command_context.dart}`、`apps/desktop/test/**`、本卡、`docs/evidence/recheck-fixes/R3-PROF-01-04/**`、`compat/actions.yaml`（仅追加）。
禁止改变：`main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`features/subs/**`、`features/runtime/**`、`features/settings/**`、`features/monitor/**`、`features/update/**`、`features/backup/**`、`features/routing/**`、`crates/**`；不改 FIX-10/10B 已提交的排序写链与选中集合语义，不回归 RE-PROF-13/14。

测试夹具和原版预期：合成 `SyntheticBridgePort`（RFC 5737 地址、无网络）。多选 A+B（主行=最后一次点击 B）；右键 A 时主行=A。原版预期：单对象命令作用 `SelectedProfile`，批量选择集合不变。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed lib test`
- `flutter analyze`
- `flutter test test/recheck_r3_prof_controller_test.dart test/recheck_r3_prof_menu_target_test.dart`
- 真实窗口 + FRB 多选右键逐命令对照（本轮未做）。

证据文件位置：`docs/evidence/recheck-fixes/R3-PROF-01-04/`（`README.md`、`observations.json`）。

完成条件：controller 断言主行独立于批量集合；widget 断言菜单「设为活动」「导出完整配置」目标=主行且批量选择不被折叠；analyze/format/测试通过。未做真实 Windows 窗口 + FRB 对照，保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：`features/subs/**` 下的 `shareProfilesQr` 仍是“必须单选”语义且本次按约束不可改；profiles 层新增 `shareProfileQr` 承接主行分享。后续如要统一入口，应在共享层收敛，不在本卡扩大改动面。
- 接口缺口（登记）：主行目前只存在于 `ProfilesState.primaryId`；其它窗口（托盘子菜单等）尚无共享“主窗口当前主行”通道。

本轮实际结果：`ProfilesState` 新增独立 `primaryId`（`selectRow`/`selectRange` 锚点/`navigateSelection`/`selectAll`/`handleRightTap`/`_recompute` 维护与可见性清理）；`profile_actions.dart` 新增 `resolveSingleTarget` 并为编辑/分享/设活动/完整配置导出接入可选 `targetId`，新增 `shareProfileQr`；`profiles_table.dart` 菜单命令消费 `command.primaryId`。命令：`flutter analyze` → No issues；`flutter test test/recheck_r3_prof_controller_test.dart` → 8/8；`flutter test test/recheck_r3_prof_menu_target_test.dart` → 1/1（偶发 `did not complete`，重试通过）。
