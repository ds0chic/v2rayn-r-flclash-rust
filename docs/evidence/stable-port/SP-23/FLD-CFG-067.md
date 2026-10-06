# SP-23.FLD-CFG-067 — UiItem.EnableAutoAdjustMainLvColWidth

状态：implemented（实例登记完成；真实列宽计算联动验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-067（主 owner SP-23；消费者归属 SP-28）。

本次唯一用户流程：设置窗勾选主列表自动调整列宽→保存→重开→主 profiles
真实列宽按开关计算；开/关、100/150/200% DPI 下手动宽度与未知列不丢失。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI原版布局/窗口消费者；单次legacy迁移`。

对应 ID：FLD-CFG-067；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：FLD-CFG-117/118/119（canonical 列 Name/Width/Index），SD-06/SD-13。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: UIItem.EnableAutoAdjustMainLvColWidth`；
只读核对原版主表列宽自动调整语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `0b80627`
（全 `0b80627661536f6bb76b4b05c897400646076048`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
UI 草稿 `option_setting_window.dart:849-851`、默认 `settings_defaults.dart:110 false`、
透传 `ui_shell_controller.dart:243 autoAdjustColWidth`。
持久化 save；生效 immediate。手动宽度/未知列不丢失。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-28）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:849-851`）、`apps/desktop/lib/app/shell/ui_shell_controller.dart`（`:243`）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 false；手动宽度与未知列保留；其它 UiItem 不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` UiItem 段（默认 false）。
- DTO：`bridge_api` settings UiItem 段 + FRB wire；Dart 侧 wire + 默认。
- 调用链：正式窗 `_set('UiItem','EnableAutoAdjustMainLvColWidth')`→保存→重开→
  shell controller 列宽输入。
- 缺口：真实列宽计算（开/关 × DPI × 手动宽度）端到端验收未跑；CSV 记
  `autoAdjustColWidth state assigned but never read`——HEAD 已见 `:243` 读取，
  但列宽效果仍未验收，诚实记 implemented。

测试夹具和原版预期：合成列布局；正向开→列宽重算、关→保持手动宽度；
多 DPI 与未知列保留；负向坏类型→拒绝。

本次必须通过的命令/真实场景（SP-28，未运行）：`flutter test`（settings/布局合同）；
补正式窗→FRB→保存→重开→多 DPI 真实列宽观察。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-067.md`。

完成条件：保存、重开和真实列宽三者一致；仅落盘不算。

发现接口缺口时的处理：列宽效果验收缺口已登记；归属 SP-28。
