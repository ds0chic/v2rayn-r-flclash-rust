# SP-23.FLD-CFG-079 — UiItem.Hide2TrayWhenClose

状态：implemented（实例登记完成；主窗 X 双路径与独立窗关闭非退出真实验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-079（主 owner SP-23；消费者归属 SP-16/SP-18）。

本次唯一用户流程：设置窗开关关闭到托盘（UiItem.Hide2TrayWhenClose，bool，默认 false）
→保存→immediate→主窗 X 按开关决定隐藏到托盘 vs 真实退出，
独立窗关闭不退出应用，重开与托盘菜单一致。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI 原版布局/窗口消费者；单次 legacy 迁移`；
关联 SD-18（DesktopIntegration）。

对应 ID：FLD-CFG-079；leaf；platform_scope=['linux']（CSV 台账值；见缺口说明）；
original_type=bool；original_default=false。
关联：FLD-CFG-078（同窗口生命周期组，本批已登记），SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: UIItem.Hide2TrayWhenClose`；
只读核对原版关闭语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `be408da`（全 `be408da40487e0a8e3bbbe812ee9431e9484c4bf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false。
正式入口 `option_setting_window.dart:907-908`
（`_bool/_set('UiItem','Hide2TrayWhenClose')`）。持久化 save；
生效 immediate。平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-16/SP-18）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:907-908`）、`apps/desktop/lib/app/shell/desktop_integration.dart`
（`:35-36` 读取、`:154/:194/:277` 关闭语义注释）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 false；独立窗关闭非退出；未知键保留；
stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，rg 只读）：
- 存储：`settings_defaults.dart:122 'Hide2TrayWhenClose': false`。
- DTO：FRB wire `hide2_tray_when_close`（`frb_generated.rs:10094/14154` 编解码在位）。
- 调用链：正式窗 `_set`→保存→`desktop_integration:35-36` 读取→
  关闭按钮按值隐藏或退出。
- 缺口：① 台账 platform_scope=['linux']，但 `DesktopIntegration` 读取为跨平台，
  Windows 行为是否适用冻结语义未核定；② 主窗 X 双路径 + 独立窗关闭 +
  托盘菜单真实验收未跑（CSV current_gap）。

测试夹具和原版预期：合成开关；正向 true→X 隐藏/托盘可恢复、
false→X 真实退出；负向独立窗关闭→应用不退出（待验）。

本次必须通过的命令/真实场景（SP-16/SP-18，未运行）：`flutter test`
（`fix15b_startup` 含托盘组合）；补正式窗→FRB→保存→双路径关闭→
独立窗对照→托盘菜单→重开；核定 Windows 适用性。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-079.md`。

完成条件：保存、双路径关闭和独立窗对照三者一致 + 平台适用性核定；
仅落盘不算。

发现接口缺口时的处理：平台适用性 + 真实关闭验收缺口已登记；归属 SP-16/SP-18。
