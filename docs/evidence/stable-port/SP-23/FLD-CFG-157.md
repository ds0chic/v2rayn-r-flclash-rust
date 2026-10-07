# SP-23.FLD-CFG-157 — UiItem.WindowSizeItem[].Width

状态：implemented（实例登记完成；canonical 宽→真实窗口尺寸验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-157（主 owner SP-23；消费者归属 SP-28）。

本次唯一用户流程：窗口 resize→按 TypeName upsert 行 Width→保存→重开→
同名窗口恢复该宽（含 DPI clamp）；非正宽视为缺失回默认。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI原版布局/窗口消费者；单次legacy迁移`；
另涉 SD-18/17。

对应 ID：FLD-CFG-157；leaf（组内子键）；platform_scope=all；
original_type=int；original_default=0。
关联：FLD-CFG-082（父组）、156（TypeName）、158（Height），SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: WindowSizeItem` 宽语义；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/ConfigItems.cs:266`
下 `WindowSizeItem` 类（Width 与 Height 同类成员）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `176fd39`
（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法正宽；≤0→行视为缺失
（`get_window_size:175` `width > 0 && height > 0`）；`save_window_size`
同名 upsert、异名追加（`settings.rs:180-197`）。持久化 save；生效 immediate
（`domain/src/settings_timing.rs:317`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-28）：`crates/application/src/settings.rs`
（`save_window_size:180-197`）、`crates/domain/src/entities.rs`
（`WindowState.width:281`）、`crates/bridge_api/src/api/settings.rs`
（`WindowSizeItemDto:368-396`）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 0；非正宽回默认窗口尺寸；upsert 不碰
`MainGirdHeight*`/orientation；其它子键不动。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain/src/entities.rs:281`（i32）。
- 调用链：`save_window_size` 仅同文件单测调用（`:382-391` 1200→1440 upsert、
  `:398-400` 0 宽回 None）；真实窗口尺寸走 `window_manager`/runner，
  不读 canonical Width（CSV current_gap 同 156）。
- 缺口：resize→保存→重开→同名窗口真实宽度（含 DPI clamp/INI 迁移/
  备份）端到端验收未跑。

测试夹具和原版预期：合成宽值；正向 resize 两档→重开同宽；
负向 0/负宽→默认尺寸；多窗口互不串扰。

本次必须通过的命令/真实场景（SP-28，未运行）：`cargo test -p application --locked`
+ 真实窗口 resize→保存→重开观察。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-157.md`。

完成条件：保存、重开和真实窗口宽度三者一致；仅单测 upsert 不算。

发现接口缺口时的处理：真实窗口消费缺口已登记；归属 SP-28。
