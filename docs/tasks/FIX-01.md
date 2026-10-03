# FIX-01 — 节点右键生成全部策略组：不可变命令上下文

状态：`implemented`（UI 捕获/生成/落库/重开全链已在本机真实 Windows 窗口验证；未做原版实机双窗口逐事件对照，故不写 `verified`）。

任务 ID：FIX-01

本次唯一用户流程：在节点表顶部分组选中一个有效订阅分组后，在节点表右键（节点或空白）→「一键生成策略组 → 全部配置项 / 按地区分组」；菜单关闭后仍按打开时捕获的分组生成，不要求选中节点，生成对象真实落库并在重开后存在。移动分组（ACT-PROF-013）同步改用同一捕获上下文。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `1251cbc6821276e35d082f7739b8b9c15b6f93dd`（本卡只读，未回退审查已修好的菜单坐标/关闭/17 根条目/中文列头/间距）。审查结论见 `docs/evidence/parity-review-2026-10-03/README.md`、`repair-queue.md` FIX-01 行、`root-report.md`「节点右键→一键生成策略组」场景、`profiles-report.md` PR-19/21/18/10、`all-items.json` 的 ACT-PROF-007/008/013 与 F-PROFILE-009。复现证据：`ui-run-04/observations.json` 在 `currentGroupId=nodeGroupId` 前提下仍 1→1、提示「请先选择节点以确定订阅分组」。

对应 feature / field / action / layout ID：`F-PROFILE-009`、`ACT-PROF-007`、`ACT-PROF-008`、`ACT-PROF-013`、`FLG-ENT-005`(Subid)、`CFG-014`(PolicyGroup)、`LAY-PROFILES-004`、`ENUM-007`(五模式)。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `ServiceLib/ViewModels/ProfilesViewModel.cs:141/610/622/660`（`GenGroupAllServer`/`GenGroupRegionServer`/`MoveToGroup`，对象是 `SelectedSub`）、`ServiceLib/Handler/ConfigHandler.cs:1422/1487/2301`（`AddGroupAllServer`/`AddGroupRegionServer`/`MoveToGroup`，生成字段 CoreType=Xray、GroupType、SubChildItems、Filter、MultipleLoad=LeastPing）、`ServiceLib/Models/Entities/ProfileItem.cs:162`（`Subid`）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：菜单打开时捕获的 `CommandContext{targetIds(不可变), primaryId, groupSubId(当前组稳定 ID), viewContext, menuOpenPosition}`；生成命令只读 `groupSubId`，不再读 live selection/session，也不再从选中节点的 `subid` 推分组。
- 输出：`gen_group_all` 返回含新 `ProfileDto` 的 `SaveProfileResult`；`gen_group_region` 返回 `GroupGenResult.profiles`。生成对象走既有 `saveProfile`/engine 保存路径，未新增 IPC。
- 错误：无有效分组时提示「请先在顶部分组中选择一个订阅分组」；分组不存在/保存被拒时提示失败；区域无匹配节点时不生成并如实提示。
- 取消：菜单外点/Esc/点击其它行关闭；关闭只清 UI 会话，不销毁已捕获命令。
- 权限：仅本机 UI + FRB/Rust/SQLite；不启动内核、不写系统代理/TUN、不监听端口。
- 持久化：SQLite（`ProfileItem`）；生成对象字段与重开一致。
- 生效：生成后刷新节点表并按原版选中首个新组（`_pendingSelectIndexId` 语义，见 `ProfilesController.selectGenerated`）。

允许修改的模块：`apps/desktop/lib/features/profiles/**`（新增 `command_context.dart`；改 `context_menu.dart`、`profiles_table.dart`、`profiles_controller.dart`）、`apps/desktop/test/**`、`apps/desktop/integration_test/**`、`docs/evidence/UX-PARITY-FIX-01/**`、本卡、`compat/features.yaml`/`actions.yaml`（仅 evidence/notes 追加）。为修复上游字段缺口做了一处最小 Rust 改动：`crates/application/src/groups.rs` 的 `new_group_all`/`new_group_region` 补 `core_type = Some(CoreType::Xray)`（原版 `AddGroupAllServer`/`AddGroupRegionServer` 均设 Xray，当前漏设导致生成 DTO `coreType=null`）。无签名变化，未跑 FRB 生成。

禁止改变的已有行为：菜单结构/条目/根顺序/分隔/32px 高度、`main_shell`、菜单坐标与关闭语义、编辑器、`profiles_page` 布局；不删入口或降分母；不伪造生成/落库结果。

测试夹具和原版预期：合成订阅 + 合成 `vless://` 节点（`127.0.0.1:11998`，不下载、不连接）；区域节点备注含 HK/US。原版预期：对象=当前订阅组、无需选中节点、无隐藏他组节点、生成字段与冻结 `ConfigHandler` 一致。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed lib test integration_test`
- `flutter analyze`
- `flutter test`（逐文件，`tools/flutter_test_retry.ps1 -PerFile`）
- `flutter build windows --release`
- 真实窗口：`flutter test integration_test/ux_parity_fix01_gen_group_test.dart -d windows`，`V2RAYN_R_FIX01_MODE=generate` 后同 data dir 再跑 `reopen`。
- Rust 最小改动：`cargo fmt -p application -- --check`、`cargo test -p application --test t10_groups_templates --locked`。

证据文件位置：`docs/evidence/UX-PARITY-FIX-01/`（`observations.json`、`reopen-observations.json`、`generate-run.log`、`reopen-run.log`、`generate-all.png`、`generate-region.png`、`reopen-window.png`、`README.md`）。

完成条件：widget/单元覆盖上下文不可变、菜单关闭后仍作用于打开时目标、空选中+有效分组可生成、分组切换后旧命令不作用于新组；真实窗口在有效分组下生成落库、节点表出现、重开仍存在；生成字段符合上游；门禁通过。未做原版实机双窗口逐事件对照，故保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：主窗口当前组目前只在 `ProfilesState.groupSubId` 内；若后续其它窗口/命令需要「主窗口当前稳定组 ID」，缺少共享 getter/命令通道，建议在共享入口统一暴露稳定组 ID，不按备注前缀猜对象（关联 FIX-06）。
- 接口缺口（登记）：`piped` 生成/移动没有专属 IPC，复用 `saveProfile`；若后续需要原子批量移动与部分失败上报，需要新用例契约，本卡不造新 IPC。

本轮实际结果：`command_context.dart` 新增不可变 `CommandContext`；`profiles_table.dart` 菜单动作改读捕获上下文并由 `_moveToGroup`/`_genGroup` 使用 `groupSubId` 与 `targetSubId`（移动迁移改为按稳定 ID，不再解析显示文字）；`profiles_controller.dart` 的 `genGroupRegion` 回传 `GroupGenResult` 并新增 `selectGenerated`。真实窗口 generate run 8/8 通过、reopen run 2/2 通过（`recordingComplete=true`，`failures=[]`）；生成对象 `coreType=xray/groupType=PolicyGroup/subChildItems=<subId>/childItems=null/filter=DEFAULT_ALL/filter/multipleLoad=0/isSub=false`。Rust `t10_groups_templates` 10/10 通过。以 `parity_review_smoke_test.dart` 在同一有效分组前提重跑，`context-generate-group-keeps-menu-target=true`，ui-run-04 场景转绿。门禁：format 0 改变、analyze No issues、release 构建成功；逐文件测试仅 `ux_space03_vless_editor_test.dart` 因并行编辑 `profile_editor_dialog.dart`/`profile_fields.dart` 触发间距断言失败，与本卡无关。
