# SP-23.FLD-CFG-056 — GuiItem.AutoRun

状态：implemented（实例登记完成；正式入口/OS 事实验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-056（主 owner SP-23；消费者归属 SP-15/SP-32）。

本次唯一用户流程：设置窗勾选开机自启→保存→重开→OS Run 键/计划任务事实存在；
取消勾选→事实删除；合成 OS 写失败→同窗重试可见失败（非静默落盘）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-04; autostart actual fact/ownership adapter; 受控OS PC`。

对应 ID：FLD-CFG-056；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：SD-05/SD-18。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: GUIItem.AutoRun`；
只读核对原版开机自启写 Run 键/任务计划语义（以冻结源码为准）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `393fafd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
UI 草稿 `option_setting_window.dart:923-924`、`settings_defaults.dart:98`。
SP-02 提交语义；持久化 save（desired）；生效 immediate
（`settings_timing.rs:119`），但 OS 事实以写后读回为准——落盘 ≠ 生效。
失败（合成 OS 写失败、无权限）返回分阶段事实并允许重试已保存内容。

允许修改的模块（SP-15/SP-32）：`apps/desktop/lib/features/settings/settings_controller.dart`
（`:169,409,460,512,568-582` desired/applied 分离与 `_writeAutostart`、
`:787-794` 文档/草稿读取）、`crates/application/src/platform_service.rs`
（`:153-177` 后端装配、`:556-572` enable/disable、`AutoStartBackend`）。
本卡未改生产代码。

禁止改变的已有行为：不得把 persisted desired 当 Run 键事实；
提权/降权路径语义不动；其它 GuiItem 不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:404,428`（默认 false）。
- DTO：`bridge_api` settings GuiItem 段 + FRB wire（以生成物为准）。
- 调用链：草稿→`settings_controller`→`platform_service.set_autostart`→`AutoStartBackend`。
- 缺口：同窗重试已修（CSV）；`load()` 误把 desired 当已生效确认
  （`_autostartApplied` 语义）；admin 破坏/无权限真实路径未验证。

测试夹具和原版预期：合成 OS 后端（内存 Run 键）；正向 开→事实存在→重开仍开、
关→事实删除；负向 写失败→分阶段失败+重试成功、load 不伪报。

本次必须通过的命令/真实场景（SP-15/SP-32，未运行）：`cargo test -p application --locked`（autostart 合同 `:866-878`）；
正式窗勾选→FRB→保存→重开→隔离机 Run 键事实读写。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-056.md`。

完成条件：desired/applied/OS 事实三者一致 + 失败重试；仅落盘不算。

发现接口缺口时的处理：load 伪确认与 admin 路径已登记；归属 SP-15/SP-32。
