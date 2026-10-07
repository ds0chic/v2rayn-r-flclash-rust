# SP-23.FLD-CFG-068 — UiItem.MainGirdHeight1

状态：implemented（实例登记完成；真实 splitter 联动验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-068（主 owner SP-23；消费者归属 SP-28）。

本次唯一用户流程：主窗水平/垂直布局拖动分隔条→保存分隔高度 1→重开→
同一 splitter 恢复该高度；极小高度/三布局/未知列共存不丢失。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI原版布局/窗口消费者；单次legacy迁移`。

对应 ID：FLD-CFG-068；leaf；platform_scope=all；original_type=int；
original_default=0。
关联：FLD-CFG-069（高度 2）、FLD-CFG-070（布局方向），SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: UIItem.MainGirdHeight1`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/ConfigItems.cs:91`
（`int MainGirdHeight1`，原版拼写 `Gird` 保留）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `176fd39`
（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法非负 int；
非法值→冻结回退且旧值不变。持久化 save；生效 immediate
（`domain/src/settings_timing.rs:307`）。取消提交前不写。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-28）：`crates/application/src/settings.rs`
（`save_main_grid_height:201-204`，`SaveMainGirdHeight` 移植）、
`crates/bridge_api/src/api/settings.rs`（DTO `:413/:437/:466`）、FRB wire
（`frb_generated.dart:8904/12984/16274`）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 0；`SaveWindowSizeItem` 不碰
`MainGirdHeight*`（`settings.rs:178-179` 注释）；其它 UiItem 不动。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain/src/settings.rs:461`（默认 `0:500`）。
- DTO：`bridge_api` UiItem 段 + FRB wire（Dart/ Rust 双向在位）。
- 调用链：`save_main_grid_height` 仅被同文件单测调用
  （`:406-408`，320/480 断言）；真实 splitter 走 `ui_state_store`
  `horizontal_split/vertical_split`（`ui_shell_controller.dart:189-190`），
  不读 canonical `MainGirdHeight1`（CSV current_gap）。
- 缺口：拖动→保存→重开→splitter 恢复的端到端验收未跑。

测试夹具和原版预期：合成高度值；正向两组不同高度→重开恢复；
负向非法值→回退旧值；与 069/070 组合不互扰。

本次必须通过的命令/真实场景（SP-28，未运行）：`cargo test -p application --locked`
（settings 单测）+ `flutter test`（布局合同）；补正式布局→保存→重开→
真实 splitter 观察。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-068.md`。

完成条件：保存、重开和真实 splitter 三者一致；仅落盘/仅单测不算。

发现接口缺口时的处理：splitter 联动缺口已登记；归属 SP-28。
