# R4-01 启动重试与真实运行事实 — 证据

状态：implemented（控制器/契约层与原生构建已通过；真实“缺核→修复→同 GUI 重试”未实测）。

HEAD/baseline：`b0ce431`（应用基线 `77c74ed`，上游冻结 `7d6a967`）。armed=false（`V2RAYN_R_SMOKE_ARMED` 未定义）。无端口使用；全部用合成夹具，未碰 10808、未改系统代理/注册表/路由/TUN/自启，未读取用户凭据。

## 本次用户流程

第一次 apply 因历史错误（如缺核）失败 → 修复原因后，同一 GUI 的显式 apply / F5 必须再次提交；普通“设为默认”的同默认动作仍幂等。

## 合同与实现

- 历史 `snapshot.error` 不再否决新命令：`RuntimeController.applyActive`/`reload`/`restoreActiveOnLaunch` 移除刷新后的 `state.error` 早退；仅当快照读取本身抛错时回退本地 desired revision（`ExplicitTargetRuntimeBridge.desiredRevision`）。
- 在途命令立即 busy：`RuntimeView.commandPending`，apply 同步置位，`isBusy` 合并 UI pending 与后端状态机。
- 失败不谎报 Running：apply 失败保留结构化 error，且不再刷新覆盖旧会话，运行中的旧会话/端口/会话 id 保留。
- 同默认 no-op 诚实：`activateProfileDetailed` 同活动返回 `persisted=true, applied=false, noop=true`（不再声称一次成功运行）。
- 错误不转空 DTO：`get_snapshot` 失败经 `error_snapshot_dto`（`api/engine.rs:500-515`），既有测试 `storage_unavailable_engine_fails_closed`/`error_snapshot_dto` 覆盖。
- applied 事实可信：`reconcile_applied_session` 仅 `Running`+有端口才发布端点（既有测试 `failed_candidate_does_not_publish_endpoint` 等）。

## 断言复现（先失败后通过）

1. 复制审计 `startup-contract-repro.dart` 为 `apps/desktop/test/repair/r4_01_repro_test.dart`（仅改 import 为 `../support/counting_runtime_bridge.dart`）。
2. 修复前（暂时 stash 运行线三个文件的改动后运行）三条断言失败，与审计日志一致：
   - `explicit apply ...` Expected <2> Actual <1>
   - `F5 retries ...` Expected <2> Actual <1>
   - `a pending command reports busy ...` Expected true Actual false
3. 恢复改动后三条全部通过，另加 `test/r4_01_contract_test.dart` 4 条。

## 实际命令与结果

- `flutter analyze` → No issues found!
- `flutter test test/repair/r4_01_repro_test.dart test/r4_01_contract_test.dart test/r4_02_contract_test.dart` → All tests passed（18）。
- 回归：`test/runtime_controller_test.dart test/recheck_r01_runtime_reload_test.dart test/sr03_restore_lifecycle_test.dart test/fix11_monitor_session_test.dart test/r3_01_07_10_test.dart test/rr_mon_rebind_test.dart test/recheck02_active_apply_test.dart test/t18b_runtime_ui_test.dart` 全部通过（t18b 需单独运行，见其 harness 的 native 资源泄漏说明）。
- `cargo fmt --all -- --check` → 通过。
- `cargo clippy --workspace --all-targets --locked -- -D warnings` → 通过。
- `cargo test --workspace --locked -- --test-threads=1` → EXIT=0，105 个 test binary 全 ok；并行默认线程下 `subs::tests::scheduler_pass_*` 两个本地端点测试偶发 E_UNAVAILABLE（端口/资源竞争，非本次改动），单线程复跑通过。
- `flutter build windows --release` → Built build\windows\x64\runner\Release\v2rayn_desktop.exe。

## 未完成 / 未验证

- 真实独立数据目录缺核 → 安装核心 → 同一 GUI 重试到 Running 的真机场景未运行（无 armed 包，未启动受管内核）。
- F5 后 `reload→refresh→apply→refresh` 合并与并发去重属 R4-04；本卡只保证不因历史错误否决。
