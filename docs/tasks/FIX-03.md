# FIX-03 — 特殊节点按 configType 分派专用编辑器

状态：`implemented`（Custom 与 PolicyGroup 的右键编辑→保存→重开→取消全链已在本机真实 Windows 窗口验证；未做原版实机双窗口逐事件对照，故不写 `verified`）。

任务 ID：FIX-03

本次唯一用户流程：已存在的特殊节点（configType=Custom / Outbound / PolicyGroup 策略组 / Chain 链）→ 右键编辑 → 保存 → 重开，按 configType 分派到专用编辑器；取消不改数据库；所有 extra/引用/原文保留；策略组子节点选择/插入/排序/五模式按冻结源码恢复；链式语义一致。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD（本卡只读 dirty tree，未回退任何已修条目）。审查结论见 `docs/evidence/parity-review-2026-10-03/README.md`、`repair-queue.md` FIX-03 行（第 21 行）、`profiles-report.md` PR-02/PR-07/PR-20、`runtime-report.md` RT-08、`all-items.json` 对应条目。复现证据：`editSelectedProfile` 无条件 `showProfileEditor`（`profile_actions.dart:31`），`editSelectedCustom`/`editSelectedGroup` 有定义但三入口（右键/Ctrl+D/双击）均不分流；组编辑器 `_isEligible` 排除全部复杂类型。

对应 feature / field / action / layout ID：`F-PROFILE-002`（编辑节点）、`CFG-014`（PolicyGroup）、`ENUM-007`（五模式 MultipleLoad）、`ACT-PROF-001`（编辑选中节点，右键/Ctrl+D/双击）、`CFG-013`（Custom/Outbound）、`REF-ENT-003`（ChildItems 保序引用）、`REF-ENT-004`（SubChildItems+Filter）。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `ServiceLib/ViewModels/ProfilesViewModel.cs:464/479/484`（`EditServerAsync`：Custom/Outbound→`AddServer2ViewModel`，`IsGroupType()`→`AddGroupServerViewModel`，其余→`AddServerViewModel`)、`ServiceLib/ViewModels/AddGroupServerViewModel.cs:44/90/114/142/199/225`（深拷贝草稿、五模式默认 LeastPing、手工选择仅排除 Custom、多选插入、T/U/D/B 移动、保存校验与字段写回）、`ServiceLib/ViewModels/AddServer2ViewModel.cs:23/52/83/109`（深拷贝、备注+Address 必填、`EditCustomServer` 字段语义、浏览导入/外部编辑）、`ServiceLib/Handler/ConfigHandler.cs:631`（`EditCustomServer` 仅更新 Remarks/Address/CoreType/DisplayLog/PreSocksPort/ProtoExtra）、`ServiceLib/Handler/ConfigHandler.cs:1422`（`AddGroupAllServer` 生成字段 CoreType=Xray/GroupType/SubChildItems/Filter/MultipleLoad=LeastPing）、`ServiceLib/Models/Entities/ProfileItem.cs`（ConfigType）、`ServiceLib/Common/Extension.cs:89`（`IsGroupType`）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：已选中的单个已存节点 DTO；纯函数 `resolveEditorKind(configType)` 决定编辑器（Custom/Outbound→custom，PolicyGroup/ProxyChain→group，其余→generic）。
- 输出：保存走既有 `saveProfile`/engine 路径（`SaveProfileResult`），未新增 IPC；组预览走既有 `groupChildren`（`ProfilesController.groupChildPreview` 尽力封装）。
- 错误：备注必填；组要求子节点或订阅子项至少其一（`error.group_children_required`，与 Rust `validate_group` 一致）；悬空引用/环/非法正则由 Rust 拒绝并回显。
- 取消：各对话框编辑本地 `ProfileDraft` 拷贝，取消直接 pop，`onSave` 零调用，数据库不动（widget + 真实窗口双重断言）。
- 权限：仅本机 UI + FRB/Rust/SQLite；不启动内核、不写系统代理/TUN/注册表/路由、不监听端口、不按名杀进程。
- 持久化：SQLite（`ProfileItem`）；`childItems` 保序、`subChildItems`、`filter`、`multipleLoad`、`groupType`、`customConfigText`、未知扩展键与 legacy 列经保存/重开一致。
- 生效：保存后刷新节点表；重开同一节点回到同一专用编辑器。

允许修改的模块：`apps/desktop/lib/features/profiles/**`（改 `profile_actions.dart`、`group_editor_dialog.dart`、`profiles_controller.dart`）、`apps/desktop/test/**`（新增 `fix03_special_editors_test.dart`；改 `t10_group_editor_test.dart` 适配多选器）、`apps/desktop/integration_test/**`（新增 `ux_parity_fix03_test.dart`）、`docs/evidence/UX-PARITY-FIX-03/**`、本卡、`compat/features.yaml`/`compat/actions.yaml`（仅 notes 追加）。

禁止改变的已有行为：菜单结构/条目/根顺序/32px 高度、`main_shell`、菜单坐标与关闭语义、通用编辑器、`profiles_page` 布局、FRB 生成文件、`crates/subscriptions/**`、features/subs 与 features/settings 目录；不删入口或降分母；不伪造保存/落库结果。

测试夹具和原版预期：合成 vless 叶子（`127.0.0.1:11981/11982`，不连接）+ 合成 Custom（Address `fix03-custom.yaml`，`customConfigText` JSON，PreSocksPort 11820）+ 合成 PolicyGroup（childItems 双叶子保序，multipleLoad=3/RoundRobin，filter `^fix03`）。原版预期：Custom/Outbound 进 AddServer2 对等窗，PolicyGroup/ProxyChain 进 AddGroup 对等窗；取消零落库；备注-only 保存不重置其余字段。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed <6 个改动文件>`
- `flutter analyze`
- `flutter test test/fix03_special_editors_test.dart test/t10_group_editor_test.dart test/t10_custom_editor_test.dart`（逐文件；偶发 exit 79/0xC0000005 引擎崩溃与 did-not-complete 属已知环境抖动，重试即过）
- 相邻回归：`t10_menu_test`、`profiles_keyboard_test`、`profiles_pointer_test`、`t06a_editor_cancel_test`、`t06a_editor_save_test`、`table_actions_test`、`profiles_models_test`
- 真实窗口：`flutter test integration_test/ux_parity_fix03_test.dart -d windows`（隔离 data dir，`V2RAYN_R_DATA_DIR` + `V2RAYN_R_FIX03_EVIDENCE`）
- 不跑 `flutter build windows --release`（根代理统一跑）；无 Rust 改动，不跑 Rust 门禁。

证据文件位置：`docs/evidence/UX-PARITY-FIX-03/`（`observations.json` 11/11、`fix03-run.log`、`01-custom-editor.png`、`02-custom-saved.png`、`03-custom-reopen.png`、`04-group-editor.png`、`05-group-saved.png`、`06-group-reopen.png`、`README.md`）。

完成条件：右键/Ctrl+D/双击三入口对四种特殊 configType 均进专用编辑器（widget 纯函数断言 15 种全覆盖 + 真实窗口 Custom/PolicyGroup 断言）；取消零落库；保存落库且重开同窗同值；组多选插入/保序/T-U-D-B/五模式/订阅子项+Filter 与上游字段一致；门禁通过。Outbound 文件浏览导入与 Chain 内核端到端拆至后续卡（本卡保留入口与分派）。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：Custom `BrowseServer`（文件浏览→配置目录拷贝→Address 改写）与 `EditServer`（外部打开）需要 file-picker 依赖 + bridge 文件导入 API；本卡无新增依赖、无新 bridge 函数，Address 保持手工路径输入。建议后续卡 FIX-03b 引入 `file_selector` 并新增 `import_custom_file` 桥接（不改生成文件，本卡先登记）。
- 接口缺口（登记）：组编辑器预览用的是已落库解析（`groupChildren`），草稿态实时预览需 Rust `resolve_children` 的 draft 版桥接；当前以“已落库解析+保存后刷新确认”诚实标注。建议后续卡评估是否新增草稿预览 IPC。
- 接口缺口（登记）：ProxyChain 内核端到端（Xray/sing-box chain 出站组合）校验归后续卡 FIX-03c；本卡分派/保存/重开与组共路已通。

本轮实际结果：`profile_actions.dart` 新增 `SpecialEditorKind`/`resolveEditorKind` 并改 `editSelectedProfile` 分派（三入口共用）；`profiles_controller.dart` 新增 `groupChildPreview`；`group_editor_dialog.dart` 改手工候选为仅排除 Custom、单一下拉改为多选选择器（全选/追加去重保序）、新增已落库组合预览区与刷新。widget `fix03_special_editors_test.dart` 7/7；`t10_group_editor_test.dart` 6/6；`t10_custom_editor_test.dart` 5/5；相邻回归逐文件全绿；真实窗口 11/11（`recordingComplete=true`，`failures=[]`）；Custom 落库 `preSocksPort=11820/customConfigText/futureFlag/futureTransport/HeaderType` 保留，组落库 `childItems` 保序/`multipleLoad=3/filter/groupType=PolicyGroup`。门禁：format 0 改变、analyze No issues。途中一次集成 assertion 失败查明为测试期望错误（自由顶层键无 SQLite 列，按上游有意丢弃；`store_repo.rs:455` 明示），已修正期望并复绿。另遇两次环境抖动：并行代理改动导致的一次性 Rust 构建失败（`cargo check -p bridge_api` 随后即过，未动他人文件）与三次 flutter_tester did-not-complete（重试即过）。compat 仅追加两条 notes，未删行、未降分母。
