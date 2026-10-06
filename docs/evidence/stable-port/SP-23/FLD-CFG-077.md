# SP-23.FLD-CFG-077 — UiItem.DoubleClick2Activate

状态：implemented（实例登记完成；单击/双击/空白/右键全路径与失败反馈验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-077（主 owner SP-23；消费者归属 SP-16）。

本次唯一用户流程：设置窗开关双击激活（UiItem.DoubleClick2Activate，bool，默认 false）
→保存→immediate→profiles 表双击行按开关决定激活 vs 打开编辑器，
行 focus/selection 不混 active，旧 active 事实失败时不变。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI 原版布局/窗口消费者；单次 legacy 迁移`；
关联 SD-03（active 身份）。

对应 ID：FLD-CFG-077；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：FLD-CFG-076（同表格交互组，本批已登记），SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: UIItem.DoubleClick2Activate`；
只读核对原版双击语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `be408da`（全 `be408da40487e0a8e3bbbe812ee9431e9484c4bf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
激活失败→反馈且旧 active 不变。正式入口 `option_setting_window.dart:881-882`
（`_bool/_set('UiItem','DoubleClick2Activate')`）。持久化 save；
生效 immediate。平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-16）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:881-882`）、`apps/desktop/lib/features/profiles/profiles_controller.dart`
（`:1626-1638` `toggleDoubleClick2Activate`/`setDoubleClick2Activate`）、
`apps/desktop/lib/features/settings/settings_controller.dart`（`:850`
播种 doubleClick）、`apps/desktop/lib/app/shell/main_shell.dart`（`:338` 切换入口）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 false（false=双击开编辑器）；focus/selection
与 active 分离；未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，rg 只读）：
- 存储：`settings_defaults.dart:120 'DoubleClick2Activate': false`。
- DTO：FRB wire `double_click2_activate`（`frb_generated.rs:10092/14152` 编解码在位）。
- 调用链：正式窗 `_set`→保存→`settings_controller:850` 播种→表格双击按值分流。
- 单测：`profiles_pointer_test.dart:23`、`r4_08_contract_test.dart:46-55`
 （false 开编辑器路径）。
- 缺口：开/关×单击/双击/空白/右键全路径 + 启动/失败反馈验收未跑
  （CSV current_gap）。

测试夹具和原版预期：合成开关；正向 true→双击激活、false→双击开编辑器；
负向激活失败→旧 active 不变且有反馈（待验）。

本次必须通过的命令/真实场景（SP-16，未运行）：`flutter test`
（`profiles_pointer`、`r4_08_contract`）；补正式窗→FRB→保存→全路径双击→
失败反馈→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-077.md`。

完成条件：保存、双击分流和失败反馈三者符合原版；仅开关落盘不算。

发现接口缺口时的处理：全路径验收缺口已登记；归属 SP-16。
