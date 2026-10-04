# RE-PROF-10 — 策略组“刷新预览”使用当前草稿而非存量

状态：`implemented`（Dart widget/纯函数覆盖已通过；未做原版实机双窗口逐事件对照，未跑真实 Windows 窗口链路，故不写 `verified`）。

任务 ID：RE-PROF-10

本次唯一用户流程：在策略组/代理链编辑器中修改草稿的 `ChildItems`、五模式、`SubChildItems`、`Filter` 后点“刷新预览”，展示的是**当前草稿**的解析结果，而不是已落库版本；新建组在保存前也能预览；取消后库不变、预览无副作用；保存后重开与预览一致。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；复核结论见 `docs/evidence/parity-recheck-2026-10-04/profiles.md` 的 RE-PROF-10。开始 HEAD `256c3dd`，工作树干净；本卡只读 `main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`features/subs/**`、`crates/**` 及禁止清单内文件。

对应 feature / field / action / layout ID：`ACT-GROUP-005`(策略组-预览页签刷新)、`ACT-GROUP-001`(添加策略组)、`F-PROFILE-009`、`FLG-ENT-005`(Subid)、`CFG-014`(PolicyGroup)、`ENUM-007`(五模式)。

必读上游文件、符号和固定 commit：
- `ServiceLib/ViewModels/AddGroupServerViewModel.cs:192-222`：`GetUpdatedProtocolExtra` 取当前 `ChildItemsObs`/`PolicyGroupType`/`SelectedSubItem?.Id`/`Filter`；`UpdatePreviewList` 调用 `GroupProfileManager.GetChildProfileItemsByProtocolExtra`；
- `ServiceLib/Manager/GroupProfileManager.cs:78-125`：`GetChildProfileItemsByProtocolExtra` = `GetSubChildProfileItems`（`ProfileItems(subid)`，过滤 `IsValid() && (!IsComplexType() || Outbound) && IsRegexMatch(Remarks, Filter)`）后接 `GetSelectedChildProfileItems`（`GetProfileItemsOrderedByIndexIds`）；
- `ServiceLib/Common/Utils.cs:737-761`（`IsRegexMatch`：空模式=true、空输入=false、非法模式/超时=true）。
- 当前持久化生成路径：`crates/application/src/groups.rs:233-270`（`resolve_children`/`resolve_sub_children`，订阅匹配按 `IndexId` 排序后接显式 `ChildItems` 顺序并去重）→ `Engine::group_children` → `bridge_api::group_children`。预览必须与该路径一致，才能保证“保存后重开与预览一致”。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：编辑器已加载的 `allProfiles` 与当前草稿（`ChildItems` 列表、`SubChildItems` 选择、`Filter` 文本、组自身 `subid`）。`self` 哨兵按组自身 `subid` 解析。
- 输出：解析后的有序 `ProfileDto` 预览；订阅匹配优先（leaf/Outbound、备注正则、按 `IndexId` 排序），再接显式 `ChildItems` 顺序并去重。
- 错误：备注正则为空=全部匹配；空备注不匹配；非法正则按上游 `IsRegexMatch` 视为 `true`（匹配全部）。
- 取消：预览解析为纯函数，无桥接、无持久化；关闭窗口不落库、不改库。
- 权限：仅本机 UI；不启动内核、不监听端口、不写系统代理/TUN、不碰 10808。
- 持久化：预览本身不持久化；保存仍走既有 `saveProfile`（Rust `normalize_group`+`validate_group`）。
- 生效：`reload()` 后节点表刷新；重开解析与预览使用同一套语义。

允许修改的模块：`apps/desktop/lib/features/profiles/{group_editor_dialog.dart,profiles_controller.dart,profile_actions.dart}`、`apps/desktop/test/**`、本卡、`docs/evidence/recheck-fixes/RE-PROF-10/**`、`compat/actions.yaml`（仅追加 notes）。未改 `main_shell.dart`、`app.dart`、`frb_generated.*`、`lib/bridge/api/**`、`features/*` 其它域、`crates/**`。

禁止改变的已有行为：FIX-03/03C 已提交的专用编辑器分派、多选选择器、五模式、链语义与既有 T/U/D/B；FIX-03 的 `previewChildren` 测试注入仍作为可选覆盖保留。

接口缺口（登记）：无新增。预览在 Dart 侧用已加载的 `allProfiles` 解析（合同优先项 4），未新增桥接；`ProfilesController.groupChildPreview`（旧存量解析入口）已删除，不再被生产路径引用。`BridgePort.groupChildren` 仍保留供持久化解耦路径使用。

测试夹具和原版预期：合成节点（RFC 5737 `192.0.2.1:443`，不下载、不连接）、合成订阅 `sub-1`/`sub-2`、合成组。原版预期：改草稿后刷新预览看草稿；新建可预览；取消库不变；订阅子项按 `Filter` 备注正则并按 `IndexId` 排序；链按草稿 `ChildItems` 顺序；状态不落库。

本次必须通过的命令/真实场景（实际运行）：
- `dart format`（改动文件）：Passed（2 changed）。
- `flutter analyze`：No issues found。
- `flutter test test/reprof10_group_preview_test.dart`：8 passed。
- `flutter test test/fix03_special_editors_test.dart test/fix03c_proxy_chain_editor_test.dart test/t10_group_editor_test.dart`：20 passed（回归）。
- `flutter test test/recheck01_group_inheritance_test.dart test/t10_groups_panel_test.dart test/ux_space01_group_flow_test.dart test/ux_parity_fix01_gen_group_ui_test.dart test/ux_parity_fix01_gen_group_guard_test.dart`：`t10_groups_panel_test` 首跑 `did not complete`（flutter_tester 偶发），单独重跑 2 passed；其余通过。

证据文件位置：`docs/evidence/recheck-fixes/RE-PROF-10/`（`README.md`、`observations.json`、`runs.txt`）。

完成条件：widget 覆盖“改草稿+Filter→预览显示草稿、取消不落库、新建保存前可预览、订阅子项+Filter 解析、链草稿顺序”；纯函数覆盖订阅资格/排序/去重/`self`/非法正则；FIX-03/03C/T10 回归通过；`flutter analyze` 通过。未跑真实 Windows 窗口与原版实机双窗口，保持 `implemented`。

本轮实际结果：见 `docs/evidence/recheck-fixes/RE-PROF-10/README.md`。
