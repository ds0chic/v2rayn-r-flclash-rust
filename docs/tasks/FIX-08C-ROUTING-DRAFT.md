# FIX-08C — 路由草稿余量（FIX-08 拆分）

状态：`identified`

本次唯一用户流程：路由方案编辑会话内的剩余草稿操作——外部规则模板导入
（内置/URL）、导出→重导入往返、多选/键盘、节点选择器 parity。
FIX-08 已保证单草稿语义（稳定 ID、移动/导入/导出同草稿、删空可存、
失败不关窗）；本卡补齐草稿操作的完整 parity。

前置：`docs/tasks/FIX-08.md`（草稿/单次保存/合并语义已落地）、
冻结 `RoutingRuleSettingViewModel.cs` 全文件（`RuleExportSelectedAsync`
camelCase 去 Id、`AddBatchRoutingRules` 新 GUID、SelectedSources 多选）、
`RoutingRuleDetailsViewModel.SelectProfileAsync`（排除 Custom 的节点选择器）、
`ConfigHandler.InitRouting/InitExternalRouting`（外部模板入口）。

对应 ID：`ACT-RR-002/003/004/005/006/008/009/011/012`、`ACT-ROUTE-001/008`、
`F-ROUTING-003/004/005`、`LAY-ROUTINGRULESET-001`、`LAY-ROUTINGRULEDETAIL-001`。

输入/提交/取消/错误/生效：
- 外部模板导入：`InitRouting/InitExternalRouting` 入口缺失，
  `ConstItem` 路由源 URL 未消费，后续下载未接通（SET-19）。
- 多选 parity：Ctrl/Shift/Ctrl+A、上/下移动快捷键、右键同结构；
  删除确认（`RemoveServer`）与取消不删。
- 导出：Rust `export_rules` snake_case 与上游 camelCase 去 Id 不一致
  （FIX-08 接口缺口）；Dart 草稿导出已用上游形状，本卡对齐 Rust 侧
  （含测试更新）或明确保留差异。
- 节点选择器：排除 Custom 的完整节点表、过滤、确认/取消、Remarks 写回；
  同备注多节点可区分；`ruleKind`(Type) 的编辑控件与往返。
- URL 导入：空 Url 提示 `MsgNeedUrl`（已做）；下载失败/非法文本失败保留
  草稿、错误明确。

允许修改：`routing_windows.dart`、`routing_actions.dart`、
`routing_controller.dart`、路由相关 bridge/Rust（不改生成文件；
需新 API 则登记缺口）、测试与证据。

禁止：改动 subscriptions/profiles/main_shell/生成文件；全仓 format；
release 构建（根代理统一跑）。

测试：widget（多选/删除确认/取消、选择器排除 Custom、导出重导入往返、
外部 URL 失败保留草稿）＋ Rust（casing 对齐、批量导入替换/追加）；
真实窗口集成证据归 `docs/evidence/UX-PARITY-FIX-08C/`。

完成条件：任一导入/导出/删除分支取消不留痕、错误不关窗不报成功、
草稿 JSON 可导出重导入、重开一致。
