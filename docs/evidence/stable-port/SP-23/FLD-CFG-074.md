# SP-23.FLD-CFG-074 — UiItem.CurrentFontFamily

状态：implemented（实例登记完成；多 DPI/缺字体 fallback 与独立窗真实字体验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-074（主 owner SP-23；消费者归属 SP-16）。

本次唯一用户流程：设置窗改字体 family（UiItem.CurrentFontFamily，可空 string，默认 null）
→保存→restart_app→所有 Flutter engine 按 family 启动排印，缺字体按 fallback，
主窗与独立设置窗真实文字一致。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI 原版布局/窗口消费者；单次 legacy 迁移`。

对应 ID：FLD-CFG-074；leaf；platform_scope=all；original_type=string；original_default=null。
关联：FLD-CFG-075（同排印组，本批已登记），SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: UIItem.CurrentFontFamily`；
只读核对原版字体 family 语义（可空）。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `be408da`（全 `be408da40487e0a8e3bbbe812ee9431e9484c4bf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 family 字符串 / null（缺省）；
未知/缺失字体→fallback，不截断/混文（待验）。正式入口
`option_setting_window.dart:954-955`（`_str/_set('UiItem','CurrentFontFamily')`）+
`theme_setting_dialog.dart:161-162`。持久化 save；生效 restart_app。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-16）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:954-955`）、`apps/desktop/lib/features/settings/theme_setting_dialog.dart`
（`:103-104/:161`）、`apps/desktop/lib/app/shell/ui_shell_controller.dart`
（`:228-230` family 读取）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 null；未知列/键保留；stale revision 拒绝；
保存/运行分离；其它 UiItem 不动。

当前 provider / caller / DTO（HEAD 实测，rg 只读）：
- 存储：`settings_defaults.dart:117 'CurrentFontFamily': null`。
- DTO：FRB wire `current_font_family`（`frb_generated.rs:10089/14149` 编解码在位）。
- 调用链：正式窗/主题对话框 `_set`→保存→重开→`ui_shell_controller:228-230` 读取、
  `theme_setting_dialog:103-104` 应用。
- 单测：`fix16_settings_field_test.dart:65` 置值、`r4_11_contract_test.dart:80` null 往返。
- 缺口：独立设置窗硬编码 light theme 且忽略 locale/font（CSV current_gap）；
  多 DPI/CJK 真实字体验收未跑。

测试夹具和原版预期：合成 family 值；正向 `Consolas`→重开真实字体变化；
负向 null→默认、缺字体→fallback 不截断（待验）。

本次必须通过的命令/真实场景（SP-16，未运行）：`flutter test`
（`fix16_settings_field`、`r4_11_contract`）；补正式窗→FRB→保存→
restart_app→主/独立窗真实文字→多 DPI→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-074.md`。

完成条件：保存、restart_app 和主/独立窗真实字体三者一致；仅落盘不算。

发现接口缺口时的处理：独立窗 locale/font 忽略缺口已登记；归属 SP-16。
