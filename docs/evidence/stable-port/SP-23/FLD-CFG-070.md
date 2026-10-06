# SP-23.FLD-CFG-070 — UiItem.MainGirdOrientation

状态：implemented（实例登记完成；三布局重启应用联动验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-070（主 owner SP-23；消费者归属 SP-28）。

本次唯一用户流程：设置窗选择主窗 3 布局（EGirdOrientation 0/1/2）→保存→
重启应用→菜单/表/日志/状态位置按冻结布局生成；重开保持。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI原版布局/窗口消费者；单次legacy迁移`。

对应 ID：FLD-CFG-070；leaf；platform_scope=all；original_type=enum；original_default="Vertical"。
关联：FLD-CFG-068/069（分隔高度），SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: UIItem.MainGirdOrientation`；
只读核对原版三布局枚举语义（CSV 冻结默认 "Vertical"，Dart 默认 `1`，差异已登记）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `0b80627`
（全 `0b80627661536f6bb76b4b05c897400646076048`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法布局枚举（0/1/2）；
UI 草稿 `option_setting_window.dart:866-875`（str/int 双读）、默认
`settings_defaults.dart:113 1`。持久化 save；生效 restart_app
（`settings_timing.rs:309`）。非法值→冻结回退且旧值不变。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-28）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:866-875`）、`apps/desktop/lib/app/shell/ui_shell_controller.dart`（`:222` 读取）、
`apps/desktop/lib/features/profiles/ui_state_store.dart`（`:220` 枚举注释）。
本卡未改生产代码。

禁止改变的已有行为：原版三布局位置语义；未知值冻结回退；其它 UiItem 不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` UiItem 段（`:1289/1409` 枚举往返单测）。
- DTO：`bridge_api` settings UiItem 段 + FRB wire；Dart 侧 wire + 默认。
- 调用链：正式窗 `_set('UiItem','MainGirdOrientation')`→保存→重启应用→
  shell 布局生成。
- 单测：Dart `r4_12_contract_test.dart:170-177`（布局状态源）。
- 缺口：三布局真实位置 + restart_app + 重开端到端验收未跑；CSV/Dart 默认
  表述差异（"Vertical" vs `1`）待 SP-28 按冻结枚举对齐确认（CSV current_gap）。

测试夹具和原版预期：合成布局值；正向 0/1/2→重启应用→布局位置正确；
负向非法值→回退旧值。

本次必须通过的命令/真实场景（SP-28，未运行）：`flutter test`（`r4_12_contract`）；
补正式窗→FRB→保存→重启应用→三布局真实观察→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-070.md`。

完成条件：保存、重启应用、重开和真实布局四者一致；仅落盘/仅单测不算。

发现接口缺口时的处理：布局联动 + 默认表述对齐缺口已登记；归属 SP-28。
