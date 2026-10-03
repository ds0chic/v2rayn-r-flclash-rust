# FIX-09C — 订阅前置/后置节点链（PrevProfile/NextProfile 生成期虚拟 ProxyChain）

状态：`implemented`（Rust 生成期合成链 + 悬空告警回退由合成链单测断言；未做原版实机双窗口逐事件对照，UI 仍是文本输入框而非上游的排除 Custom 节点选择器，故不写 `verified`）。

任务 ID：FIX-09C

本次唯一用户流程：订阅设置前后置节点（PrevProfile/NextProfile，按节点 Remarks）→ 生成期按上游 `BuildSubscriptionChainNodeAsync` 合成虚拟 ProxyChain → 生成出的 Xray/sing-box 配置实际按链串联生效；存储中的 ProfileItem 不被改写。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；`docs/evidence/parity-review-2026-10-03/repair-queue.md` 第 34 行（FIX-09）；`docs/tasks/FIX-09.md` 拆卡登记（FIX-09C，`SET-05`、`F-SUB-008`、`ACT-SUB-007`）；FIX-03C 已交付链式生成语义（`crates/config_codegen` 的 `build_chain_outbounds_list`，`docs/evidence/UX-PARITY-FIX-03C/README.md`），其 §50 明确登记「订阅级虚拟 ProxyChain 未实现」由本卡承接。

对应 feature / field / action ID：`F-SUB-008`、`SET-05`、`ACT-SUB-007`、`REF-ENT-002`（PrevProfile/NextProfile → 节点 Remarks）、`CFG-014`/`EConfigType.ProxyChain`(102)。

必读上游文件、符号、固定 commit：
- `ServiceLib/Handler/Builder/CoreConfigContextBuilder.cs:220-315`：`ResolveNodeAsync` 在 `includeSubChain` 时调 `BuildSubscriptionChainNodeAsync`，命中则把虚拟链节点加入 `AllProxiesMap` 并递归解析（`includeSubChain=false`）；节点缺失仅告警，两者皆空不建链。
- `ServiceLib/Handler/Builder/CoreConfigContextBuilder.cs:258-315`：`BuildSubscriptionChainNodeAsync` — 守卫 `Subid.IsNullOrEmpty() || ConfigType == Custom`；`prevNode=GetProfileItemViaRemarks(PrevProfile)`、`nextNode=...NextProfile`；`ChildItems=[prev?.IndexId, node.IndexId, next?.IndexId]`（过滤空值）；虚拟节点 `IndexId=inner-{guid}`、`ConfigType=ProxyChain`、`CoreType=GetCoreType(node)`、`Remarks=node.Remarks`、`GroupType=ProxyChain`。
- `ServiceLib/Manager/AppManager.cs:287`：`GetProfileItemViaRemarks` = `FirstOrDefault(it => it.Remarks == remarks)`（精确匹配，无 subid 过滤）。
- `ServiceLib/ViewModels/SubEditViewModel.cs:27/35/101`：选择器 `SetConfigTypeFilter([Custom], exclude:true)`，返回 Remarks 写 `PrevProfile`/`NextProfile`。

## 输入、输出、错误、取消、权限、持久化及生效语义

- 输入：活动节点 `index_id` + `SubItem.PrevProfile`/`NextProfile`（Remarks 字符串）。
- 输出：`AppEngine::build_codegen_input` 返回的 `CodegenInput` 中，`input.profile` 被替换为合成 `ProxyChain`，其 `protoExtra.childItems` 为 `[prev,node,next]`（保序去重），并插入 `input.profiles`；`build_codegen_input_with_warnings` 额外返回诊断。
- 错误：`子项引用悬空` → `config_codegen::Diagnostic::warning`，code `error.subscription_prev_profile_not_found` / `error.subscription_next_profile_not_found`，`field_path=SubItem.PrevProfile` / `SubItem.NextProfile`；绝不因悬空而生成失败。两者皆空（含订阅缺失/节点无 Subid/`Custom`）→ 不建链，回退单节点。
- 取消：不涉及异步/取消；纯生成期变换。
- 权限：仅本机 Rust/SQLite 读；不启动内核、不写系统代理/TUN、不监听端口。
- 持久化：**不写回**。合成链只存在于 `CodegenInput`；存储 `ProfileItem` 保持原样（`config_type`、`child_items` 不变）。
- 生效：生成器对 `input.profile` 调 `build_all_proxy_outbounds(..., "proxy")`；`ProxyChain` 走 `build_chain_outbounds_list`（FIX-03C 已对齐）——`ChildItems` 反序、Xray `streamSettings.sockopt.dialerProxy` / sing-box `detour` 串联、入口 tag=`proxy`、非负载均衡。

## 允许修改的模块（本轮实际改动）

- `crates/application/src/subs.rs`：新增 `build_subscription_chain_node` / `resolve_remarks`（合成虚拟 ProxyChain + 悬空告警）。
- `crates/application/src/engine.rs`：`build_codegen_input` 改为 `build_codegen_input_with_warnings` 的薄封装；后者装配后合成链；`build_runtime_plan_with_hints` 改用带告警版本并把告警并入 `generated.diagnostics`。
- `docs/evidence/UX-PARITY-FIX-09C/**`、本卡、`compat/features.yaml`（仅追加 `F-FIX-09C`）。
- 未改 `crates/config_codegen`、`crates/domain`、`crates/bridge_api`、任何 Dart 生产代码；无 FRB 签名变化。

## 禁止改变的已有行为

不改 FIX-09/09B/09D 已提交语义（job id/取消、转换目标、定时更新）；不改 `main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`features/settings|profiles|runtime|monitor|update|backup|routing/**`、`crates/application/src/{dns.rs,routing.rs,speedtest.rs,monitor.rs}`、`crates/updater/**`；不删入口或降分母。

## 测试夹具和原版预期

- 合成 vless 叶子三条（`synthetic_full_profile` 改写备注为 `chain-prev`/`chain-active`/`chain-next`，地址 192.0.2.x，不连接）+ 合成 `SubItem`（`https://example.com/sub`，不下载）。
- 测试端口 11818/11819/11820（≥11808，不触碰 10808）。
- 原版预期：带 Subid 且非 Custom 的节点按 Remarks 解析 prev/next，`ChildItems=[prev,node,next]`；缺失仅告警回退；`Custom`/无 Subid 不建链；链生成反序串联，入口 `proxy`。

## 本次必须通过的命令/真实场景（实际运行见下）

- Rust：`cargo fmt -p application -- --check`、`cargo clippy -p application --all-targets --locked -- -D warnings`、`cargo test -p application --lib --locked`、`cargo test -p bridge_api --lib --locked`。
- Flutter：本卡未改 Dart，未运行（UI 交互为既有文本输入框）。

## 证据文件位置

`docs/evidence/UX-PARITY-FIX-09C/README.md`（命令、结果、上游对照、缺口）。

## 完成条件

- 带 Subid 且非 Custom 的节点在生成期按上游语义合成虚拟 ProxyChain 进入链 → 已实现。
- 合成结果与 FIX-03C 链式生成一致（反序 `dialerProxy`/`detour`、入口 `proxy`）→ 已断言（Xray + sing-box）。
- 悬空引用明确告警并回退、双空不建链 → 已断言。
- 存储 ProfileItem 不被改写 → 已断言。
- Rust 门禁通过；未做原版实机双窗口逐事件对照，保持 `implemented`。

## 拆卡登记（本卡不做）

- **上游节点选择器**：上游 `SubEditViewModel.SelectProfileAsync` 用 `SetConfigTypeFilter([Custom], exclude:true)` 的节点选择器返回 Remarks；本仓库 UI（`apps/desktop/lib/features/subs/sub_edit_window.dart`）目前是文本输入框，用户仍可手填 Remarks 完成同一数据流。选择器交互缺口，建议后续卡实现（不改本卡生成语义）。
- **子节点有效性过滤**：上游 `TraverseGroupNodeAsync` 会对 prev/next 走 `NodeValidator`，非法叶子被剔除并计入 `MsgGroupChildNode*` 告警；本卡只做 Remarks 悬空告警，不做逐节点 schema 校验。

## 本轮实际结果

- `cargo fmt -p application -- --check`：干净。
- `cargo clippy -p application --all-targets --locked -- -D warnings`：通过，无告警。
- `cargo test -p application --lib --locked`：192/192 通过（含新增 4 项 `subscription_chain_*`）。
- `cargo test -p bridge_api --lib --locked`：51/51 通过（未改 bridge）。
- 上游对照结论：合成时机/命名/串联/排除规则与冻结 `BuildSubscriptionChainNodeAsync` 一致；链式生成复用 FIX-03C 已对齐语义，未改其行为。
- 未运行：原版实机双窗口逐事件对照、真实窗口集成；UI 选择器未实现（文本输入框维持现状）。
