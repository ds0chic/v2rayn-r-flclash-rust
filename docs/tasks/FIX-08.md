# FIX-08 — 参数修改→应用：窗口草稿提交语义

状态：`implemented`（第一唯一流程的 UI→Rust→持久化→重开全链已在本机真实
Windows 窗口验证；未做原版实机双窗口逐事件对照，故不写 `verified`）。

任务 ID：FIX-08

本次唯一用户流程：在设置窗口（参数设置）改动参数后点应用，使用眼前草稿
一次性提交；点取消/关闭（含 X、Esc）不落盘，不留提前落盘结果；保存失败时
窗口不关闭、不显示成功。成功后按原版时机生效（immediate 字段直接应用，
core 生效走 apply）并刷新。DNS 导入/应用、路由草稿各自另卡（FIX-08B/08C），
但本卡保证：不同 DTO 未改字段/未知字段保留；路由顶部全局策略按冻结对象写；
DNS 全局 FakeIP 不被普通设置修改擦掉。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit
`7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD
`71dac24c0cb43a3d62da0887905bdf4ebda5b1a2`（只读基线；
审查已修好的菜单坐标/关闭等未回退）。审查结论见
`docs/evidence/parity-review-2026-10-03/README.md`、`repair-queue.md` FIX-08 行
（SET-06/11/07/08/09/10）、`settings-report.md`、`settings-items.json`、
`root-report.md`。

对应 feature / field / action / layout ID：`ACT-OPT-001`、`ACT-MAIN-025`、
`ACT-MAIN-026`、`ACT-ROUTE-005`、`ACT-RR-001`、`ACT-RR-007`、`ACT-RR-010`、
`ACT-DNS-001`、`F-ROUTING-002`、`F-ROUTING-003`、`F-DNS-001`、
`FLD-CFG-056`(AutoRun)、`FLD-CFG-114/115`(DomainStrategy)、`FLD-CFG-162`
(GlobalFakeIp)、`FLD-ENT-125..150`、`LAY-OPTSET-001`、`LAY-ROUTINGSET-001`、
`LAY-ROUTINGRULESET-001`、`LAY-ROUTINGRULEDETAIL-001`、`LAY-DNSSET-001`、
`INV-WPF-015/016/017/018/019`。

必读上游文件、符号和固定 commit：
`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的
`ServiceLib/ViewModels/OptionSettingViewModel.cs:293-298`（端口校验非法即
返回）、`:308-312`（needReboot 判定）、`:400-407`（SaveConfig 成功后才
UpdateTask/Reset/关闭）、`ServiceLib/ViewModels/RoutingSettingViewModel.cs:75-76`
（Init 读 `RoutingBasicItem`）、`:110-115`（SaveSettingsAsync 写全局字段）、
`ServiceLib/ViewModels/RoutingRuleSettingViewModel.cs:94-99`（`_rules` 草稿；
新方案空列表）、`:124-148`（新增进草稿）、`:204-222`（草稿内移动）、
`:224-249`（备注校验、RuleNum、单次 SaveRoutingItem）、`:313-343`
（导入赋新 Id、追加/替换只改 `_rules`）、
`ServiceLib/ViewModels/RoutingRuleDetailsViewModel.cs:59-69`
（新规则进入即 GUID + proxy + enabled）、`:82-124`（有任一匹配准则即可，
不强制出站）、`ServiceLib/ViewModels/DNSSettingViewModel.cs:49-61`
（导入命令只改窗口属性）、`:106-176`（全部文本先验后存）、`:178-196`
（依次存 SimpleDNS/Xray/sing-box 后 SaveConfig 关窗）、
`ServiceLib/Handler/ConfigHandler.cs:120`（GlobalFakeIp 缺省 true）、
`:205-230`（SaveConfig 整文件写）、`:2322-2337`（SaveRoutingItem）、
`:2733-2753`（SaveDNSItems）、`:2801-2814`（InitBuiltinSimpleDNS）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：各窗口内草稿（参数窗口 `_draft` 整文档；规则集窗口 `_rules` 列表；
  DNS 窗口各页控制器）。应用/保存只读眼前草稿，不读过期已存文档。
- 输出：参数保存走 `saveSettingsJson`（整文档 JSON，未知键与 null/空串区分
  保留）；规则集保存走单次 `saveRouting`（RuleSet 内嵌序列化，删空也存）；
  DNS 保存走 `saveSimpleDns` + 两行 `saveDns`（保存前全部自定义文本先验）。
- 错误：端口非法 / 备注空 / DNS 文本非法 / 后端拒绝时窗口不关闭、不显示
  成功、不做 apply；错误信息就地展示。
- 取消：取消/关闭/X/Esc 只关窗，草稿丢弃；AutoRun 开关只改草稿，Run 键仅
  在保存成功且值变化时写一次；DNS 导入默认只预览不落盘；路由移动/导入/
  导出只作用于草稿。
- 权限：仅本机 UI + FRB/Rust/SQLite 与（保存成功后）Run 键同步；不启动
  内核（应用走既有 apply 通道）、不写系统代理/TUN、不监听端口（测试端口
  ≥11808 且只做存储值）。
- 持久化：SQLite（RoutingItem RuleSet 文本列、DNSItem 行）+ guiNConfig.json
  整树；引擎保存做 extras 合并（DTO 无 extra 通道时按 id 补回未知键）。
- 生效：immediate 字段保存后直接应用（主题/双击等）；core 相关置
  desired revision 并提示未应用，由应用入口 apply；成功后刷新列表与状态。

允许修改的模块：`apps/desktop/lib/features/settings/option_setting_window.dart`、
`apps/desktop/lib/features/routing/{routing_controller.dart,routing_windows.dart,routing_actions.dart,dns_window.dart}`、
`apps/desktop/lib/bridge/bridge_port.dart`（仅 Synthetic 测试替身语义对齐）、
`crates/application/src/{settings.rs 未动、routing.rs,dns.rs,engine.rs}`、
`crates/bridge_api/src/api/dns.rs`（`save_simple_dns` 合并）、
`apps/desktop/test/fix08_*.dart`、`apps/desktop/integration_test/ux_parity_fix08_test.dart`、
`docs/evidence/UX-PARITY-FIX-08/**`、本卡与两张后续卡、
`compat/{actions,features}.yaml`（仅 evidence/test_ids/notes 追加）。

禁止改变的已有行为：`main_shell`、bridge 生成文件、subscriptions、profiles
（并行代理在改）；不删入口或降分母；不伪造保存/落库/应用结果；不跑
`flutter build windows --release`（根代理统一跑）；不新增 bridge API
（缺口见下）。

测试夹具和原版预期：合成端口值（11821/11822，只存不绑）、合成规则
（`geosite:google/cn/private` + proxy/direct/block 出站）、合成 DNS
（`119.29.29.29`、内置模板文本）。原版预期：提交边界=窗口、
AutoRun 在 SaveConfig 后更新、策略写全局、导入只改窗口属性、
GlobalFakeIp 无窗口控件且 LoadConfig 缺省 true。

本次必须通过的命令/真实场景：
- `dart format <changed files>`（只格式化改动文件）
- `flutter analyze`（No issues）
- `flutter test test/fix08_*.dart`（6 文件：apply/cancel/error/路由草稿/DNS 草稿/纯单元）
- Rust：`cargo fmt -p application -p bridge_api -- --check`、
  `cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings`、
  `cargo test -p application --locked`、`cargo test -p bridge_api --locked`
- 真实窗口：`flutter test integration_test/ux_parity_fix08_test.dart -d windows`，
  按 cancel→apply→reopen 顺序同 data dir 各跑一次
 （`V2RAYN_R_DATA_DIR` 隔离、`V2RAYN_R_OPEN_SETTINGS=1`）。
- `flutter build windows --release` 未跑（根代理统一跑）。

证据文件位置：`docs/evidence/UX-PARITY-FIX-08/`（`cancel/apply/reopen-observations.json`、
5 张截图、`README.md`）。

完成条件：改参→应用保存眼前草稿并生效、改参→取消→重开无变化、
改参→应用→重开生效（真实窗口）；新规则稳定唯一 ID；移动/导入/导出同草稿；
删空可存；错误不关窗不报成功；策略写全局；GlobalFakeIp 与未知字段保留
（Rust + widget 回归测试）；门禁通过。原版实机双窗口逐事件对照未做，
故状态保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：无原子“方案+规则+策略”单事务桥接函数。本卡用客户端
  RuleSet 内嵌序列化 + 单次 `save_routing` 绕过；若后续流程需要跨对象事务，
  建议新增用例契约（本卡不造新 IPC、不改生成文件）。
- 接口缺口（登记）：`RoutingRuleDto`/`DnsProfileDto` 无 `extra_json` 通道
  （加字段需 FRB 重新生成，属根代理职责）。本卡用引擎侧按 id 合并未知键
  兜底；行级未知列仍按上游固定列语义丢弃（与上游实体类一致，不算缺口）。
- 接口缺口（登记）：DNS（SimpleDNS + 双核行）无多行原子保存 API。客户端
  先验后写已把窗口收窄到 Rust 侧二次拒绝的极小情形；真原子需组合保存用例。
- 接口缺口（登记）：Rust `export_rules` 输出 snake_case，与上游 camelCase
  `RulesItem` JSON（去 Id）不一致。本卡 Dart 草稿导出用上游形状且可重导入；
  Rust 侧对齐（含测试更新）归 FIX-08C。
- 有意偏离（记录）：上游 `SaveRoutingAsync` 保存时重发全部规则 Id，本实现
  保持草稿 Id 稳定（编辑/移动/选择可定位；规则仅按位置引用，无外部外键）。
- 有意保留（记录）：详情校验仍把 inboundTag 计入有效准则（与 Rust
  `validate_rule` 一致），上游详情页不计入；属字段级校验差，归 FIX-16 逐字段
  排队，不在本卡改。

本轮实际结果：`option_setting_window.dart` 应用=先存草稿后 apply、
取消零落盘、AutoRun 延后写、端口前验；`routing_controller.dart` 新增
`newRuleId/parseImportedRuleDtos/exportDraftRulesJson/rulesToRuleSetJson`；
`routing_windows.dart` 策略读写全局、规则集全草稿化单次保存、详情透传
ruleKind、行内 T/U/D/B 移出双击竞技场；
`routing_actions.dart` 改为草稿导入流（解析失败/取消不碰草稿）；
`dns_window.dart` 导入仅预览、保存透传 GlobalFakeIp、先验后写、应用先存；
Rust `preserve_extras/preserve_dns_extras` + `save_simple_dns` 合并。
widget 6 文件 11 用例通过；Rust application 全量 + bridge_api 42 用例通过；
真实窗口 cancel/apply/reopen 三模式全绿（apply 在零节点下返回结构化
`error.no_active_profile`，设置照常落盘，无崩溃无伪造运行态）。
fmt/clippy/analyze 全过；release 构建未跑。
回归：t11/t12a/t13/t21e-dialogs/fix06 相关文件通过（fix06 多文件同进程
偶发未完成属已知 flaky，单跑全过）。
