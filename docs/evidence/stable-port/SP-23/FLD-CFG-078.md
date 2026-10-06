# SP-23.FLD-CFG-078 — UiItem.AutoHideStartup

状态：implemented（实例登记完成；自启隐藏后托盘恢复/缺托盘保留入口验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-078（主 owner SP-23；消费者归属 SP-16/SP-18）。

本次唯一用户流程：设置窗开关启动隐藏（UiItem.AutoHideStartup，bool，默认 false）
→保存→immediate→下次启动（含自启场景）主窗初始隐藏，
托盘可恢复窗口，缺托盘保留可见入口。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI 原版布局/窗口消费者；单次 legacy 迁移`；
关联 SD-18（DesktopIntegration）。

对应 ID：FLD-CFG-078；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：FLD-CFG-079（同窗口生命周期组，本批已登记），SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: UIItem.AutoHideStartup`；
只读核对原版启动隐藏语义（`MainWindow.xaml.cs:146/321` 对应，见测试注释）。
`work/` 仅只读核对（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `be408da`（全 `be408da40487e0a8e3bbbe812ee9431e9484c4bf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false。
正式入口 `option_setting_window.dart:912-913`
（`_bool/_set('UiItem','AutoHideStartup')`）。持久化 save；生效 immediate
（下次启动时读取）。平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-16/SP-18）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:912-913`）、`apps/desktop/lib/app/shell/desktop_integration.dart`
（`:178/:187` 启动隐藏语义、`start()` 决策）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 false；窗口构造期一次性读取；
缺托盘保留可见入口；未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，rg 只读）：
- 存储：`settings_defaults.dart:121 'AutoHideStartup': false`。
- DTO：FRB wire `auto_hide_startup`（`frb_generated.rs:10093/14153` 编解码在位）。
- 调用链：正式窗 `_set`→保存→启动→`DesktopIntegration.start()` 读值决定隐藏。
- 单测：`fix15b_startup_test.dart:11-52`（true 隐藏/false 可见/非法值/托盘组合）、
  集成 `ux_parity_fix15b_startup_test.dart:37`（重开隐藏）。
- 缺口：自启场景隐藏后托盘打开/退出、缺托盘保留入口真实验收未跑
  （CSV current_gap）。

测试夹具和原版预期：合成开关；正向 true→启动隐藏→托盘恢复；
负向 false→正常显示（单测已覆）；缺托盘→保留可见入口（待验）。

本次必须通过的命令/真实场景（SP-16/SP-18，未运行）：`flutter test`
（`fix15b_startup`）；补正式窗→FRB→保存→普通/自启双场景重开→托盘恢复/
退出→缺托盘对照。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-078.md`。

完成条件：保存、双场景启动隐藏和托盘恢复三者一致；仅落盘不算。

发现接口缺口时的处理：自启/缺托盘验收缺口已登记；归属 SP-16/SP-18。
