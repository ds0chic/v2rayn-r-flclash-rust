# FIX-09D — 定时更新 scheduler（普通启动 + 到期下载）

状态：`implemented`（Rust scheduler 单测覆盖到期/下载/失败/停止清理，Flutter 覆盖启动幂等与停止清理；`app.dart` 接线由根代理落地，未跑真实窗口/真实网络，故不写 `verified`）。

任务 ID：FIX-09D

本次唯一用户流程：普通启动→订阅自动更新到期→下载完成；端点不可用→明确报错。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；`docs/evidence/parity-review-2026-10-03/repair-queue.md` 第 34 行（FIX-09/09D 行）、`settings-report.md` SET-04、`runtime-report.md` RT-16、`all-items.csv` 的 `F-SUB-010`、`ACT-SUB-006`；`docs/tasks/FIX-09.md` 的拆卡登记（本卡为其 09D 拆分）。

对应 feature / action ID：`F-SUB-010`、`SET-04`、`RT-16`、`ACT-SUB-006`。

必读上游文件、符号、固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的
- `v2rayN/ServiceLib/Manager/TaskManager.cs:10`（`RegUpdateTask`）、`:18`（`PeriodicTimer(1min)`）、`:87`（`UpdateTaskRunSubscription`：过滤 `AutoUpdateInterval > 0` 且 `updateTime - t.UpdateTime >= AutoUpdateInterval*60`）、`:104`（`UpdateProcess(_config, item.Id, blProxy: true, ...)`）、`:112`（`item.UpdateTime = updateTime`）、`:113`（`AddSubItem`）；
- `v2rayN/ServiceLib/Handler/SubscriptionHandler.cs:5`（`UpdateProcess`）、`:102`（`DownloadSubscriptionContent` 代理失败回退直连，见 FIX-09）。

## 输入、输出、错误、取消、权限、持久化及生效语义

- 输入：无 UI 输入。普通启动调用 scheduler 启动函数；每个 tick 读取全部 `SubItem`，按 `is_due(item, now)`（enabled 且 `AutoUpdateInterval>0` 且 `now - UpdateTime >= interval*60`）选出到期项。
- 输出：对每个到期项执行既有 `refresh_subscriptions` 候选优先管线；成功替换该 subid 节点并 touch `UpdateTime`。`run_scheduler_pass` 返回合并后的结构化 `SubUpdateReport`。
- 错误：端点不可用/解析失败/下载失败按 `PreservedError`/`Failed` 记录（旧节点保留），并在 tick 内以 stderr 记录，不再静默吞掉、不假成功。FIX-09 的显式用户路径（`update_subscriptions(via_proxy=true)` 无端点）仍返回 `E_PROXY_UNAVAILABLE`，语义不变。
- 代理：tick 不再强制 `via_proxy=true`。`scheduler_proxy_choice(local_proxy_url())`：有已记录本地端点→经代理（失败由既有 `download_with_fallback` 回退直连）；无端点→直连。无内核时不会因 `E_PROXY_UNAVAILABLE` 使整批失败。
- 取消/退出：`stopScheduler()`（Dart）→ `stop_sub_scheduler()` → `SubScheduler::stop` 置停止标志并 `notify_one` 唤醒循环；循环在下一轮顶部退出，运行时随线程结束释放，无残留定时器。
- 权限：仅本机 UI + FRB/Rust/SQLite；不启动内核、不写系统代理/TUN、不监听入站端口（测试的本地 HTTP 端点为回环合成，端口 ≥11808）。
- 持久化：仅在成功分支 touch `UpdateTime` 并替换节点；失败/取消不落盘替换。
- 生效：正常启动即启动 scheduler（非测试 env 武装）；退出即停止。

## 允许修改的模块（本轮实际改动）

- `crates/application/src/subs.rs`：`SubScheduler` 改为独立 OS 线程 + current-thread tokio runtime（FRB 同步入口无环境下也能启动），新增 `is_finished()`、`stop` 改用 `notify_one` 防丢唤醒；抽出 `run_scheduler_pass(engine, max_items, now)`（可控时钟 + 结构化报告）与纯函数 `scheduler_proxy_choice`；`run_scheduler_tick` 失败记录 stderr；新增 5 个单测。
- `apps/desktop/lib/features/subs/subs_controller.dart`：`startScheduler()` 幂等（已运行则 no-op）、`stopScheduler()` 幂等且无残留。
- `apps/desktop/test/fix09d_scheduler_ctrl_test.dart`（新增）。
- `docs/tasks/FIX-09D.md`、`docs/evidence/UX-PARITY-FIX-09D/**`。

`app.dart` 由根代理接线（见证据 README「app.dart 最小补丁」），本卡未改 `app.dart`。

## 禁止改变的已有行为

不改 `main_shell.dart`、`app.dart`、`frb_generated.*`、`features/profiles|runtime|monitor|update|settings|routing/**`、`crates/updater/**`、`crates/application/src/engine.rs`；不改 FIX-09 已提交的 job id/取消语义；不删入口、不降分母、不伪造下载/落库结果。

## 测试夹具和原版预期

- Rust：本地合成回环 HTTP 端点（端口 11808..11950，先 bind 探测；body 为合成 `vless://`）+ 合成 `SubItem`（`auto_update_interval=10`、`update_time=0`，`https`/`http` 合成 URL，不连接真实网络）。失败用例取探测后可用的空闲端口再释放，令连接被拒。
- Flutter：`SchedulerBridge`（继承 `SeededSubsBridge`）维护 running 标志并计数 start/stop，断言幂等与清理。
- 原版预期：`AutoUpdateInterval > 0` 且到期的项在后台更新并写 `UpdateTime`；失败仅记录日志不替换旧节点。

## 本次必须通过的命令/真实场景（实际运行见证据 README）

- Rust：`cargo test -p application --lib subs --locked`、`cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings`、`rustfmt --edition 2021 --check crates/application/src/subs.rs`。
- Flutter：`dart format`、`flutter analyze`、`flutter test`（针对性文件）。
- 真实窗口集成：待根代理统一跑（本轮未跑）。

## 证据文件位置

`docs/evidence/UX-PARITY-FIX-09D/README.md`。

## 完成条件

- 普通启动实际启动 scheduler（由 `startScheduler` 幂等函数 + 根代理 `app.dart` 补丁保证）→ 已实现（Flutter 幂等测试）。
- tick 不再强制 `via_proxy` → 已实现并有单测（`scheduler_proxy_choice`）。
- 到期下载能完成（本地合成端点 + 可控时钟）→ 已实现并有单测。
- 失败/端点不可用明确报错、不静默/不假成功 → 已实现并有单测。
- 启动/停止清理无残留定时器 → 已实现并有 Rust/Flutter 单测。
- 未跑真实窗口/真实网络，保持 `implemented`。

## 接口缺口 / 拆卡登记

- **接口缺口**：后台 scheduler 的失败目前只进 `SubUpdateReport` + stderr，未通过 `subscriptions_updated` 控制事件推给 UI。要把它作为可见通知上报，需要一个由 `bridge_api` 持有回调、注入 `SubScheduler` 或 engine 的通道；`SubScheduler::start` 由 `crates/application/src/engine.rs` 固定签名调用，而本卡不允许改 `engine.rs`，故登记待根代理决策（可加 FRB 事件源）。
- 未新增 FRB 函数；`startSubScheduler`/`stopSubScheduler`/`subSchedulerRunning` 沿用既有签名，未动生成文件。

## 本轮实际结果

见 `docs/evidence/UX-PARITY-FIX-09D/README.md`。
