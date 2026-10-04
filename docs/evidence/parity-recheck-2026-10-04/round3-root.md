# 第三轮主窗口、测试与发布物复查

基线：冻结 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`；应用审查开始 `7edf1ee`，Windows x64 RC 元数据标明从干净 `a4ceab5` 构建。原第二轮报告 `root.md` 和其他域报告的缺口必须重新判定，不能按旧结论统计。审查期间另外的执行者正在改 `profiles_table.dart` 与拖选测试；这些未提交改动不属于本报告，未覆盖、暂存或提交。

## 已转绿的旧问题

- [真实 Windows 集成测试](round3-windows-ui-01/README.md) exit 0：上轮“选A新增节点却落无组”与“切组仍保留隐藏选择”两项，从同一冻结合同失败转为通过。限于 TUIC 手工新增和组切换，不外推到扫码/粘贴、重开或批量动作。
- `ACT-MAIN-035` 菜单与 F5 已由 `apps/desktop/lib/app/shell/main_shell.dart:160-164,421-422` 接至共享 `RuntimeController.reload()`；`main_menu.dart:156-157` 不再 `preservedOnly`。上轮“F5 完全未实现”的说法已过时。
- 当前 ZIP SHA-256 `6e9e28551376537f6dd10c244f05ffd0b03e5ae8e0878a4ee0bfd447d76dac21` 与 `dist/SHA256SUMS` 一致；exe 清单已含 `v2rayN-upgrade.exe`，旧“包内没有升级runner”过时。应用源码与包的 build commit 相差一个仅刷新 dist 元数据的提交，不能当成二进制源码遗漏。自身升级的真实远端效果另见运行域复查。
- `tools/release/v2rayn-r.iss:71-77` 已把卸载由递归删除 `{app}` 改为只清理明确的 staging/app.previous，并仅在目录为空时删除安装目录。上轮关于整删目录的静态风险已修；未实际安装/卸载测试。

## 当前仍不完全对齐

**R3-01 / P2：F5 重载的重入与空活动反馈仍不等价。** 冻结 `ServiceLib/ViewModels/MainWindowViewModel.cs:664-669,693,746-750` 在正在重载时设置 `_hasNextReloadJob`，完成后再执行一次；无默认节点会 `NoticeManager.Enqueue(CheckServerSettings)`。当前 `apps/desktop/lib/features/runtime/runtime_controller.dart:142-147` 在 `state.isBusy` 直接 `return`，无活动节点也只返回，没有面向用户的提示；`apps/desktop/test/recheck_r01_runtime_reload_test.dart` 甚至将“busy时no-op”写成通过断言。现状是菜单/F5已能触发普通 apply，但快速二次 F5 可能丢失最后一次期望，空配置时没有原版指导。没有对真实运行中重复按F5做隔离效果测试；此项为冻结源与当前代码的明确语义差异。

**R3-02 / 台账漂移：已接线动作仍标 `preserved_only`。** `compat/actions.yaml:1090-1119` 的 `ACT-MAIN-035` 仍记录 `implementation_location: null`、`status: preserved_only`，注释还说 `main_shell.dart` 未接线；这与上面现行源码相反。`RE-PROF-01-05-02` 的旧任务证据也是接线前状态。后续按台账工作的模型会误判并可能重复实现。应在任务卡/台账保留历史证据的前提下更新当前状态与准确实现定位，不删除条目或降低分母。

**R3-03 / 功能仍保留占位。** `apps/desktop/lib/app/menu/main_menu.dart:96-135` 对“以管理员身份重启”、UWP 回环代理工具、区域预置（默认/俄罗斯/伊朗）、核心网站继续标 `preservedOnly`；这些属于冻结 Windows 菜单中适用的入口，不因其他 P1 修复完成而自动验收。是否需实现每项及其平台权限/取消流程，依 `compat/actions.yaml` 与冻结源码逐条立卡；此处没有操作宿主系统。

## 本轮实际命令与边界

- `flutter test integration_test/parity_recheck_group_selection_test.dart -d windows -r expanded`：Windows Debug构建成功，1/1通过，两条细分观察均通过。见 [窗口记录](round3-windows-ui-01/README.md)。没有启动发布版 exe 或原版双窗口。
- `flutter analyze`：0 issue；`cargo fmt --all -- --check`：通过。当前外部未提交的拖选改动随时在变化，门禁是运行时快照。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过；`cargo test --workspace --locked`：exit 0，所有非 ignored 测试通过。测试通过只证明现有断言成立；预 SOCKS 启动顺序等与冻结源码相反的断言仍需纠正。
- `flutter test test/recheck_r01_runtime_reload_test.dart -r expanded`：3/3通过；其中一项正验证上述 busy no-op，**通过不等于原版合同对齐**。
- `profiles_selection_test.dart` 整文件第一次与 `flutter analyze` 并行，且运行中有人拆分/修改测试文件，出现 `did not complete`；该结果不作稳定产品结论。之后按旧的三个用例分别单进程顺序跑均通过。新增自动滚动测试仍在外部编辑中，本报告未认定其状态。
- 未运行宿主系统代理、TUN/UAC、安装器卸载、真实远端升级、原版窗口或完整 Flutter 门禁。只有合成数据，未触碰 10808 和用户秘密。
