# UX-PARITY-FIX-09 — 订阅下载取消绑定真实 Job ID

状态：`implemented`（未跑真实窗口/真实网络，故非 `verified`）。
关联：`docs/tasks/FIX-09.md`、`repair-queue.md` FIX-09、`settings-report.md` SET-03/05、`runtime-report.md` RT-16。
上游冻结：`2dust-v2rayN` `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

## 结论

1. **先返回真实 job ID（SET-03）**：`crates/bridge_api/src/api/subs.rs::update_subscriptions` 现在先 `register_sub_job()` 拿到 `JobId`，设置 `subs.downloading` 进度阶段后把刷新交给 `spawn_sub_update`（专用线程 + current-thread tokio runtime）执行，并立即返回 `{ ok:true, success:0, entries:[], job_id:Some(id) }`。`ok` 仅表示“已受理”，完成由作业终态与 `subscriptions_updated` 事件给出。
2. **进度/取消绑定该 ID**：Dart `SubsController.update()` 在 await 受理结果后立即写 `lastJobId`，再轮询 `job_view(job_id)` 到终态；`cancel()` 用 `lastJobId` 调 `cancel_job`。取消在下载期间即可命中当前作业，不再拿上次已结束 ID。
3. **取消不替换旧组/不报假成功/不留半成品**：沿用既有 candidate-first pipeline——`refresh_one` 只在下载解析成功后才 `replace_sub_profiles`；取消时 `Cancelled` 条目 + `break`，旧节点保留，作业终态 `Cancelled`。Dart 仅在作业 `done` 时报成功，成功计数由更新前后节点数派生，绝不伪造。
4. **header 不区分大小写重复项（对照上游 `HttpRequestHeadersHelper.TryParse`）**：`parse_request_headers` 用 `HashSet` 记录小写名，碰撞即报 `E_FIELD_FORMAT`（`request_headers`）。(JSON 精确重复键在 serde_json 层已合并，故仅保证大小写碰撞。)

## 上游对照

- `SubscriptionHandler.UpdateProcess` 经 `Task.Run` 异步化，作业生命周期随 UI；本实现的 job id/进度/取消即该生命周期的桥接等价物。
- `HttpRequestHeadersHelper.TryParse`：`Dictionary(StringComparer.OrdinalIgnoreCase)` + `TryAdd` 拒绝重复键；本实现等价。
- `DownloadSubscriptionContent` 代理失败回退直连、`DownloadAllSubscriptions` 的 ConvertTarget 跳过 MoreUrl 顺序：既有实现保留，转换服务本身属 FIX-09B。
- `DownloadMainSubscription` 转换 URL 组装、`SubEditViewModel` 前后置 picker：登记为 FIX-09B/09C，本卡不做。
- `TaskManager` 定时更新：登记为 FIX-09D，本卡不做。

## 实际命令与结果

Rust（workdir=repo 根）：

| 命令 | 结果 |
|---|---|
| `cargo test -p application --lib subs --locked` | 11 passed / 0 failed（含新增大小写重复测试） |
| `cargo test -p bridge_api --lib api::subs --locked` | 8 passed / 0 failed |
| `cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings` | 通过，无告警 |
| `rustfmt --edition 2021 --check crates/application/src/subs.rs crates/bridge_api/src/api/subs.rs` | exit 0 |

- 说明：`cargo fmt -p application -- --check` 会在**未改动**的 `crates/application/src/engine.rs`、`crates/application/src/lib.rs` 报出既有/并行改动差异；与 FIX-09 无关，未触碰。已改动文件单独 `rustfmt --check` 为 0。

Flutter（workdir=`apps/desktop`）：

| 命令 | 结果 |
|---|---|
| `flutter test test/fix09_subs_job_cancel_test.dart test/t09_sub_setting_test.dart` | 7 passed / 0 failed |
| `flutter analyze lib/features/subs/subs_controller.dart test/fix09_subs_job_cancel_test.dart` | No issues found |
| `dart format lib/features/subs/subs_controller.dart test/fix09_subs_job_cancel_test.dart` | 已格式化 |

- `flutter test` 未出现 exit 79 引擎崩溃，无需重试。

## 未完成 / 待根代理统一跑

- 真实窗口集成（`flutter test integration_test/... -d windows`）与真实网络下载未运行。
- 真实内核下 `job_view` 的进度阶段显示未实测（当前只设 `subs.downloading`，不伪造百分比）。

## 接口缺口 / 拆卡

- FIX-09B 订阅转换目标（SET-05）。
- FIX-09C 前置/后置节点链（SET-05）。
- FIX-09D 定时更新普通启动 + 到期下载（SET-04/RT-16）：普通启动缺 `startSubScheduler()` 调用；tick 强制 via_proxy 在无内核时必失败。最小补丁建议见任务卡。

## 改动文件

- `crates/application/src/subs.rs`
- `crates/bridge_api/src/api/subs.rs`
- `apps/desktop/lib/features/subs/subs_controller.dart`
- `apps/desktop/test/fix09_subs_job_cancel_test.dart`（新增）
- `docs/tasks/FIX-09.md`（新增）
- `docs/evidence/UX-PARITY-FIX-09/README.md`（新增）
