# SP-12 已保存未生效重试 — 证据

状态：implemented（定向检查绿；真实原生/正式包/OS 隔离验收未跑，不写 verified）。
基线：`92d46dd`。不 commit。合成数据；fake 桥仅故障注入；Dart 侧无 socket；
绝不占/改 10808；不改宿主系统代理/路由/TUN/DNS/Run-key（读 OS 自启事实仅用于
desired≠applied 对账）；`work/`、`outputs/` 只读未动；`compat/` 未动。

## 0. 缺陷复现（红，正确预期未改弱）

- `docs/evidence/complete-port-audit-2026-10-06/settings/settings_retry_contract_test.dart`
  修前 0 pass / 3 fail（`settings_retry_contract.log` 一致）：
  1. 自启写失败后重开不再重试（`load()` 把持久 desire 当 OS 已确认）；
  2. 独立窗首次保存成功/应用失败后，用旧 revision 重试被 stale 挡住到不了 apply；
  3. 更新选项保存失败仍显示成功且无回滚/错误。
- 本卡修后：1、2 绿；3 仍红（根因在 `features/update/update_controller.dart`
  先乐观改 state、无失败回滚/错误位，属 SP-27 文件锁范围，已登记阻塞，见 §5）。

## 1. 改动文件

- `apps/desktop/lib/features/settings/settings_controller.dart`（本卡独占）：
  新增 `SettingsPhaseReceipt`（save/core/platform/autostart + savedRevision/
  contentHash/errors）并挂到 `SettingsViewState.lastReceipt`（分阶段事实可见）；
  `SettingsApplyOutcome` 新增 newRevision/contentHash/各相 ok/phaseErrors；
  `load()` 不再用持久 `AutoRun` 覆盖 `_autostartApplied`，改为读 OS 实际
  `getAutostart`（查不到=null，强制下次重试；重载持久 desire 永不确认失败的
  OS 写）；`saveDocument` 成功记录 revision+`contentHashOf`（键排序规范化）；
  `saveAndApply` 对 `E_REVISION_STALE` + 内容与已持久一致时视为已保存、只走
  apply 阶段（独立窗旧 revision 重试直达 apply，不重提旧版本）；
  新增 `retrySettingsApply(draft, savedRevision, savedContentHash, phases)`：
  零持久写，仅执行所选未成功阶段，draft 与当前持久 hash 必须等于已保存 hash
  否则 `E_RETRY_STALE` 且不执行任何阶段；全路径经 `_applySavedVersion`
  记录 receipt（core 失败早返，platform/autostart 失败保留 saved=true）。
- `apps/desktop/lib/features/settings/settings_actions.dart`：
  独立窗 `_applyOptionDraft` 注释 SP-12 幂等语义；新增 `retryOptionApply`
 （已保存 draft 的“再次确认”只重试未成功阶段入口）。
- `crates/application/src/settings.rs`：新增纯函数 `settings_content_hash`
 （FNV-1a/32 hex）+ `retry_content_matches_saved` + 2 单测。
- `crates/bridge_api/src/api/settings.rs`：新增非 FRB 纯 helper
  `settings_content_hash_for_retry` / `retry_content_matches_saved`
 （零新增 FRB 面、零生成物变更）+ 1 单测。
- `crates/platform/src/sysproxy/mod.rs`：新增 `applied_content_hash`
 （mode/server/bypass/PAC/autodetect+applied 会话键；同端口改 bypass 必变 hash，
  失败不推进 applied）+ trait 默认方法 + 1 单测。
- 新增 `apps/desktop/test/repair/sp_12_retry_test.dart`（6 项，见 §3）。
- 未改：engine.rs、lib.rs 导出、IPC/stable DTO、BridgePort、FRB 生成物、Cargo 锁。

## 2. 命令与 exit

- `flutter test .../settings_retry_contract_test.dart`：修前 0/3 fail；修后 2 pass / 1 fail（第 3 项属 SP-27，见 §5）。
- `flutter test test/repair/sp_12_retry_test.dart`：6/6 pass（exit 0）。
- 既有 `r4_13_s05` + `r4_13_s07` + `audit_tun_settings_contract` + `r4_13_s21_repro`：15/15 pass（exit 0）。
- `flutter analyze`（apps/desktop）：No issues found（exit 0）。
- `dart format --set-exit-if-changed`（3 个 Dart 文件）：exit 0。
- `rustfmt --check`（3 个 Rust 文件）：exit 0；`cargo fmt --all --check` 仍 fail，
  全部剩余 diff 在他卡脏文件（net_host_client/events/server/session/helper_client），本卡文件零 diff。
- `cargo test -p application --locked settings`：lib 22 pass / 0 fail。
- `cargo test -p bridge_api --locked`：80 pass / 0 fail。
- `cargo test -p platform --locked sysproxy`：6 pass / 0 fail。
- `cargo clippy -p application -p bridge_api -p platform --all-targets --locked -- -D warnings`：
  唯一 error 在 `application/src/store_repo.rs`（SP-21 脏文件，collapsible_if），
  本卡文件零告警（已登记，见 §5）。

## 3. 新增测试（6/6 pass，不得改弱）

`test/repair/sp_12_retry_test.dart`（合成数据，CountingRuntimeBridge/CountingAutostart/SyntheticBridgePort）：

1. 保存返回新版本、core 失败保留已保存事实、receipt 分阶段可见、desired≠applied、重开可见。
2. retry 仅跑失败 core 相：applyCalls +1、revision 不变（无重存）、autostart attempts 不变。
3. 独立窗旧 revision 重试直达 apply（幂等已保存路径）。
4. 内容分叉的 stale retry 跑零阶段（`E_RETRY_STALE`，applyCalls 不变）。
5. 自启失败重开必重试（OS 实查，不确认失败写）。
6. 组保存失败状态可见（`_RejectGroupBridge` 注入 `E_STORAGE_UNAVAILABLE`）。

## 4. 语义保证

- 保存返回新版本；core/platform 失败不抹已保存事实（saved=true 贯穿）。
- `retrySettingsApply` 零写持久，只跑 `phases` 子集，且内容 hash 指向已保存版本并校验仍为当前可应用版本。
- desired（持久 document）≠ applied（runtime/平台/OS 事实）；取消不撤已提交事实（本卡无取消写路径）。

## 5. 接口需求 / 阻塞（登记，不私改他卡文件）

1. 第三合同 `update option failure must be visible or rolled back`：
   提供方 `features/update/update_controller.dart::_persistCheckUpdate`
   （SP-27 所有）；调用方 update 窗；需签名：先持久、成功再改 state（或失败回滚
   + `status.kind='error'`）；错误 `saveGroup` 原样透出；幂等：重复 toggle 同值
   不重复写；版本：沿用现有 group revision。SP-12 侧 settings 状态/错误已可见，
   跨控制器回滚留给 SP-27。
2. 共享 `SettingsSaveReceipt/retrySettingsApply` 稳定 DTO/FRB（如未来要 Rust 落点）：
   提供方 application/persistence/platform；调用方 bridge_api/主窗/独立窗；
   候选签名 `saveSettings(datasetEpoch, expectedRevision, mutationId, patch)` /
   `querySettingsMutation(datasetEpoch, mutationId)` /
   `retrySettingsApply(datasetEpoch, mutationId, savedDocumentToken, phases)`；
   错误含 CommitUnknown/RecoveryRequired + `E_RETRY_STALE`；幂等键 mutationId；
   版本化合同由 SP-00 整合者冻结。本卡 Dart 先行实现等价语义，未新增 FRB 面。
3. `store_repo.rs` clippy collapsible_if 与全量 `cargo fmt` 剩余 diff 属他卡脏文件，
   由对应卡/整合者处理；本卡文件 fmt/clippy 干净。
