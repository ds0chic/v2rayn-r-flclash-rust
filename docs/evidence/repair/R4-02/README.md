# R4-02 显式启动目标与默认合同 — 证据

状态：implemented（控制器/契约层、Rust set_active 与原生构建已通过；真实 GUI 点击“启动选中 B、F5 重载默认 A”未实测）。

HEAD/baseline：`b0ce431`（应用基线 `77c74ed`，上游冻结 `7d6a967`）。armed=false。无端口使用；合成夹具；未碰 10808、未改系统代理/注册表/路由/TUN/自启，未读取用户凭据。

## 本次用户流程

选中 B 后点顶部“启动”只启动 B；F5/“重启服务”仍按默认节点；普通单击只选择不启用。

## 合同与实现

- 顶部按钮在点击时冻结显式目标（`resolveSingleTarget(state, null)` → primaryId/单选中项），调用 `profile_actions.startProfileExplicit`；无目标走默认/原版恢复，无法恢复明确提示。
- `ProfilesController.prepareStartTarget`：显式目标 > primaryId > 单选 > 持久化默认 > `SetDefaultServer` 首个 Port>0 回退；目标变化先持久化（Rust 侧 bump desired），再按该目标 apply。持久化失败不更新内存 active。
- `RuntimeController.applyActive({String? targetId})`：显式目标经 `ExplicitTargetRuntimeBridge.applyTarget` 提交；默认路径仍 `applyActive`（空目标）。
- Rust `AppEngine::set_active`：不同 ID 才 bump desired（D27）；同 ID 幂等不 bump；persist 失败回滚 active 与 revision，绝不留下内存伪 active。
- 未改 R4-08 的即时选择/键盘对象合同（pointer-down 冻结与立即 Enter 目标），未改 sameactive 默认动作幂等。

## 断言复现

`test/r4_02_contract_test.dart`（11 条）覆盖：无 active、A active+B selected、多选 primary=B、显式冻结优先、无选择回退默认、无 active 恢复候选、无节点不虚构、persist 失败不留伪 active、F5 用默认、顶部显式 apply 目标、同默认 noop。全部通过。（这些断言是新增合同，不针对旧实现改预期。）

## 实际命令与结果

- `flutter analyze` → No issues found!
- `flutter test test/repair/r4_01_repro_test.dart test/r4_01_contract_test.dart test/r4_02_contract_test.dart` → All tests passed（18）。
- 回归（同 R4-01 列表，另 `recheck02_active_apply_test.dart`）全部通过。
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --locked -- --test-threads=1`（EXIT=0）通过。
- Rust 新增：`active_set_bumps_desired_only_when_the_id_changes`、`active_set_persist_failure_does_not_leave_a_fake_active` 通过。
- `flutter build windows --release` → Built v2rayn_desktop.exe。

## 未完成 / 未验证

- 真实 GUI：A 运行中 → 选中 B → 顶部“启动”实际启动 B、F5 仍默认 A 的端到端未运行（无 armed 包，未启动受管内核）。
- desired/applied 的 `未应用` 提示贯穿重开见 R4-04/R4-27；本卡只保证 set_active 目标变换 bump 且 persist 失败不伪 active。
