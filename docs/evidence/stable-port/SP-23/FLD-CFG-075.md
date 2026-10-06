# SP-23.FLD-CFG-075 — UiItem.CurrentFontSize

状态：implemented（实例登记完成；多字号布局/点击区/DPI 真实验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-075（主 owner SP-23；消费者归属 SP-16）。

本次唯一用户流程：设置窗改字号（UiItem.CurrentFontSize，int，默认 0）
→保存→immediate→全窗口排印即时生效，行高/点击区/菜单/表头随字号生长，
设置表单布局不拥挤。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI 原版布局/窗口消费者；单次 legacy 迁移`。

对应 ID：FLD-CFG-075；leaf；platform_scope=all；original_type=int；original_default=0。
关联：FLD-CFG-074（同排印组，本批已登记），SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: UIItem.CurrentFontSize`；
只读核对原版字号语义（默认 0）。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `be408da`（全 `be408da40487e0a8e3bbbe812ee9431e9484c4bf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 int 字号 / 0（缺省）；
非法类型→拒绝且旧值不变。正式入口 `option_setting_window.dart:959-960`
（`_int/_set('UiItem','CurrentFontSize')`）+ `theme_setting_dialog.dart:167`。
持久化 save；生效 immediate。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-16）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:959-960`）、`apps/desktop/lib/features/settings/theme_setting_dialog.dart`
（`:104/:128/:167`）、`apps/desktop/lib/app/shell/ui_shell_controller.dart`
（`:228`）、`apps/desktop/lib/shared/theme/app_theme.dart`（`:91/:190/:216`
字号生长注释）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 0；未知键保留；stale revision 拒绝；
保存/运行分离；其它 UiItem 不动。

当前 provider / caller / DTO（HEAD 实测，rg 只读）：
- 存储：`settings_defaults.dart:118 'CurrentFontSize': 0`。
- DTO：FRB wire `current_font_size`（`frb_generated.rs:10090/14150` 编解码在位）。
- 调用链：正式窗 `_set`→保存→`ui_shell_controller:228` 读取→`app_theme`
  字号生长（大字号不裁剪字形，`:216` 注释）。
- 单测：`fix16_settings_field_test.dart:66` 置 14。
- 缺口：独立设置窗硬编码 light theme 且忽略 locale/font（CSV current_gap，
  同 074）；多字号行高/点击区/菜单/表头真实布局验收未跑。

测试夹具和原版预期：合成字号；正向 14/18→即时生效且布局不拥挤；
负向坏类型→拒绝。

本次必须通过的命令/真实场景（SP-16，未运行）：`flutter test`
（`fix16_settings_field`）；补正式窗→FRB→保存→即时生效→多字号布局/
点击区/多 DPI→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-075.md`。

完成条件：保存、即时生效和多字号真实布局三者一致；仅落盘不算。

发现接口缺口时的处理：独立窗 locale/font 忽略缺口已登记（同 074）；归属 SP-16。
