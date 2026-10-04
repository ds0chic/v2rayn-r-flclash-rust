# R3-WPF-ROUTING — 路由设置窗口可见结构/文案对齐证据

日期：2026-10-04。基线 repo HEAD `f9ebe29`；冻结原版 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

对照来源：`docs/evidence/recheck-fixes/R3-WPF-COMPARE/README.md` §4.3（R1–R4）与 `original-routing.png` vs `rc-routing.png`；冻结 `v2rayN/v2rayN/Views/RoutingSettingWindow.xaml`、`RoutingSettingWindow.xaml.cs`、`ServiceLib/ViewModels/RoutingSettingViewModel.cs`、`ServiceLib/Resx/ResUI.zh-Hans.resx`。

## 逐项结论

| # | 项 | 冻结上游（XAML + zh-Hans resx） | 修复前 RC | 修复后 RC |
| --- | --- | --- | --- | --- |
| 标题 | Title | `路由设置`（menuRoutingSetting） | 路由设置 | 不变（已一致） |
| R2 | 区块标题 | `预定义规则集列表`（TbRoutingTabRuleList，TabItem header） | 无标题 | 蓝色下划线 `预定义规则集列表`（`routing-block-title`） |
| R4 | 顶部操作 | ToolBarTray：`添加规则集` + `一键导入规则集` | 无（在底部） | 顶部 `添加规则集`（`routing-add`）+ `一键导入规则集`（`routing-import-builtin`） |
| R1 | 策略标签 | `域名解析策略` / `sing-box 域名解析策略` | `域名策略` / `域名策略 (sing-box)`，空值显 `(空)` | 上游文案；空值显示为空 |
| R3 | 列头 | `别名`(LvRemarks) / `数量`(LvCount) / `排序`(LvSort) / `可选地址 (Url)`(LvUrl) / `自定义图标`(LvCustomIcon) | `备注 / 规则数 / 排序 / URL / 状态` | 上游五列；去掉 `状态` 列，行展示 `customIcon`，活动行用背景高亮（对应上游 IsActive DataTrigger） |
| R4 | 底部按钮集 | 无底部动作栏；命令在工具栏 + 行上下文菜单（`添加规则集/移除所选规则/全选/设为活动规则/一键导入规则集`） | `添加 / 删除 / 设为默认 / 应用 / 关闭` | 底部仅 `关闭`（内嵌 dialog 关闭入口，窗口形态另卡）；`添加规则集/移除所选规则/设为活动规则/一键导入规则集` 移入行右击菜单 |

## 未改动 / 缺口

- 窗口形态（独立窗口 vs 内嵌 dialog、标题栏关闭、`DialogResult=IsModified`）不在本轮，登记后续卡。
- `一键导入规则集` 上游为 `ConfigHandler.InitRouting(config, true)`；Rust 侧无对应用例，本轮落 `reload()` + 提示，登记接口缺口。
- 行 `全选`（多选）未实现，登记后续卡。

## 命令与结果

见 `test-run.log`。摘要：
- `dart format lib/features/routing/routing_windows.dart test/r3_wpf_routing_structure_test.dart` → 2 files formatted。
- `flutter analyze lib/features/routing test/r3_wpf_routing_structure_test.dart` → No issues found。
- `flutter test test/r3_wpf_routing_structure_test.dart test/t11_routing_test.dart test/fix08_routing_draft_test.dart test/fix08c_routing_draft_test.dart` → 13/13 passed。
- `flutter test test/t21e_dialogs_responsive_test.dart`（1000×700）→ 首次偶发 exit，`--reporter expanded` 重试 1/1 passed。

## 安全边界

仅运行合成 bridge 的 widget 测试；未监听/占用 `127.0.0.1:10808`；未改系统代理/注册表/路由/TUN；未读用户凭据；未跑 `flutter build windows`。
