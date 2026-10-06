# SP-23.FLD-CFG-076 — UiItem.EnableDragDropSort

状态：implemented（实例登记完成；跨组/锁订阅/取消/多选拖动与重开顺序验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-076（主 owner SP-23；消费者归属 SP-16）。

本次唯一用户流程：设置窗开关拖动排序（UiItem.EnableDragDropSort，bool，默认 false）
→保存→restart_app→profiles 表格按开关注册/解注册行拖动重排，
顺序持久化，重开保持正确 Id 顺序。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI 原版布局/窗口消费者；单次 legacy 迁移`；
关联 SD-03（profiles 顺序身份）。

对应 ID：FLD-CFG-076；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：FLD-CFG-077（同表格交互组，本批已登记），SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: UIItem.EnableDragDropSort`；
只读核对原版拖动排序语义（`ProfilesView.xaml:27-34` 对应，见 provider 注释）。
`work/` 仅只读核对（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `be408da`（全 `be408da40487e0a8e3bbbe812ee9431e9484c4bf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
拖动取消→顺序不变。正式入口 `option_setting_window.dart:924-925`
（`_bool/_set('UiItem','EnableDragDropSort')`）。持久化 save；
生效 restart_app（冻结台账）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-16）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:924-925`）、`apps/desktop/lib/features/profiles/profiles_controller.dart`
（`:31-50` `profilesEnableDragDropSortProvider`/`readEnableDragDropSort`）、
`apps/desktop/lib/features/profiles/profiles_table.dart`（`:165-167` 按开关注册拖动）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 false；冻结 restart_app 时机；锁订阅组拖动语义；
未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，rg 只读）：
- 存储：`settings_defaults.dart:119 'EnableDragDropSort': false`。
- DTO：FRB wire `enable_drag_drop_sort`（`frb_generated.rs:10091/14151` 编解码在位）。
- 调用链：正式窗 `_set`→保存→重开→`readEnableDragDropSort` 读文档→
  `profiles_table:165-167` 条件注册行拖动。
- 缺口：同组/跨组/锁订阅/拖动取消/多选真实拖动与重开顺序验收未跑
  （CSV current_gap：production consumer traced statically 级别）。

测试夹具和原版预期：合成开关；正向 true→拖动重排→重开顺序保持；
负向 false→无拖动注册、拖动取消→顺序不变。

本次必须通过的命令/真实场景（SP-16，未运行）：`flutter test`（profiles 相关）；
补正式窗→FRB→保存→restart_app→同组/跨组/锁订阅真实拖动→取消→重开。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-076.md`。

完成条件：保存、重开和真实拖动顺序三者一致；仅开关落盘不算。

发现接口缺口时的处理：真实拖动矩阵验收缺口已登记；归属 SP-16。
