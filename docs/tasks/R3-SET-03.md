# R3-SET-03 — 恢复前排空在途订阅任务并拒绝旧提交（P1）

状态：`implemented`（Rust scheduler 取消/超时、手动任务 drain 超时、restore epoch 拒绝均有单测；慢响应合成 mock 端口 ≥11808 已验证；未做真实窗口并行恢复）。

任务 ID：R3-SET-03

本次唯一用户流程：订阅下载响应较慢时执行恢复；旧下载返回后不得把节点写回换库后的存储；等待/排空超时必须阻断恢复并报错，不得静默继续。

前置任务及已验证证据：冻结版；开始 HEAD `6699c31`；复核见 `docs/evidence/parity-recheck-2026-10-04/round3-settings.md` R3-SET-03。

上游对照：`U/ServiceLib/ViewModels/BackupAndRestoreViewModel.cs:133/134/150`（AppExitAsync/DisposeDbConnectionAsync/Shutdown）。

对应 feature / field / action：F-SUB-003/011、F-BACKUP-001..004；`SubScheduler`、`replace_sub_profiles`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`AppEngine::prepare_restore`、`SubScheduler::stop`、手动 `update_subscription` 任务。
- 输出：`prepare_restore` 先 `bump_restore_epoch`，再 `cancel_and_drain_sub_tasks`、`stop_sub_scheduler_blocking`，超时返回结构化错误。
- 错误：`error.restore_sub_task_timeout` / `error.restore_scheduler_timeout`；旧任务提交命中 `error.restore_in_progress`（`codes::CONFLICT`）。
- 取消：scheduler tick 使用共享 `CancellationToken`，`stop` 立即取消在途下载；手动 job 走 `JobManager::cancel`。

允许修改的模块：同 R3-SET-01（重点 `crates/application/src/{engine.rs,subs.rs}`）。

禁止改变的已有行为：不改 `frb_generated`；不杀不受管进程；测试端口 ≥11808 且不使用 10808。

测试夹具和原版预期：合成慢响应 HTTP mock（bind 11808..11949）；原版在恢复前退出并销毁 DB 连接。

本次必须通过的命令/真实场景：
- `cargo test -p application --lib --locked subs::tests::scheduler_stop_cancels_slow_download_before_commit`
- `cargo test -p application --lib --locked engine::tests::replace_sub_profiles_rejects_stale_restore_epoch`
- `cargo test -p application --lib --locked engine::tests::cancel_and_drain_sub_tasks_times_out_on_stuck_job`
- `cargo test -p application --lib --locked engine::tests::prepare_restore_bumps_epoch_before_exchange`

证据文件位置：`docs/evidence/recheck-fixes/R3-SET-01-04/`。

完成条件：慢响应下 stop 立即结束 tick 且不提交；epoch 变化后旧提交被拒绝；drain/scheduler 超时返回错误而非静默。门禁通过。

接口缺口（登记）：真实窗口下“慢下载 + 恢复”端到端并发未在本机实测；`error.restore_*` 文案尚未加入 UI 本地化表（现有错误通道已透传 code/messageKey）。
