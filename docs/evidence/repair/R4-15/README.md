# R4-15 DNS 草稿保护与 Reload — 证据

- 任务卡：`docs/repair/tasks/R4-15.md`
- 仓库 HEAD：`841accd`（工作树执行前干净）
- 应用审查基线：`77c74ed`
- 冻结原版：v2rayN 7.25.4 / `7d6a967`
- 状态：implemented（合成 widget 契约通过；真实 Windows 窗口 / 内核 / 平台效果未实测）
- armed=false；未启动内核、未监听端口、未改宿主系统代理/注册表/路由/TUN/自启，未读写用户凭据。

## 修复的两条缺陷

- **D17 保存后未 Reload**：原版 `DNSSettingViewModel.SaveSettingAsync`（`UP/.../DNSSettingViewModel.cs:195-196`）保存成功后 `RequestClose`，主窗 `MainWindowViewModel.DNSSettingAsync`（`:613-620`）收到成功即 `await Reload()`。当前实现默认 `applyAfter=false` 只保存不重载。修复后普通“保存”成功即触发 `runtimeController.reload()`；失败不重载、不改持久化。
- **D18 草稿基线按文本作键**：原实现 `_textBaseline[controller.text] = value` 以共享文本内容为键，`syncText` 用当前文本反查，导致“把 direct 改成 remote 的原值”被误判为未编辑，预设刷新丢草稿（审计 UFS-05 / plan D18）。修复后基线以稳定字段身份（controller 身份）为键，只比较该字段自身的原值；布尔字段同步后同时推进基线。

## 改动文件

- `apps/desktop/lib/features/routing/dns_window.dart`
- `apps/desktop/lib/features/routing/dns_controller.dart`（保存成功状态文案改为“保存成功即按原版重载”）
- `apps/desktop/test/r4_15_contract_test.dart`（新增）
- `apps/desktop/test/repair/r4_15_repro_test.dart`（新增，复制审计 repro 断言）
- `docs/repair/tasks/R4-15.md`、本证据目录

未改 Rust（`crates/**` 无改动），故未运行 cargo 门禁。

## 断言复现（先失败后通过）

1. 暂存（`git stash push`）两处 lib 修复，只保留新测试，对原始代码运行：
   - `test/repair/r4_15_repro_test.dart` 的碰撞用例：`Expected: '8.8.8.8' Actual: '1.1.1.1'`（与审计 `dns-baseline-collision-repro.log` 一致），exit 1。
   - `test/r4_15_contract_test.dart`：D18 同样失败；D17 失败 `Expected: 'Running' Actual: 'Stopped'`；其余 3 条通过，exit 1。
2. `git stash pop` 恢复修复后：
   - 两个文件共 10 条全部通过，exit 0。

## 命令与结果

| 命令（apps/desktop） | 结果 |
|---|---|
| `dart format lib/.../dns_window.dart lib/.../dns_controller.dart test/r4_15_contract_test.dart test/repair/r4_15_repro_test.dart` | exit 0，1 file changed |
| `flutter analyze` | exit 0，No issues found |
| `flutter test test/r4_15_contract_test.dart test/repair/r4_15_repro_test.dart` | exit 0，10/10 |
| `flutter test test/fix08_dns_draft_test.dart test/fix08b_dns_apply_test.dart test/t11_dns_test.dart` | exit 0，12/12（无回归） |

## 上游对照结论

- 保存成功→主窗 Reload 的原版时机已恢复（D17）。
- 基线按字段身份保存，同值字段不合并、预设/外部刷新不覆盖已改字段（D18）；原版没有共享文本键的脏检测算法。
- 应用/确定/取消语义：保存（及额外“应用”）成功才关闭并重载；取消不落盘不重载；失败保持窗口开放并提示。

## 未完成 / 接口缺口

- 未在真实 Windows 窗口、真实 FRB/SQLite、真实内核上验收“保存后实际解析生效”（隔离环境/内核前置缺失），未实测 DPI。
- DNS 多行 IO 失败的部分落库（settings-audit 候选 3）未在本卡处理：SimpleDNS 与两核行仍顺序提交，后段失败时前段已写。需要 Rust 单事务 + 结构化结果，建议后续卡承接。
- 未运行 `flutter test` 全量、`flutter build windows --release`（R4-32 全门禁承接）。
