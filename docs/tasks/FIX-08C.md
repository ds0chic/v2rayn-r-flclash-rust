# FIX-08C — 路由规则窗口草稿余量（导入/导出/删除/选择器）

状态：`implemented`（widget + Rust 单测通过；未做原版实机双窗口逐事件对照，
故不写 `verified`）。

任务 ID：FIX-08C

本次唯一用户流程：路由规则窗口内——外部模板导入（文件/剪贴板/URL）→ 编辑
草稿 → 导出选中 → 重开；删除规则需确认且取消不删；出站选择器排除 Custom。
解析失败/取消不碰草稿，任一导入错误不关窗、不报成功。

前置任务及已验证证据：`docs/tasks/FIX-08.md`（单草稿/单次保存/合并语义已落
地）、`docs/tasks/FIX-08C-ROUTING-DRAFT.md`（登记卡）、冻结 v2rayN 7.25.4
commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。证据见
`docs/evidence/UX-PARITY-FIX-08C/README.md`。

对应 feature / field / action / layout ID：`ACT-RR-002`（删除确认
RemoveServer）、`ACT-RR-003`（导出选中 camelCase 去 Id）、`ACT-RR-004`（文件
导入）、`ACT-RR-005`（剪贴板导入）、`ACT-RR-006`（URL 导入）、`F-ROUTING-004`
（出站选择器排除 Custom）、`LAY-ROUTINGRULESET-001`、`LAY-ROUTINGRULEDETAIL-001`。

必读上游文件、符号和固定 commit（`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967`）：
- `ServiceLib/ViewModels/RoutingRuleSettingViewModel.cs:49-61`（Import 命令）、
  `:150-171`（RuleRemoveAsync：`RemoveServer` 确认，取消不删）、
  `:173-202`（RuleExportSelectedAsync：camelCase + `Id=null` + `IgnoreNull`）、
  `:253-311`（文件/剪贴板/URL 导入）、`:313-343`（`AddBatchRoutingRulesAsync`：
  Yes→追加、No→替换，新 GUID）。
- `ServiceLib/ViewModels/RoutingRuleDetailsViewModel.cs:126-140`（SelectProfileAsync：
  `SetConfigTypeFilter([Custom], exclude: true)`）。
- `ServiceLib/Models/Entities/RulesItem.cs`（13 字段，含 `Type`/`RuleType`）。
- `ServiceLib/Models/Configs/ConfigItems.cs`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：规则集窗口草稿 `_rules`（窗口本地）+ 导入文本（文件/剪贴板/URL body）。
- 输出：导入合并只改 `_rules`；保存走单次 `save_routing`（FIX-08 语义）；
  导出写剪贴板（Dart 草稿）或经 FRB `export_routing_rules`（camelCase）。
- 错误：解析失败/下载失败/空选择 → 就地提示，不关窗、不改草稿、不落盘。
- 取消：文件选择器取消、导入对话框取消、删除确认取消 —— 均不碰草稿。
- 权限：仅本机 UI + FRB/Rust；不启内核、不写系统代理/TUN、不监听端口。
- 持久化：SQLite `RoutingItem.RuleSet`；草稿在保存前不落盘。
- 生效：导入/移动/删除只作用于草稿；保存后重开一致。

允许修改的模块：`apps/desktop/lib/features/routing/**`、
`apps/desktop/test/**`、`crates/application/src/routing.rs`、
`crates/bridge_api/src/api/**`（routing 相关函数）、
`docs/evidence/UX-PARITY-FIX-08C/**`、本卡、`compat/actions.yaml` /
`compat/features.yaml`（仅 evidence/test_ids/notes 追加）。

禁止改变的已有行为：`main_shell`、`app.dart`、两处 `frb_generated`、
features/settings、features/profiles（并行代理在改）、subscriptions/runtime/
monitor/update/backup、`engine.rs`、`dns.rs`、`updater/**`；不删入口或降分母；
不伪造导入/导出/删除结果；不跑全仓 fmt、不跑 `flutter build windows --release`。

测试夹具和原版预期：合成路由方案 + 合成规则 JSON（camelCase `outboundTag`/
`domain`）；SyntheticBridgePort 节点（含一个 `ConfigType.custom`）。原版预期：
导入先问追加/替换、失败退出码 -1 且不改列表、导出 camelCase 去 Id、删除先问
`RemoveServer`、选择器排除 Custom。

本次已通过的命令/场景：
- Rust：`cargo fmt -p application -p bridge_api -- --check`；
  `cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings`；
  `cargo test -p application --lib routing`（5 passed）；
  `cargo test -p bridge_api --locked`（43 passed）。
- Flutter：`flutter test test/fix08c_routing_draft_test.dart`（3 passed）；
  `flutter test test/fix08_routing_draft_test.dart test/fix08_draft_unit_test.dart`
  （7 passed，FIX-08 回归）。
- `dart format`（仅改动文件）；`flutter analyze` 本卡文件 0 issue。
- 真实窗口集成测试与 release 构建未跑（根代理统一）。

证据文件位置：`docs/evidence/UX-PARITY-FIX-08C/README.md`（含命令与结论，
未生成截图）。

完成条件：导入成功进草稿可编辑、失败/取消不留痕；导出 JSON 与原版一致且可
再导入；删除需确认且取消不删；选择器排除 Custom；FIX-08 草稿 ID/删空/未知键
语义不回退；门禁通过。原版实机双窗口逐事件对照未做，故保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。
- 接口缺口（登记）：`crates/domain/src/routing.rs::export_rules` 仍 snake_case，
  该文件不在本卡允许修改清单；FRB 面（bridge_api/application）已对齐
  camelCase。若需 domain 层统一，请根代理确认后可让本卡扩展范围。
- 未完成（登记）：多选/键盘 parity 只完成可测部分（点击多选、删除确认）；
  Ctrl/Shift/Ctrl+A 选择、T/U/D/B 快捷键绑定未实现，建议后续卡或本卡扩展。
- 未验证：真实 Windows 窗口的重开/取消场景未跑。

本轮实际结果：
- `crates/application/src/routing.rs` 新增 `parse_imported_rules_compat`
  （camelCase/snake_case、数组或逗号/换行字符串、`ruleType` 序号或名称、未知键
  进 `extra`、每规则新 id、非法输入报错）与 `export_rules_camel`（camelCase、
  去 id、空值省略）；`merge_imported_rules`/`export_selected_rules` 改走上述
  函数；新增 3 个内联单测。
- `crates/bridge_api/src/api/routing.rs::export_routing_rules` 不再走 engine 的
  snake_case 分支，改为取规则后调 `application::export_rules_camel`，全量/选中
  统一 camelCase。
- `apps/desktop/lib/features/routing/routing_actions.dart::pickRulesFromFile`
  改用 `file_selector.openFile`（json/txt），取消/读取失败保草稿。
- `apps/desktop/lib/features/routing/routing_windows.dart`：`_selectProfile`
  排除 `ConfigType.custom`；`_confirmRemove` 加删除确认（取消不删）。
- `apps/desktop/test/fix08c_routing_draft_test.dart` 新增 3 用例；
  `fix08_routing_draft_test.dart` 删除流程补确认步骤；`compat` 台账 append。
