# SP-11 原生子窗丢回包 证据

状态：implemented（Dart 正确合同绿；真实原生窗口/HWND 未验，不写 verified）。
基线：`df8582b`。SP-00/SP-02 已合入。不 commit，根整合者验收提交。
夹具：合成快照/草稿 + 注入式丢包/迟到/重复回复；无真实用户数据。
副作用：无 socket/端口、无宿主代理/路由/TUN/DNS/Run-key 写入；未占用 10808。

## 0. 缺陷复现（红）

审计 CP-09（`docs/evidence/complete-port-audit-2026-10-06/checks/audit-ui-native_reply_loss-confirmed.log`）：
`saveDraft` 传输 ACK 成功但 `saveOutcome` 丢失，31s 后仍永久等待（2 项失败）；
根因在 Dart 侧：`NativeSettingsEditorHost.save` /
`NativeRoutingEditorHost.save|commit` 直接 `await completer.future`
（无超时），主侧 `_handle` 中 `save` 抛异常则 `reportOutcome` 永不发出。

本卡红测试（`apps/desktop/test/repair/sp_11_window_reply_test.dart` 初版，仅用旧 API）：
3 项按预期失败——设置丢回包永久等待、路由丢回包永久等待、close 遗弃
pending；主侧异常项在旧代码下经 `handlePlatformMessage` 被 binding
吞掉而空过，修复后改由 `debugDispatchApplyDraft` 精确断言
（见 §2 R3b；另有一次中间态 lib 文件被外部恢复为基线，疑似并行
worker 操作，已重新应用并用 grep/format/analyze 校验落盘）。

## 1. 改动文件

- `apps/desktop/lib/features/settings/settings_window_host.dart`
  - 新增 `WindowOutcomeStatus`（ok/failed/pendingConfirmation，wire 名与
    `crates/ipc_contract/src/stable.rs` 的 snake_case 一致）、
    `WindowPendingQuery`（reconciler seam，只观察不重发）、
    `WindowReplyGate`（(generation, requestId) 跟踪、有界等待、
    settle 过滤迟到/重复/旧代际、`abandonAll` 结算并旋转代际）。
  - `SettingsEditorOutcome` 新增 `pendingConfirmation`（默认 false，兼容旧调用方）。
  - `NativeSettingsEditorHost.save`：`saveDraft` 只当传输 ACK，请求携带
    `{id, draft, requestId, windowGeneration, mutationId}`；超时完成
    PendingConfirmation 并走 reconciler seam（不重发）；`close()` 结算
    pending 并旋转代际；`_handle(saveOutcome)` 按代际/requestId 过滤，
    无 envelope 的旧回复按数字 id 兼容接受（无 `status` 时信任 `ok`）。
  - `OptionWindowHost._handle`：`save` 异常转为结构化失败 Outcome 并恒发
    `reportOutcome`（回显 requestId/generation/status，发送本身 try/catch）；
    新增 `debugDispatchApplyDraft` 可测 seam。
- `apps/desktop/lib/features/routing/routing_windows.dart`
  - 同上：`RoutingEditorOutcome.pendingConfirmation`、
    `NativeRoutingEditorHost`（save/commit 共用 `_send`，单次发送）、
    `RoutingWindowHost._handle` 异常→Outcome（含 schemes 回显）、
    `_commitAction`/`_ok`/`_deleteSchemes` 把 Pending 显示为可恢复状态
    （释放 busy、窗不关、不重放）。
- `apps/desktop/test/repair/sp_11_window_reply_test.dart`（新增，12 项，见 §2）。
- `apps/desktop/windows/runner`：未改动。C++ 两处 host 只是不透明转发
  参数 map（`saveDraft` 立即 `Success()` = 传输 ACK，业务结果走
  `saveOutcome`），转发已天然分离 ACK≠Outcome；新增 envelope 字段经
  StandardMethodCodec 双向兼容。关闭后 `reportOutcome` 丢弃符合 close
  隔离语义；main 缺失时 ACK 后无 Outcome 由 Dart 有界等待覆盖。

## 2. 正确合同（12 项，`flutter test test/repair/sp_11_window_reply_test.dart`）

| # | 预期 | 结果 |
|---|---|---|
| R1 | 设置丢回包→有界 settling（默认 10s）且为 PendingConfirmation | pass |
| R2 | 路由丢 commit 回包→PendingConfirmation，不永久 busy | pass |
| R3 | close 结算 pending，不遗弃 | pass |
| R3b | 主侧 `save` 抛异常→`reportOutcome{ok:false,status:failed}` + envelope 回显 | pass |
| R4 | 正常 ok 回包→真实结果，envelope 随请求发出 | pass |
| R5 | 重复回包忽略（第二次 failed 不翻转已完成的 ok） | pass |
| R6 | 超时后迟到回包丢弃，且 `saveDraft` 计数恒为 1（不重放） | pass |
| R7 | close 旋转 generation；旧代际迟到回包丢弃；新代际请求正常 | pass |
| R8 | 无 envelope 的旧式 `{id, ok}` 回包仍兼容完成 | pass |
| R9 | 超时触发 reconciler 查询（mutationId 非空），仍不重发 | pass |
| R10 | 路由 ok 回包完成；重复忽略 | pass |
| R11 | 路由 close 旋转代际；pending 清零 | pass |

线协议（增量兼容，C++ 不透明转发）：
`saveDraft{id, draft, requestId, windowGeneration, mutationId}`；
`saveOutcome/reportOutcome{id, ok, requestId?, windowGeneration?, status, message?, schemes?}`。
对照 `stable.rs`：`WindowRequestEnvelope{windowId, windowGeneration, requestId, mutationId?, datasetEpoch, domain, expectedRevision?, payload}` /
`WindowSaveOutcome{... status: WindowOutcomeStatus ...}` —— Dart 侧字段名
一致；`datasetEpoch` 在第二引擎侧未知，未伪造（见 §4 阻塞）。

## 3. 真实命令与 exit（`apps/desktop`，另注 cargo）

- `dart format --output=none --set-exit-if-changed <3 个改动文件>`：exit 0。
- `flutter analyze`：exit 0（本卡引入的 10 个 info/warning 已清零，
  含 `use_null_aware_elements`/`prefer_initializing_formals`/
  `invalid_use_of_visible_for_testing_member`/`avoid_returning_null_for_void`）。
- `flutter test test/repair/sp_11_window_reply_test.dart`：12/12 pass，exit 0
 （日志 `C:\Users\Colby\AppData\Local\Temp\opencode\sp11c.log`，测试外留存）。
- 受影响既有测试：
  - `test/r4_12_contract_test.dart` 单跑：8/8 pass，exit 0。
  - `r3_wpf_option_window` + `r3_wpf_routing_window` +
    `r3_wpf_routing_structure` + `t21e_dialogs_responsive` +
    `repair/r4_11_repro`：16/16 pass，exit 0。
  - 10 文件批量（含 fix08*/t12a/r4_14/t11/r4_12）：27 pass；
    其中 6 个 r4_12 用例报 did-not-complete（批量加载器 flake；
    r4_12 单跑全绿；审计对批量跑亦有“加载异常/多项未完成”记录，
    非本卡回归）。
- `cargo test -p bridge_api --locked`（仓库根）：77 passed / 0 failed，exit 0
  （Rust 零改动，确认无回归）。

## 4. 未完成 / 阻塞（含整合者共享接口）

1. 真实原生窗口验收未做：真实 HWND 缺 main/关窗/迟到/双击/长时间运行、
   `flutter build windows --release` 均未跑 → 状态不得写 verified。
2. Reconciler 接线（根整合者/SP-12）：`WindowPendingQuery{windowId,
   windowGeneration, requestId, mutationId}` seam 已留，
   `mutationId = win-settings|routing-<requestId>` 已生成并随包；
   但 `querySettingsMutation(datasetEpoch, mutationId)` 的 FRB 暴露、
   `datasetEpoch` 在第二引擎的来源、menu/option 侧 `onReconciled` 的 UI
   刷新归属，需 SP-00 整合者确认（FRB/共享 DTO 独占，不自行改动）。
3. Runner native 侧边界（未改，待 owner 评审）：`reportOutcome` 在设置窗
   已销毁时丢弃（符合隔离语义）；`saveDraft` 在 main 未 Attach 时 ACK
   后无 Outcome（Dart 超时覆盖）。如需 native 合成 Outcome/代际自增，
   需 Windows 构建验证后另卡实施。
4. 路由畸形快照静默产出空草稿属 CP-08/SP-13 范畴，本卡未动
   `decodeRoutingSnapshot`（路由 `loadSnapshot` 仍不抛，见 R4-12 既有行为）。
5. 性能（10k/100k）、正式包、跨平台、24h/500 切换均未验。

## 5. 改动文件清单（本卡）

```
M apps/desktop/lib/features/settings/settings_window_host.dart
M apps/desktop/lib/features/routing/routing_windows.dart
?? apps/desktop/test/repair/sp_11_window_reply_test.dart
M docs/repair/stable-port-2026-10-06/tasks/SP-11.md（仅状态行）
M docs/repair/stable-port-2026-10-06/execution-manifest.json（仅 SP-11 块 status）
+ docs/evidence/stable-port/SP-11/README.md（本文件）
```

工作树另有他人（SP-03 及 Rust 侧）改动，本卡未触碰：
`crates/application/src/{backup_service,engine,lib,recoverable_commit}.rs`、
`tasks/SP-03.md`、manifest SP-03 块、`selection.rs`、
`tests/sp03_backup_identity.rs`、`docs/evidence/stable-port/SP-03/`。
