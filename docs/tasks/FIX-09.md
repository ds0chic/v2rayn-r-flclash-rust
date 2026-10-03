# FIX-09 — 订阅更新：先返回真实 job ID，取消绑定该 ID

状态：`implemented`（Rust bridge 早返回 job ID + 后台 worker、Dart 控制器按 job ID 轮询/取消、header 不区分大小写重复项检测均实现并有针对性测试通过；本轮未跑真实窗口集成与真实网络下载，故不写 `verified`）。

任务 ID：FIX-09

本次唯一用户流程：「开始订阅下载→取消」。开始更新后 UI 立即持有真实 job ID，进度与取消都绑定该 ID；取消后不替换旧组、不报假成功、不留半成品。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；审查结论见 `docs/evidence/parity-review-2026-10-03/` 的 `repair-queue.md` 第 34 行（FIX-09）、`settings-report.md` SET-03/04/05、`runtime-report.md` RT-16、`all-items.csv` 的 F-SUB-003/004/010/011 与 ACT-MAIN-020..023、ACT-SUB-006。

对应 feature / action ID：`F-SUB-003`、`F-SUB-004`、`F-SUB-011`、`ACT-MAIN-020/021/022/023`、`SET-03`；header 校验对应 `F-SUB-004`。

必读上游文件、符号、固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的
- `v2rayN/ServiceLib/Handler/SubscriptionHandler.cs:5`（`UpdateProcess`）、`:83`（`CreateDownloadHandler`）、`:102`（`DownloadSubscriptionContent` 代理失败回退直连）、`:115/129/158/197`（DownloadAll/主/附加/结果处理，转换与 MoreUrl 顺序）；
- `v2rayN/ServiceLib/Helper/HttpRequestHeadersHelper.cs:5`（`TryParse`：`Dictionary(StringComparer.OrdinalIgnoreCase)` + `TryAdd`，重复键返回 false）；
- `v2rayN/ServiceLib/ViewModels/MainWindowViewModel.cs:574`（`UpdateSubscriptionProcess` 经 `Task.Run` 调 `SubscriptionHandler.UpdateProcess`）。

## 输入、输出、错误、取消、权限、持久化及生效语义

- 输入：`update_subscriptions(sub_ids, via_proxy)`；`sub_ids` 空=全部启用订阅；`via_proxy` 使用已记录的本会话本地代理端点。
- 输出：bridge 在**任何网络工作之前**返回 `SubUpdateResult{ ok=true, success=0, entries=[], job_id=Some(真实id), error=None }`（`ok` 只表示“已受理”）；后台 worker 完成后 `finish_sub_job` 落终态并把完整 report 以 `subscriptions_updated` 控制事件推出（payload 附加 `job_id`）。
- 错误：`via_proxy` 但无本地端点时仍返回结构化 `E_PROXY_UNAVAILABLE` 且 `job_id=None`（不启动工作、不回退直连）；后台 pipeline 失败按 `PreservedError`/`Failed` 上报，作业终态 `Failed`。
- 取消：`cancel_job(job_id)` → `JobManager::cancel` 翻转该作业的 `CancellationToken`；pipeline candidate-first，取消时返回 `Cancelled` 条目并 `break`，旧节点集保持不变，作业终态 `Cancelled`。
- 权限：仅本机 UI + FRB/Rust/SQLite；不启动内核、不写系统代理/TUN、不监听端口。
- 持久化：仅成功 `Updated` 分支替换该 subid 节点并 touch `UpdateTime`；取消/失败不落盘替换。
- 生效：Dart `update()` 在 await 受理结果后立即写 `lastJobId`，随后轮询 `job_view(job_id)` 直到终态；tile/report 依据作业终态生成，成功计数用更新前后节点数派生，绝不伪造。

## 允许修改的模块（本轮实际改动）

- `crates/application/src/subs.rs`：`parse_request_headers` 增加不区分大小写的重复 header 检测（对照上游 `HttpRequestHeadersHelper.TryParse`）。
- `crates/bridge_api/src/api/subs.rs`：`update_subscriptions` 改为“先返回真实 job ID，后台 worker 执行”，新增 `spawn_sub_update`；`update_subscriptions` 的 doc 明确 `ok` 语义。
- `apps/desktop/lib/features/subs/subs_controller.dart`：`update()` 立即绑定 `lastJobId`，新增 `_awaitJob`/`_isStarted`/`_isTerminal`/`_countsBySub`/`_targets`，`_summarize` 区分失败；`cancel()` 语义不变（按 id 调 `cancelJob`）。
- `apps/desktop/test/fix09_subs_job_cancel_test.dart`（新增）。
- `docs/tasks/FIX-09.md`、`docs/evidence/UX-PARITY-FIX-09/**`。

无 FRB 签名变化，未改生成文件。

## 禁止改变的已有行为

不改 `main_shell.dart`、`frb_generated.*`、`frb_generated.rs`、`features/runtime|profiles|monitor|update/**`、`crates/updater/**`、`engine.rs`；不删入口、不降分母、不伪造下载/落库结果。

## 测试夹具和原版预期

- Rust：合成 `SubItem`；header 夹具用 `{"X-Test":"1","x-test":"2"}`（原版判定非法）与合法 `{"X-A":"1","X-B":"2"}`。
- Flutter：`StartedJobBridge`（继承 `SeededSubsBridge`）模拟真实异步合同——`updateSubscriptions` 先返回 job id 且 entries 为空，`jobView` 推进状态，`cancelJob` 按 id 记录；合成订阅仅 `https://example.com/sub`，不下载、不连接。
- 原版预期：取消在下载期间即可作用于当前任务；取消/失败不替换旧组；成功才刷新并报成功。

## 本次必须通过的命令/真实场景（实际运行见下）

- Rust：`cargo fmt -p <crate> -- --check`、`cargo clippy -p <crate> --all-targets --locked -- -D warnings`、`cargo test -p <crate> --locked`。
- Flutter：`dart format`、`flutter analyze`、逐文件 `flutter test`。
- 真实窗口集成：`docs` 记录为待根代理统一跑（本轮未跑）。

## 证据文件位置

`docs/evidence/UX-PARITY-FIX-09/README.md`（命令、结果、上游对照、缺口）。

## 完成条件

- bridge 在异步工作前返回真实 job ID；进度/取消绑定该 ID；取消不替换旧组、不报假成功、不留半成品 → 已实现（Dart 合同测试 + Rust candidate-first 既有语义）。
- header 校验含不区分大小写重复项 → 已实现并有单测。
- 针对性测试通过；未做真实窗口/真实网络，保持 `implemented`。

## 拆卡登记（本卡不做）

- **FIX-09B — 订阅转换目标**（`SET-05`、`F-SUB-007`，上游 `SubscriptionHandler.cs:129-155`）：按 `ConstItem.SubConvertUrl`/`Global.SubConvertUrls.First()` 用 `string.Format` 组装转换 URL，补 `&target=` 与 `&config=Global.SubConvertConfig.First()`；候选源（ComboBox）与 `ConvertTarget` 生效。当前仅保存并用于决定是否跳过 MoreUrl。
- **FIX-09C — 前置/后置节点链**（`SET-05`、`F-SUB-008`、`ACT-SUB-007`，上游 `SubEditViewModel.cs:27/87/101`、`CoreConfigContextBuilder.cs:258-287`）：排除 `Custom` 的节点选择器（返回 Remarks 写入 `PrevProfile`/`NextProfile`），运行期按 Remarks 解析生成 `ProxyChain`/`PreSocksPort` 出站，节点缺失时告警回退。
- **FIX-09D — 定时更新 scheduler（普通启动 + 到期下载）**（`SET-04`、`RT-16`、`F-SUB-010`、`ACT-SUB-006`，上游 `TaskManager.cs:87-120`）：
  - 缺口①：普通启动无 `startSubScheduler()` 调用。最小补丁建议：在应用 bootstrap（`lib/app/app.dart` 的 `_RuntimeBootstrap`，引擎初始化成功后、非 smoke 分支）调用 `ref.read(bridgePortProvider).startSubScheduler()`，退出时 `stopSubScheduler()`；不在 `main_shell.dart` 内做。
  - 缺口②：`run_scheduler_tick` 强制 `via_proxy=true`，未起内核时 `local_proxy_url()=None` → 全部 `E_PROXY_UNAVAILABLE`。需按原版后台策略决定 blProxy（原版 `TaskManager` 走 `UpdateProcess(..., boolProxy)`），并为受控测试注入 fake clock/本地合成端点（测试端口 ≥11808）。
  - 需新增 fake-clock 覆盖“到期条件 + 合成订阅下载闭环 + 任务随启动/退出”，本轮不实现。

## 本轮实际结果

- Rust `cargo test -p application --lib subs`：11/11 通过（含新增 `request_headers_reject_case_insensitive_duplicates`）。
- Rust `cargo test -p bridge_api --lib api::subs`：8/8 通过（含既有 `update_via_proxy_without_endpoint_returns_proxy_unavailable`）。
- Rust `cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings`：通过，无告警。
- 两处改动文件 `rustfmt --check`：exit 0。
- Flutter `flutter test test/fix09_subs_job_cancel_test.dart test/t09_sub_setting_test.dart`：7/7 通过（新增 2 + 既有 5）。
- Flutter `flutter analyze lib/features/subs/subs_controller.dart test/fix09_subs_job_cancel_test.dart`：No issues；`dart format` 已写回。
- 未运行：真实窗口集成测试、真实网络下载（`flutter test integration_test/... -d windows`），登记为待根代理统一跑。
