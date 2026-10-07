# SP-23.FLD-CFG-022 — GlobalHotkeys（container）

状态：preserved_only（组保留/typed view 已登记；热键注册真实联动验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-022（主 owner SP-23；消费者归属桌面集成/热键链路）。

本次唯一用户流程：读取/迁移 `GlobalHotkeys`（`List<KeyEventItem>`）整组→
热键控制器装载→OS 注册；备份恢复后重读重注册（容器为读取/迁移其结构，不造控件）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04`。

对应 ID：FLD-CFG-022；container；platform_scope=all；original_type=list<KeyEventItem>；
original_default=null。
关联：`EGlobalHotkey` 五动作（showForm/三代理模式/PAC），SD-02 可恢复提交；
CP-SET-01/02/03 适用。

必读上游文件、符号和固定 commit：`Config.cs :: Config.GlobalHotkeys`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/Config.cs:34`
（`List<KeyEventItem> GlobalHotkeys`）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `176fd39`
（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法五动作绑定行/null/空组；
`KeyCode` null/0=未绑定（`hotkeys.dart:44-47/51`）。持久化 save；生效 save
（冻结台账；`domain/src/settings_timing.rs:58`）。未知动作值按 `fromValue`
回 showForm（`:22-27`），不崩溃。平台写入仅授权隔离机；10808 禁占，
测试端口 ≥11808 预探测。

允许修改的模块（桌面集成）：`apps/desktop/lib/features/settings/hotkeys.dart`
（`:66-80` settings 互转、`:29-64` 绑定模型）、
`apps/desktop/lib/app/shell/desktop_integration.dart`（`:169-174` 装载+注册、
`:441-453` 分发、` :518-519` 注销）、`apps/desktop/lib/features/backup/backup_controller.dart`
（`:213-222` 恢复后重注册）。
本卡未改生产代码。

禁止改变的已有行为：原版五动作语义与 KeyCode（WPF Key 值）编码；未知键保留；
其它 Config 组不动；不把 container 当开关。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain/src/settings.rs:965`（`global_hotkeys`，默认空 `:1004`）+
  `domain/src/entities.rs:307-316`（`GlobalHotkey`，`EGlobalHotkey` int）。
- DTO：`bridge_api` settings wire `globalHotkeys` + FRB
 （`frb_generated.dart:8376/12285/15822`）。
- 调用链：settings 文档→`hotkeyControllerProvider.loadFromSettings`→
  `registerAll`（`desktop_integration.dart:173-174`）；备份恢复→
  `reloadFromSettings`（`backup_controller.dart:222`）。
- 缺口：真实 OS 注册→按键→分发→重开端到端验收未跑（仅集成测试桩
  `sr_hotkey_conflict_test.dart:286`、`sr_real_hotkey_test.dart:208`、
  `ux_parity_fix15_hotkey_tray_test.dart:137`，均未运行）。

测试夹具和原版预期：合成绑定行；正向 注册→按键分发→对应代理动作；
负向 未绑定/冲突/未知动作→回退 showForm 且旧组不变；备份恢复→重注册。

本次必须通过的命令/真实场景（桌面集成，未运行）：`flutter test`
（热键/托盘集成）+ 授权隔离机真实按键分发；正式入口→保存→重开→注册一致。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-022.md`。

完成条件：组保留 + 真实注册分发 + 备份恢复重注册三者一致；仅落盘不算。

发现接口缺口时的处理：真实注册联动缺口已登记；归属桌面集成链路。
