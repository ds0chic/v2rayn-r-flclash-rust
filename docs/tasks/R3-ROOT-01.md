# R3-ROOT-01 — F5 重载重入补跑最后一次，空活动给出提示

状态：`implemented`（controller 单测断言重入 pending 与空活动提示；未对真实运行中重复按 F5 做隔离效果测试）。

任务 ID：R3-ROOT-01

本次唯一用户流程：按 F5 / 菜单「重载」时，若已有一次重载在跑，快速再次触发应记录并在完成后补执行一次（不静默丢弃）；无活动节点时给出明确提示而不是静默返回。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；复核 `docs/evidence/parity-recheck-2026-10-04/round3-root.md` R3-01。菜单/F5 已接共享 `RuntimeController.reload()`。

上游对照：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/ViewModels/MainWindowViewModel.cs:661-752`（`_hasNextReloadJob` + `_reloadSemaphore`，完成后补跑；无默认节点 `NoticeManager.Enqueue(CheckServerSettings)`）。

对应 feature / field / action：`ACT-MAIN-035`、`RR-01`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：无（读取最新 desired 与活动节点）。
- 输出：应用最新活动计划；重入时在首个完成后补跑一次。
- 错误：apply 失败保持结构化错误，不被后续 snapshot 覆盖。
- 空活动：`uiShell.setMessage('配置项无效，请检查或重新选择')`（上游 CheckServerSettings）。
- 权限：`None`；不启动内核（测试用计数 bridge）。

允许修改的模块：`apps/desktop/lib/features/runtime/runtime_controller.dart`、`apps/desktop/test/**`、本卡、证据目录、compat 台账（仅追加）。未改 `main_shell.dart`。

禁止改变的已有行为：`applyActive` 的 revision 语义；`stop`/`resyncAfterRestore`；不伪造 Running。

测试夹具与原版预期：`CountingRuntimeBridge` 增加 `applyGate` 闸；断言首个 reload 在途时第二次 reload 仅置 pending、完成补跑后 `applyCalls==2`；空活动 `activeId=null` 时 `applyCalls==0` 且 shell message 为 `配置项无效，请检查或重新选择`。原版预期：语义同上。

本次必须通过的命令/真实场景：
- `flutter analyze`
- `flutter test test/recheck_r01_runtime_reload_test.dart`

证据文件位置：`docs/evidence/recheck-fixes/R3-MISC/README.md`。

完成条件：busy 时记录 pending 并补执行一次；空活动有提示；旧 busy no-op 断言被纠正；门禁通过。

接口缺口（登记）：`RuntimeView.isBusy` 是后端快照态，不能可靠代表 reload 在途；已用 controller 本地 `_reloadInFlight` 判定重入。

本轮实际结果：`runtime_controller.dart` 用 `_reloadInFlight`/`_reloadPending` 取代 `state.isBusy` 早退；空活动经 `uiShellControllerProvider` 提示；测试文件更新为原版语义并新增 2 用例。4/4 通过。
