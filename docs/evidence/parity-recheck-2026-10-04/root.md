# 2026-10-04 主窗口与发布物复查

对照：冻结 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。审查开始时应用为 `efd6b2f` 且节点表有四个用户未提交文件；审查期间这些文件被另一个执行者提交为 `5471e4d`，发布物刷新提交为 `a2b6905`。本报告只读核对当前源码与包；未更改生产代码、未运行宿主代理/TUN/系统代理动作。

后续补充：根代理新增并运行了一个隔离的 Windows 集成测试，实际确认“选组新增节点落无组”和“切组保留隐藏选择”两项，见 [windows-ui-01](windows-ui-01/README.md)。下文“未启动发布版 exe”仍成立：测试构建的是 Debug 窗口。

## R-01 — 主界面 F5 重载仍不可用（P1，源码确认）

原版 [MainWindow.xaml.cs](../../../work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/v2rayN/Views/MainWindow.xaml.cs#L233) 的 F5 调用 `ViewModel.Reload()`；`MainWindowViewModel.cs:233-236` 的菜单命令也调用同一用例。`Reload()` 在 `MainWindowViewModel.cs:664-738` 有重入排队、活动节点检查、重新生成配置、重启核心、更新系统代理及测速等效果。当前 [main_menu.dart](../../../apps/desktop/lib/app/menu/main_menu.dart#L156) 把 `ACT-MAIN-035` 永久标为 `preservedOnly`；[main_shell.dart](../../../apps/desktop/lib/app/shell/main_shell.dart#L163) 的 F5 只显示“保留原版入口（未实现）”。已有 [RuntimeController.applyActive](../../../apps/desktop/lib/features/runtime/runtime_controller.dart#L107)，但未接到此入口，也未复刻原版重入语义。用户在编辑活动节点、改路由或想手动重载时失去共同的生效动作。`compat/actions.yaml` 已把该项记为 `preserved_only`，这是明确未完成项，不是新引入回归。

## R-02 — 现行节点表选择测试不稳定（验证阻断，原因未定）

当前 [profiles_selection_test.dart](../../../apps/desktop/test/profiles_selection_test.dart) 在同一 `flutter test` 进程跑三项时均报告 `did not complete [E]`，命令退出 1；逐项启动新进程时“single, ctrl, shift and select-all selection”和“press-and-drag selects an inclusive row range”通过；“drag upward selects the reversed range”单独连续两次仍 `did not complete [E]`，没有 Dart 异常栈。仓库 [T01 证据](../T01.md#L129) 已记录本机 Flutter 3.47.5 的 `flutter_tester` 原生崩溃，故此处不能归因于产品手势代码，也不能把向上拖选记为通过。应在真实 Windows 窗口用合成节点复现并收集 runner 退出码/崩溃转储，再区分测试引擎与产品缺陷。

## 检查与边界

- `flutter analyze`：0 issue；`cargo fmt --all -- --check`：通过；`cargo clippy --workspace --all-targets --locked -- -D warnings`：通过。这些只证明静态门禁。
- `dist/build-info.json` 在收尾时记录构建源码 `5471e4d`、`git_dirty=false`；当前 `HEAD=a2b6905` 是发布物刷新提交。两者的差别是提交后的发布元数据，不把 HEAD 与构建 commit 不同误报为代码缺失。
- 收尾时重新计算当前 zip 的 SHA-256 为 `894d1569eeb397909162490b405ea59b148128f995fc091fbf75c1829a322fe6`，与 `dist/SHA256SUMS` 一致。包中 exe 清单仅 `v2rayn_desktop.exe`、`net_host.exe`、`privileged_helper.exe`；升级 runner 缺失的影响见 `runtime.md`。未启动发布版 exe，因为默认数据/内核恢复可能触发 10808 或宿主系统代理；真实安装后的流程未验证。
- 没有运行 Flutter 全量测试、原版与重构版双窗口逐事件对照、真实 TUN、远端更新或宿主代理/自启写入。其他领域的具体发现见本目录的 `profiles.md`、`settings.md`、`runtime.md`。
