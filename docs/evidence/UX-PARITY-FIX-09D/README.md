# UX-PARITY-FIX-09D — 定时更新：普通启动 + 到期下载（可控时钟）

状态：`implemented`（Rust scheduler 单测 + Flutter hook 单测通过；`app.dart` 接线由根代理落地；未跑真实窗口/真实网络，故非 `verified`）。
关联：`docs/tasks/FIX-09D.md`、`docs/tasks/FIX-09.md`、`repair-queue.md` 第 34 行、`settings-report.md` SET-04、`runtime-report.md` RT-16、`F-SUB-010`/`ACT-SUB-006`。
上游冻结：`2dust-v2rayN` `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

## 结论

1. **普通启动实际启动 scheduler（非测试 env 武装）**：`SubsController.startScheduler()` 改为幂等（已运行则 no-op），由 `app.dart` 在引擎初始化+设置加载后调用；退出时 `stopScheduler()`。启动函数不依赖任何 `V2RAYN_R_*` 环境变量。本轮不改 `app.dart`（禁改），最小补丁见下。
2. **tick 不再强制 via_proxy**：`run_scheduler_pass` 用 `scheduler_proxy_choice(engine.local_proxy_url())`：有已记录端点→经代理（失败由既有 `download_with_fallback` 回退直连）；无端点→直连 `via_proxy=false`。无内核时不再全批 `E_PROXY_UNAVAILABLE`。FIX-09 的显式用户路径语义不变。
3. **到期下载能完成（本地合成 HTTP 端点 + 可控时钟）**：`run_scheduler_pass(engine, max_items, now)` 接受注入时钟；单测 `scheduler_pass_downloads_due_subscription_from_local_endpoint` 用回环合成端点（≥11808，先 bind 探测）下载 `vless://`，断言 `success_count==1`、节点写入、`UpdateTime` 被 touch。
4. **失败/端点不可用明确报错**：`scheduler_pass_reports_unavailable_endpoint_not_fake_success` 令端口释放后连接被拒，断言 `success_count==0`、条目为 `PreservedError`/`Failed`、无节点写入（不静默、不假成功）；`run_scheduler_tick` 对失败项写 stderr（对齐上游 `Logging.SaveLog`）。
5. **停止无残留定时器**：`SubScheduler` 改为独立 OS 线程 + current-thread tokio runtime（FRB 同步入口无 ambient runtime），`stop` 用 `notify_one` 防丢唤醒；`scheduler_stop_exits_loop_without_residual_timer` 断言 `is_finished()`；`stopScheduler()` Flutter 侧幂等。
6. **不改 FIX-09 job id/取消语义**：未触碰 `update_subscriptions`/`cancel_job`/`spawn_sub_update`/`finish_sub_job`。

## 上游对照

- `TaskManager.cs:10 RegUpdateTask` → 普通启动注册任务；本实现 `app.dart` 启动调用 + `SubScheduler` 常驻，等价。
- `TaskManager.cs:18-24 PeriodicTimer(1min)` → 固定周期检查；本实现 `SCHEDULER_TICK=60s` 循环 + `Notify` 唤醒退出，等价。
- `TaskManager.cs:87-93` 到期过滤（`AutoUpdateInterval>0` 且差值≥`interval*60`）→ `is_due`，逐字段一致。
- `TaskManager.cs:104 UpdateProcess(..., blProxy: true)` + `SubscriptionHandler.cs:102` 代理失败回退直连 → **差异**：上游字面强制 `true` 但在下载层回退直连；本实现的 bridge 显式路径把「via_proxy 无端点」判为 `E_PROXY_UNAVAILABLE`（FIX-09 用户语义，须保留），故 scheduler 在无端点时改走直连，避免后台 tick 整批失败。有效结果一致（有代理用代理、否则直连），实现位置不同，已登记。
- `TaskManager.cs:112 item.UpdateTime = updateTime` → 成功分支 `touch_sub_update_time`（沿用 FIX-09 管线）。
- 上游失败 `Logging.SaveLog` → 本实现 stderr 记录 `SubUpdateOutcome::Failed/PreservedError`（UI 事件通道见缺口）。

## app.dart 最小补丁（由根代理落地，本卡未改）

`apps/desktop/lib/app/app.dart`：

```dart
import 'package:v2rayn_desktop/features/subs/subs_controller.dart'; // 新增

// _RuntimeBootstrapState.windowShutdown()
void windowShutdown() {
  _integration?.removeListener();
  ref.read(subsControllerProvider.notifier).stopScheduler(); // 新增：退出停止，无残留
}

// _RuntimeBootstrapState.initState() 的 post-frame 回调内，
// 在 await controller.restoreActiveOnLaunch(); 之后新增：
// FIX-09D: 普通启动启动定时订阅更新（非测试 env 武装，幂等）。
ref.read(subsControllerProvider.notifier).startScheduler();
```

说明：`startScheduler`/`stopScheduler` 均幂等；放在 `restoreActiveOnLaunch()` 之后，使 `local_proxy_url()` 能反映已恢复会话。

## 实际命令与结果

Rust（workdir=repo 根）：

| 命令 | 结果 |
|---|---|
| `cargo test -p application --lib subs --locked` | `16 passed; 0 failed`（原 11 + 新增 5：proxy_choice/skip-not-due/due-download/unavailable-endpoint/stop-exit） |
| `cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings` | `Finished`，无告警 |
| `rustfmt --edition 2021 --check crates/application/src/subs.rs` | exit 0（先 `rustfmt` 写回 3 处） |

Flutter（workdir=`apps/desktop`）：

| 命令 | 结果 |
|---|---|
| `dart format lib/features/subs/subs_controller.dart test/fix09d_scheduler_ctrl_test.dart` | `0 changed` |
| `flutter analyze lib/features/subs/subs_controller.dart test/fix09d_scheduler_ctrl_test.dart` | `No issues found!` |
| `flutter test test/fix09d_scheduler_ctrl_test.dart` | `All tests passed!`（1/1） |
| `flutter test test/fix09_subs_job_cancel_test.dart test/t09_sub_setting_test.dart` | `All tests passed!`（7/7，回归） |

- 未出现 `flutter test` exit 79，无需重试。
- 未运行：真实窗口集成（`flutter test integration_test/... -d windows`）、真实网络下载，登记为待根代理统一跑。

## 未完成 / 接口缺口 / 下一步前置

- **缺口（接口）**：后台 scheduler 的失败目前只进 `SubUpdateReport` + stderr，未作为 `subscriptions_updated` 控制事件推给 UI。要可见上报需 `bridge_api` 持有回调并注入 `SubScheduler`/engine；`SubScheduler::start` 被 `engine.rs` 固定签名调用，而本卡禁改 `engine.rs`。建议根代理决策：新增一个由 `bridge_api` 管理的 scheduler 包装（或给 engine 加 report sink），本卡不造。
- **待根代理**：落地上述 `app.dart` 补丁；真实窗口集成验证「启动即常驻、退出即停止」与到期下载。
- **未验证**：真实内核运行时的 proxy-prefer 分支（`local_proxy_url()` 返回真实 mixed 端口时经代理下载）未实测；测试仅覆盖无端点直连与纯函数决策。
- 下一步前置：`crates/application/src/engine.rs` 若需开放 scheduler report sink 或暴露启动钩子，需根代理授权修改（本卡禁改）。

## 改动文件

- `crates/application/src/subs.rs`
- `apps/desktop/lib/features/subs/subs_controller.dart`
- `apps/desktop/test/fix09d_scheduler_ctrl_test.dart`（新增）
- `docs/tasks/FIX-09D.md`（新增）
- `docs/evidence/UX-PARITY-FIX-09D/README.md`（新增）
