# R4-17 订阅保护与稳定身份 — 证据

状态：implemented（未 verified）。代码与合成测试已通过；未做真实 FRB/窗口与真实订阅运行，D05 的“活动统计在运行中的实时转移”存在接口缺口（见下）。

## 环境与固定信息

- repo HEAD：ef02954a6de973c8ba8ed6917f9b84dbb9560d94（应用基线 77c74ed…未额外核对，按卡要求）。
- 冻结上游：work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967，commit 7d6a967。
- armed=false：未启动任何内核/未接触 10808/宿主代理/路由/TUN/自启/注册表。
- 测试夹具：全合成（`synthetic_full_profile`、合成 vless URL、内存 SQLite）；未读取用户订阅/凭据。
- 端口：本次测试未监听任何端口；既有 `t18b` 内核 e2e 使用 >=11808（见“无关失败”）。

## 改动摘要

- `crates/application/src/store_repo.rs`
  - `SqliteProfileRepository::replace_for_sub`：替换只删除 `Subid = subid AND IsSub = 1`（对齐 `RemoveServersViaSubid(..., isSub: true)`），同组手工 `IsSub = 0` 节点保留；`(added, removed)` 语义改为“替换集大小/被替换订阅行数”，不再是整组计数。
  - 同一事务内按 `find_matched_profile` 把匹配的 `ServerStatItem` 行克隆到新身份（对齐 `CloneServerStatItem`）。
  - 内存后端 `ProfileStore::replace_for_sub`：同样只删 `is_sub` 行，`added = profiles.len()`，失败回滚快照。
- `crates/application/src/subs.rs`
  - 新增 `profiles_match` / `find_matched_profile`（冻结 `CompareProfileItem` / `FindMatchedProfileItem` 的两级+备注/地址回退规则）、`mark_subscription_candidates`（刷新候选强制 `IsSub = true`）、`remap_active_after_replace`（对齐 `AddBatchServers` 的活动默认重定向）。
  - `refresh_one_with_convert`：候选标记为订阅来源；替换成功后按旧活动身份匹配并 `set_active` 到新节点。
- `crates/bridge_api/src/api/subs.rs`
  - 通用 `import_from_text`（节点页手工粘贴/扫码）写入 `is_sub = false`（对齐 `AddBatchServers(..., isSub: false)`），不再把手工节点统一成 `is_sub = true`；订阅内容仍走刷新管线（候选为 true）。
- `apps/desktop/lib/features/subs/subs_actions.dart`
  - `updateCurrentGroup`：命令开始捕获节点页当前组 `profilesControllerProvider.groupSubId`（非订阅设置 `subsState.selected`）；All（空）传空 `sub_ids` 走“更新全部合法订阅”。
- 新增测试：`apps/desktop/test/r4_17_contract_test.dart`、`apps/desktop/test/repair/r4_17_repro_test.dart`；Rust 侧新增 6 个测试（`subs::tests` 5 个 + `store_repo::tests` 1 个）。

## 上游对照结论

- 批导入 `IsSub=false`：`MainWindowViewModel.cs:484-502`、`ConfigHandler.cs:1672-1673`（`AddBatchServersCommon` 写 `profileItem.IsSub = isSub`）。
- 删除范围：`ConfigHandler.cs:2246-2259`，`isSub=true` 时 `delete ... where isSub = 1 and subid = ...`；删整组（`DeleteSubItem`）用 `isSub=false`。
- 替换后活动重映射：`ConfigHandler.cs:2109-2117` `FindMatchedProfileItem(lstSub, activeProfile)` -> `SetDefaultServerIndex`。
- 统计克隆：`ConfigHandler.cs:2120-2131` -> `StatisticsManager.CloneServerStatItem`（复制而非移动）。
- 匹配规则：`ConfigHandler.cs:1258-1304`（`CompareProfileItem`）、`:1317-1355`（`FindMatchedProfileItem` 三级回退）。
- 当前订阅范围：`MainWindowViewModel.cs:186-192` 传 `_config.SubIndexId`；`SubscriptionHandler` 仅当 subid 非空时限定，因此 All 等价全部合法订阅。

## 已运行命令与结果

- `flutter analyze`（apps/desktop）：`No issues found!`，EXIT=0。
- `flutter test test/r4_17_contract_test.dart test/repair/r4_17_repro_test.dart --no-pub`：`All tests passed!`（7/7），EXIT=0。
  - 复现测试先失败后通过证据见 `commands.log`（修复前 `Expected ['s-A'] Actual ['syn-sub-1']`；All 组 `Expected empty Actual ['syn-sub-1']`）。
- `cargo fmt --all -- --check`：EXIT=0。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：EXIT=0。
- `cargo test --workspace --locked`：仅 `application` 的 `t18b_runtime_e2e::rr07_custom_config_plan_serves_on_its_real_port` 失败（真实 xray 内核启动即退出，与 R4-17 无关；该测试使用 `save`/`build_runtime_plan`，不经过本卡改动）。`--skip rr07_custom_config_plan_serves_on_its_real_port` 重跑全 workspace：EXIT=0，无失败。
- 定向：`cargo test -p application --lib`：239 passed, 0 failed。

## 未完成 / 接口缺口

- D05 运行中实时统计转移：本卡在持久化事务内克隆 `ServerStatItem`（重开后不丢），但 `StatsService` 的内存态 `nodes`（`monitor.rs`）与活动索引 `set_active_index` 由 engine/monitor 持有；未修改 engine.rs/monitor.rs（另一代理在改 / 本卡禁改），因此“运行中无需重开即正确关联”尚未闭环。建议由 engine/monitor 写入者提供提交结果驱动的内存态转移接口。
- 未做真实 FRB/窗口、真实订阅下载（合成端口仅 Rust 测试用）、重开 SubIndexId 场景与真实 SQLite 重开截图；这些属 R4-17 的 UI→FRB→Rust→持久化→重开链路，目前标记为未验证。
- `updateCurrentGroup` 的“读取实际 SubItem 校验存在/禁用”仅通过 id 传递，未对不存在的组做额外拒绝（后端按空/禁用跳过）。原版对 All 边界保持不变。
