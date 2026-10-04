# RE-PROF-03 — 节点表 Ctrl+C 导出分享链接、Ctrl+F 打开分享窗口

状态：`implemented`（widget 层已实测单/多选、空选中分支与分享窗口出现；未做原版实机双窗口逐事件对照，未跑真实 Windows 窗口集成，故不写 `verified`）。

任务 ID：RE-PROF-03

本次唯一用户流程：节点表获得焦点时，`Ctrl+C` 将所选节点的分享链接导出到剪贴板（不克隆、不改行数）；`Ctrl+F` 打开所选节点的分享 QR 窗口；无选中与多选分支与冻结上游一致。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `eeb2932`（工作树干净）。复核结论见 `docs/evidence/parity-recheck-2026-10-04/profiles.md` 的 RE-PROF-03 节（原 P1 缺口：Ctrl+C 走 `copySelectedProfiles` 克隆、Ctrl+F 落入 `emitAction` default log/echo）。FIX-01 的命令上下文修复未覆盖此入口，本卡不改动 FIX-01。

对应 feature / action ID：`ACT-PROF-024`（批量导出分享链接，shortcut Ctrl+C，上游 `ProfilesViewModel.Export2ShareUrlCmd`/`Export2ShareUrlAsync`）、`ACT-PROF-006`（分享选中节点二维码，shortcut Ctrl+F，上游 `ProfilesViewModel.ShareServerCmd`/`ShareServerAsync`）；被隔离保护的 `ACT-PROF-004`（复制选中节点，`CopyServerCmd`）。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`
- `v2rayN/Views/ProfilesView.xaml.cs:220-246`：`PreviewKeyDown` 中 `Ctrl+C -> Export2ShareUrlAsync(false)`、`Ctrl+F -> ShareServerAsync`。
- `ServiceLib/ViewModels/ProfilesViewModel.cs:799-830`：`Export2ShareUrlAsync(false)` 经 `GetProfileItems(true)` 取所有选中项（`SelectedProfiles`），逐个 `GetShareUri` 后写剪贴板；无选中时返回 null 静默返回。
- `ServiceLib/ViewModels/ProfilesViewModel.cs:443-462`：`GetProfileItems` 在 `SelectedProfiles` 为空时返回 null。
- `ServiceLib/ViewModels/ProfilesViewModel.cs:593-608`：`ShareServerAsync` 用 `SelectedProfile.IndexId`（单一 primary，绑定 `lstProfiles.SelectedItem`，见 `ProfilesView.xaml.cs:41`），为空时提示 `PleaseSelectServer`，否则 `ShareServerInteraction` 打开二维码。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：节点表焦点下的键盘事件；`selected`（多选集合）读取自 `ProfilesState`。
- 输出：`Ctrl+C` 经 `bridge.exportProfiles(selected, 'share')` 得分享 URI 文本，`Clipboard.setData` 写入；`Ctrl+F` 经既有 `shareProfilesQr` 打开 `profile-share-qr` 对话框。
- 错误：`Ctrl+C` 无选中时静默返回（上游一致，不改剪贴板、无提示）；导出失败提示 `导出失败：<messageKey>`。`Ctrl+F` 无选中提示 `请先选择节点`（上游 `PleaseSelectServer`）；多选时提示 `请选择单个节点后分享`（本卡选择“分享窗口单选”，见接口缺口）。
- 取消：分享窗口点“关闭”不产生持久化副作用。
- 权限：仅本机 UI + FRB/Rust；不启动内核、不监听端口、不写系统代理/TUN。
- 持久化：`Ctrl+C`/`Ctrl+F` 不新增、不修改、不删除任何节点行；克隆动作仅由菜单 `复制`（`ACT-PROF-004`）触发。
- 生效：`_onKey` 返回 `handled`，表焦点时优先于 `main_shell` 的全局 `Ctrl+C` 绑定（`CallbackShortcuts` 祖先节点），不修改 `main_shell.dart`。

允许修改的模块（本卡实际改动）：`apps/desktop/lib/features/profiles/table_actions.dart`、`apps/desktop/lib/features/profiles/profiles_table.dart`、`apps/desktop/lib/features/profiles/profile_actions.dart`、`apps/desktop/test/**`（改 `table_actions_test.dart`、`profiles_keyboard_test.dart`，新增 `profiles_share_shortcut_test.dart`、`profiles_share_shortcut_empty_test.dart`）、本卡、`docs/evidence/recheck-fixes/RE-PROF-03/**`。

禁止改变的已有行为（本卡未动）：`main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`features/subs/**`（复用 `subs_actions.shareProfilesQr`/`exportProfiles` 但不改其源码）、`features/runtime|settings|monitor|update|backup|routing/**`、`crates/**`；FIX-01 命令上下文与右键分享/导出分享语义；菜单“克隆/复制”语义。

测试夹具和原版预期：合成两节点（`SyntheticBridgePort` id `syn-000000/000001`，不下载、不连接）。原版预期：`Ctrl+C` 剪贴板含所选节点分享 URI 且行数不变；`Ctrl+F` 打开对应节点二维码；无选中时 `Ctrl+C` 静默、`Ctrl+F` 提示选择节点。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed <changed files>`
- `flutter analyze lib/features/profiles test/profiles_share_shortcut_test.dart test/profiles_share_shortcut_empty_test.dart test/table_actions_test.dart test/profiles_keyboard_test.dart`
- `flutter test test/table_actions_test.dart`
- `flutter test test/profiles_share_shortcut_test.dart`
- `flutter test test/profiles_share_shortcut_empty_test.dart`
- `flutter test test/profiles_keyboard_test.dart`
- 未跑：`flutter build windows --release`、全仓 `flutter test`、真实 Windows 窗口集成（按任务约束不跑）。

证据文件位置：`docs/evidence/recheck-fixes/RE-PROF-03/`（`README.md`、`observations.json`）。

完成条件：`actionForKey(Ctrl+C)==export-share-url`、`Ctrl+F==share`；表 `_onKey` 对两者 `handled` 且分别调用分享导出与 QR 窗口，不再落入 controller 默认 log/echo；克隆语义仍仅由菜单 `复制` 触发；单/多选/空选中分支 widget 测试通过；门禁（format/analyze）通过。未做原版实机双窗口对照，故保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：`ProfilesState` 只有 `Set<String> selected`，没有原版 `SelectedProfile`（WPF DataGrid `SelectedItem`，多选时的 primary/当前行）语义，键盘入口无法定位“当前行”。因此 `Ctrl+F` 复用既有 `shareProfilesQr`（要求恰好 1 个选中）；多选时提示 `请选择单个节点后分享`，与上游“对 primary 节点开二维码”存在差异。建议在状态层补稳定 primary id（键盘选择/右键主目标共享），再让分享窗口按 primary 工作；本卡不改选择模型以降风险。
- 接口缺口（登记）：`compat/actions.yaml` 的 `ACT-PROF-024`/`ACT-PROF-006`/`ACT-PROF-004` notes 需追加本卡证据；受 15 文件读取预算限制本卡未读取并追加，建议由根代理补一行 notes 追加（不删行）。

本轮实际结果：
- `table_actions.dart`：新增 `ProfileAction.exportShareUrl='export-share-url'`；`Ctrl+C` 映射由 `copy` 改为 `exportShareUrl`（`Ctrl+F` 仍 `share`）。
- `profiles_table.dart`：`_onKey` 删除 `ProfileAction.copy` 分支，新增 `exportShareUrl -> exportSelectedShareUrls(ref)`、`share -> shareProfilesQr(context, ref)`，均先 `logAction(...,'keyboard')` 并返回 `handled`；菜单 `复制` 仍走 `copySelectedProfiles`。
- `profile_actions.dart`：新增 `exportSelectedShareUrls(WidgetRef)`，空选中静默返回，否则 `bridge.exportProfiles(selected,'share')` 写剪贴板并提示；新增 `package:flutter/services.dart` 导入。
- 测试：`table_actions_test.dart` 断言更新为 `exportShareUrl`；`profiles_keyboard_test.dart` 的 `Ctrl+C` 断言更新为 `export-share-url`，并移除会渲染 QR 路由的 `Ctrl+F` 步骤（改由专用文件覆盖）；新增 `profiles_share_shortcut_test.dart`（单/多选导出 + 行数不变 + `Ctrl+F` 窗口出现并关闭）与 `profiles_share_shortcut_empty_test.dart`（空选中静默/提示）。

门禁结果：`dart format --set-exit-if-changed` 0 改变；`flutter analyze` 5 项 No issues；`table_actions_test` 6/6、`profiles_share_shortcut_test` 1/1、`profiles_share_shortcut_empty_test` 1/1、`profiles_keyboard_test` 1/1 通过（`profiles_keyboard_test` 首两次运行 exit 79 原生崩溃，按任务说明重跑；移除 QR 路由渲染后稳定通过）。
