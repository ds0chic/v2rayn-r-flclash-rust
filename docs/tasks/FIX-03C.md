# FIX-03C — ProxyChain 链节点专用编辑与链式生成语义

状态：`implemented`（ProxyChain 编辑器行为由 widget 测试 7/7 覆盖；链式生成语义由 Rust 合成链单测（Xray dialerProxy / sing-box detour）断言；未做原版实机双窗口逐事件对照，故不写 `verified`）。

任务 ID：FIX-03C

本次唯一用户流程：ProxyChain 链节点：右键编辑（三入口共用 `resolveEditorKind`）→ 子节点顺序 / 五模式 MultipleLoad / SubChildItems+Filter → 保存 → 重开同编辑器同值 → 生成配置时按上游 ProxyChain 语义串联出站（反向 `ChildItems` + `dialerProxy`/`detour`）。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `a940c21`（FIX-03 已通：`resolveEditorKind` 将 PolicyGroup/ProxyChain→group 编辑器，`docs/evidence/UX-PARITY-FIX-03/README.md`；FIX-03B 已通 Custom/Outbound 浏览）。FIX-03 任务卡 §46 明确「ProxyChain 内核端到端校验归后续卡 FIX-03c；本卡分派/保存/重开与组共路已通」。本卡即补齐该方向：链节点编辑器显式覆盖 + 链式生成语义断言。

对应 feature / field / action / layout ID：`F-PROFILE-002`（编辑节点分派）、`CFG-014`/`EConfigType.ProxyChain`(102)、`ENUM-007`（五模式 MultipleLoad）、`ACT-MAIN-015`（添加链式代理）、`REF-ENT-003`（ChildItems 保序引用）、`REF-ENT-004`（SubChildItems+Filter）、`ACT-PROF-001`（编辑选中节点）。

必读上游文件、符号和固定 commit：
- `ServiceLib/ViewModels/ProfilesViewModel.cs:464/479/484`：`EditServerAsync` 按 `IsGroupType()` 分流到 `AddGroupServerViewModel`（ProxyChain 属组类型）。
- `ServiceLib/ViewModels/AddGroupServerViewModel.cs:44/81/90/106/114/199/224/237`：ProxyChain 与 PolicyGroup 共用同一编辑器——深拷贝草稿、五模式 `PolicyGroupType`、`AddChildAsync` 多选（`SetConfigTypeFilter([Custom], exclude:true)`）、T/U/D/B 移动、`GetUpdatedProtocolExtra` 写回 `ChildItems/MultipleLoad/SubChildItems/Filter`、保存要求备注且（子节点或订阅子项至少其一）。
- `ServiceLib/Handler/GroupProfileManager.cs:72/78/110`：`GetChildProfileItemsByProtocolExtra` = 订阅子项优先 + 手工子项（保序）；`HasCycle` DFS 环检测。
- `ServiceLib/Services/CoreConfig/V2ray/V2rayOutboundService.cs:656-740`：`BuildChainOutboundsList` — `ChildItems` 反序，逐出站 `streamSettings.sockopt.dialerProxy` 串联（`FillDialerProxy`，xhttp 另写 `downloadSettings.sockopt.dialerProxy`）；入口 tag=`proxy`。
- `ServiceLib/Services/CoreConfig/Singbox/SingboxOutboundService.cs`：sing-box 同语义，用 `detour` 串联。
- `ServiceLib/Handler/Builder/CoreConfigContextBuilder.cs:258-315`：`BuildSubscriptionChainNodeAsync` 构造虚拟 ProxyChain 头/尾节点（订阅 prev/next）；`:440-508` `TraverseGroupNodeAsync` 组/链子节点遍历、环/去重、写回 `ChildItems`。
- `ServiceLib/Common/Extension.cs:89`：`IsGroupType = PolicyGroup or ProxyChain`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：已选中单个 ProxyChain `ProfileDto`；纯函数 `resolveEditorKind(proxyChain)`→`SpecialEditorKind.group`（FIX-03 已提交，不改）。
- 输出：保存走既有 `saveProfile`/engine 路径（`SaveProfileResult`），未新增 IPC；生成为纯 `config_codegen`（`build_chain_outbounds_list`）。
- 错误：备注必填；子节点或订阅子项至少其一（`error.group_children_required`，与 Rust `validate_group` 一致）；悬空引用/环/非法正则由 Rust `validate_group` 拒绝（FIX-03 已有单测，本卡不改）。
- 取消：编辑器编辑本地 `ProfileDraft` 拷贝，取消直接 pop，`onSave` 零调用，数据库不动。
- 权限：仅本机 UI + FRB/Rust/SQLite；不启动内核、不写系统代理/TUN/注册表/路由、不监听端口、不按名杀进程。
- 持久化：`childItems` 保序、`subChildItems`、`filter`、`multipleLoad`、`groupType=ProxyChain`、未知扩展键经保存/重开一致。
- 生效：生成配置时链节点按上游反向 `ChildItems` 依次串联（入口 `proxy`），非负载均衡（无 observatory/balancer）。

允许修改的模块：`apps/desktop/lib/features/profiles/**`、`apps/desktop/test/**`、`apps/desktop/integration_test/**`、`crates/application/src/{groups.rs,codegen.rs}`、`crates/config_codegen/**`、`docs/evidence/UX-PARITY-FIX-03C/**`、本卡、compat 台账（仅追加）。本卡实际只新增测试与证据，未改任何生产 Dart、未改 FIX-03/03B 语义。

禁止改变的已有行为：FIX-03 已提交的 `resolveEditorKind` 分派、Custom 浏览（FIX-03B）、PolicyGroup 生成语义；`main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、其它 features、`crates/application/src/engine.rs`、`crates/updater/**`；不删入口或降分母。

测试夹具和原版预期：合成 vless 叶子（`192.0.2.11/12/13`，不连接）+ 合成 ProxyChain（`childItems="c1,c2,c3"`）；合成 ProxyChain DTO（`groupType=ProxyChain`、`childItems="n2,n1"`、`subChildItems="sub-1"`、`filter="^HK"`、`multipleLoad=4(LeastLoad)`）。原版预期：ProxyChain 与 PolicyGroup 同窗编辑，保存写回上述字段；生成时 `ChildItems` 反序、`dialerProxy`/`detour` 串联、入口 `proxy`、无 balancer。

本次必须通过的命令/真实场景（已实际运行）：
- `cargo test -p application --lib codegen --locked` → 6 passed；其中新增 `engine_input_generates_xray_dialer_proxy_chain`、`engine_input_generates_singbox_detour_chain` 通过。
- `cargo fmt -p application -- --check` → 无输出（干净）。
- `dart format --output=none --set-exit-if-changed test/fix03c_proxy_chain_editor_test.dart` → 0 changed。
- `flutter test test/fix03c_proxy_chain_editor_test.dart` → 首跑 4 项 `did not complete`（已知 flutter_tester 引擎抖动），重试 7/7 全过。
- `dart analyze test/fix03c_proxy_chain_editor_test.dart` → No issues found。
- `flutter analyze`（全仓）→ 3 issues，全部位于并行代理正在修改的文件（`lib/features/profiles/ui_state_store.dart` 两条、`test/support/fake_platform_bridge.dart` 一条），非本卡改动、未触碰。

证据文件位置：`docs/evidence/UX-PARITY-FIX-03C/`（`README.md`、`observations.json`、`rust-codegen-run.log`、`dart-chain-editor-run.log`）。

完成条件：ProxyChain 走 group 编辑器（右键/Ctrl+D/双击共用分派）；子节点多选插入/排序/T-U-D-B、五模式、SubChildItems+Filter 与上游字段一致；取消零落库；保存重开一致；生成配置链式语义与冻结 `V2rayOutboundService.BuildChainOutboundsList` 一致（合成链断言通过）；门禁通过。已达成本卡范围内全部条件。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。
- 接口缺口（登记，承接 FIX-03 §46）：订阅级「虚拟 ProxyChain」（`SubItem.PrevProfile/NextProfile` → `BuildSubscriptionChainNodeAsync` 在生成时按 Remarks 解析并合成头/尾节点）当前未实现；本卡的 ProxyChain 指用户手工创建/编辑的链节点，两者不是同一路径。建议立后续卡实现订阅 prev/next 的生成期虚拟链，需 RuntimePlan/codegen 组装层新增合成节点解析（不改本卡已提交语义）。
- 未做原版实机双窗口逐事件对照；链节点的真实窗口端到端（右键→编辑→重开→启动内核）未单独跑，链式生成语义以 Rust 合成链断言为准。

本轮实际结果：未改生产代码；新增 `apps/desktop/test/fix03c_proxy_chain_editor_test.dart`（7/7：重开保序/五模式/多选插入与 T-U-D-B/取消零落库/保存重开一致/picker 仅排除 Custom/新链默认 groupType=ProxyChain）；在 `crates/application/src/codegen.rs` 测试模块新增 2 项应用层合成链断言（Xray `dialerProxy`、sing-box `detour`，均覆盖反向顺序、tag 命名、入口 `proxy`、无 balancer）。上游对照结论：ProxyChain 的「链式」不是按存储顺序依次代理，而是 `ChildItems` 反序后以 `dialerProxy`/`detour` 串成一条拨号链，入口出站 tag 固定为 `proxy`；ProxyChain 与 PolicyGroup 共用 `AddGroupServerViewModel` 与 `group_editor_dialog`，字段语义一致。门禁：`cargo fmt` 干净、`cargo test -p application --lib codegen` 6/6、`dart format` 0 changed、`dart analyze` 本文件 No issues、widget 7/7（一次已知引擎抖动重试）。compat 仅追加一条 `F-FIX-03C` 条目，未删行、未降分母。
