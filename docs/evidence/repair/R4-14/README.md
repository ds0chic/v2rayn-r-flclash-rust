# R4-14 路由原版提交与导入 — 证据

状态：implemented（应用层/单元与 widget 合同通过；真实平台效果未验证）。

- HEAD `139fbea`，应用基线 `77c74ed`，上游冻结 `7d6a967c18c697f28dc6917122ed3a4993fcf336`，armed=false。
- 夹具：内存 widget fake + 内存 engine 单测；无内核、无端口、无 OS 副作用；127.0.0.1:10808 未触碰。
- 上游对照：`RoutingSettingViewModel`（策略改动即时保存；子编辑 dialog==true → Refresh + IsModified；remove/setDefault/import → Refresh + IsModified；IsModified 关闭时 Reload）；`ConfigHandler.InitRouting(config,true)`（内置分支追加 `V4-` 白/黑/全局，外部模板下载失败回落内置，advanced 不改默认）。

## 完成合同

1. 提交点按原版：子编辑器确定即提交、策略即时保存由 R4-12 保留；独立窗口每个提交各自事务化并立即 Reload，因此关闭不再依赖整窗草稿回写。
2. 一键导入真实执行：`按钮 → import_routing_rules(空 routingId) → import_builtin_routing → application::routing::builtin_import_profiles → engine.save_routing`；导入后 SDK 回传新增方案并在窗口可见；外部模板地址离线时明确报错且不写入。
3. 提交事务化：save/delete/setDefault/strategy/import 均检查写入结果与 reload 结果；失败时不改内存态、窗口保持打开并显示错误。
4. 多选/全选/Enter/Delete 在独立（生产）窗口实现（Ctrl/Shift 多选、Ctrl+A、Delete、Enter 设默认、右键全选菜单、逐方案提交批量删除）；嵌入回退对话框仍单选，登记缺口。
5. 与 R4-12 `RoutingCommitHost` 收敛：保留即时提交通道，去掉整窗确定才会持久化的语义依赖；`import/export` camelCase 与 Custom 排除（FIX-08C）路径未改动。

## 命令结果

见 `commands.log` 与 `observations.json`。要点：`cargo fmt/clippy/test --workspace` 全绿；`flutter analyze` 无问题；`flutter test test/r4_14_contract_test.dart` 5/5；`flutter build windows --release` 成功产出 exe。

## 未完成 / 接口缺口（不自行削减需求）

- **FRB 重生成需求**：本卡在 `crates/bridge_api/src/api/routing.rs` 新增 `import_builtin_routing()` 函数，未改 `frb_generated`。Dart 目前经既有 `import_routing_rules` 的空 routingId 入口到达同一用例；接口整合者重生成后可切换为直接调用。
- 外部模板 `ConstItem.RouteRulesTemplateSourceUrl` 的异步下载导入未接线，暂返回显式错误（无半写）。
- 嵌入回退 `RoutingSettingWindow` 仍是单选。
- 真实 Windows 双引擎窗口、真实 FRB/SQLite 持久化、codegen/内核/流量平台效果未运行 → 未验证。
