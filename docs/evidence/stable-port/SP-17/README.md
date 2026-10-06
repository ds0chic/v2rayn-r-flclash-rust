# SP-17 真实运行摘要和消息 — 证据

基线 `788adf6`（工作树另有并行卡未提交改动，本卡仅动写锁内文件，不 commit）。
卡状态：identified（实现完成，verified 需真实核心/FRB actual DTO 落地后由整合者复核）。
manifest `execution-manifest.json` 中 SP-17 仍为 identified：如实，未改。

## 改动（仅写锁内）

- `apps/desktop/lib/features/runtime/runtime_bridge.dart`
  - `RuntimeErrorView` 新增 `retryable`/`operationId`（透传 `ErrorDto` 已有字段，无 DTO/FRB 改动）。
  - 新增 `RuntimeNotice{severity,atMs,operationId}` + `RuntimeView.actualSummaryLabel/hasCurrentFailure/canRetryFailed`
    与 `attemptedTargetId/failedTargetId/failureOperationId/failedAtMs/notice`（UI 侧意图/失败身份；actual 事实仍只来自 snapshot）。
- `apps/desktop/lib/features/runtime/runtime_controller.dart`
  - apply 记录 `attemptedTargetId`；失败（确定/unknown）记录失败目标+操作+时间并保留实际视图；成功/stop 成功清除失败上下文；
    snapshot 替换保留失败/notice 身份；新增 `retryFailed()`、`reportAvailabilityNotice()`。
- `apps/desktop/lib/app/shell/status_bar_view.dart`
  - 中区读 `actualSummaryLabel`（state/session/ports/applied），附双击=原版可用性测试（ACT-STAT-004）+ 测试按钮；
    `probeLoopbackEndpoint` 对已应用端点做真实 127.0.0.1 TCP 回环反馈，拒绝探测保留端口 10808；
    `resolveStatusMessages` 保证当前失败为 headline、可重试/查看，旧平台消息降级进详情弹窗。
- 新增 `apps/desktop/test/repair/sp_17_{actual_summary,message_ordering,availability_probe,status_bar}_test.dart`（合成数据，端口 ≥11808 或临时端口）。

## 先红

新测试在实现前编译失败（`failedTargetId`/`resolveStatusMessages`/`probeLoopbackEndpoint` 缺符号），见 `red.log`。

## 定向检查（exit 均为 0，另见 `checks.log`）

- `dart format`（7 个自有文件）：0 改动余量（已格式化）。
- `flutter analyze`：No issues found。
- `flutter test` sp_17 四文件：12/12 pass。
- 既有受影响：`runtime_controller_test`、`sp_04_runtime_sequence_test`、`recheck_r01_runtime_reload_test`、
  `t13_statusbar_test`（2）、`t15a_statusbar_test`（2）：全部 pass（widget 文件逐个单独跑；多 widget 文件同进程批跑触已知 flutter_tester
  资源 flake，单跑全绿，属 CP-16 基础设施问题）。
- `t18b_runtime_ui_test`：第 2/3 项挂起（did not complete）——干净 worktree 基线 `788adf6` 复现完全相同挂起，既有失败，非本卡引入。

## 未运行范围

- Rust 未改：`cargo test/clippy/fmt` 未运行；`flutter build windows --release` 未运行（无 native/AOT 接线变更）。
- 审计 `ui/status_viewport_test.dart`（docs 内证据脚本，非 flutter test 目标）未跑；真实核心切换/重开/loopback 正式包验收未做。
- 停止/改备注/重开：stop 语义有单测覆盖；改备注不触 runtime（profiles_controller 在锁外未动）；独立重开未验。

## 阻塞（需 SP-00 整合者）

`SnapshotDto`/FRB 无 actual 描述子：`target_profile_id`、`actual_generation`、`last_exit/last_error`、
`targetCore/coreVersion/planHash/readyEndpoints` 仅存在于 `ipc_contract::stable` 与 engine 内存，未经
`bridge_api SnapshotDto` 暴露。底栏 actual 摘要当前只能用 state/session/ports/operationId/appliedRevision
表达“运行什么”，不能点名实际节点身份与代次——失败保留/重试/排序逻辑已按 operation/target 身份写好，
待 DTO 暴露后直接接线，无需改 UI 结构。
