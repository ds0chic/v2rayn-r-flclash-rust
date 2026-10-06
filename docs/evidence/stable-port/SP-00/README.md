# SP-00 证据：冻结合同与文件所有权

- 基线：`a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`（开始前 `git status` 干净）。
- 状态：implemented（编译可用合同 + 合同测试；行为接线与真实验收未做）。
- 原版：v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`（本轮未改内核行为）。

## 本次唯一流程
执行模型领取一个用户流程前，固定共享接口、库存和文件写锁。

## 交付
1. `crates/ipc_contract/src/stable.rs`（新）：§3 合同族，版本 `STABLE_CONTRACT_VERSION=1`。
   - 身份与版本分离：`DatasetEpoch`、`RuntimeActualDescriptor`（actualGeneration/appliedRuntimeRevision/planHash）、
     `RuntimeIntent`/`OperationReceipt`/`OperationState`。
   - 设置保存分阶段事实：`SettingsSaveReceipt.save/coreApply/platformApply` + `PhaseError`；
     `CommitUnknown`/`RecoveryRequired` 为独立状态，不与成功混淆。
   - 窗口合同：`WindowRequestEnvelope`/`WindowSaveOutcome`（含 `PendingConfirmation`）。
   - 查询/导入：`PageQueryMeta`/`PageResultMeta`（datasetRevision + requestGeneration）、
     `CommitImportRequest`（previewToken + mutationId + datasetEpoch）。
2. `crates/bridge_api/src/api/stable.rs`（新）：`stable_contract_version()`；FRB 重生成暴露
   `stableContractVersion()`（`lib/bridge/api/stable.dart`）。
3. 测试：
   - Rust 单测 5 个（serde 往返 + 版本）。
   - 协议握手安全失败沿用既有实现与测试：`ipc_contract::version_mismatch_is_rejected`、
     `helper::helper_version_mismatch_is_rejected`、`privileged_helper/tests/dispatch.rs::version_mismatch_response`。
   - 修正 helper loopback 旧合同（TUN-A01 空闲保活）：
     `idle_connection_survives_request_timeout`、`idle_connection_times_out_at_the_idle_bound`。
   - Dart：`apps/desktop/test/repair/sp_00_contract_test.dart`（3/3）。

## 实际命令与结果（本次运行）
| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | exit 0 |
| `cargo test --workspace --locked` | 全绿（含 loopback 8/8） |
| `cargo test -p ipc_contract --locked` | 34 passed |
| `flutter analyze` | No issues found |
| `flutter test test/repair/sp_00_contract_test.dart` | All tests passed (3/3) |

## 未验证 / 下一前置
- 各合同族的真实 provider/caller 接线：SP-04（运行命令序列）、SP-05（actual 目标）、
  SP-06（核心退出观测）、SP-08/09（清理与 TUN 租约）、SP-11/12（设置保存分阶段）、
  SP-14（导入提交）、SP-21（后台分页）。
- Dart 侧完整合同类型暴露随接线卡生成；本卡只固定 Rust 合同与版本。
- 真实验收（正式 UI + 真实 FRB/OS 效果）未运行，按 START_IMPLEMENTATION 保留后续卡。
