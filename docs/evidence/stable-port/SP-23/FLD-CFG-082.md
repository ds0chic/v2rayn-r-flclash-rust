# SP-23.FLD-CFG-082 — UiItem.WindowSizeItem

状态：implemented（实例登记完成；canonical→真实窗口几何联动验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-082（主 owner SP-23；消费者归属 SP-28）。

本次唯一用户流程：窗口 resize/close→按 TypeName upsert 几何行→重开→
同名窗口恢复尺寸；退化行（非正尺寸）视为缺失回默认；未知 TypeName 保留。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI原版布局/窗口消费者；单次legacy迁移`；
另涉 SD-17/18（窗口/runner）。

对应 ID：FLD-CFG-082；leaf；platform_scope=all；original_type=list<WindowSizeItem>；
original_default=[]。
关联：FLD-CFG-156（TypeName）、157（Width）、158（Height），SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: UIItem.WindowSizeItem`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/ConfigItems.cs:105`
（`List<WindowSizeItem> WindowSizeItem`）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `176fd39`
（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法 (TypeName,Width,Height) 行/
空组；退化行（宽/高≤0）→视为缺失（`application/src/settings.rs:167-176`）。
持久化 save；生效 immediate（`domain/src/settings_timing.rs:310`）。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-28）：`crates/application/src/settings.rs`
（`get_window_size:170-176` GetWindowSizeItem 移植、
`save_window_size:180-197` SaveWindowSizeItem 移植）、
`crates/bridge_api/src/api/settings.rs`（DTO `:428/:454`）、
`crates/persistence/src/upstream_config.rs`（`:261/:368` 组读写）。
本卡未改生产代码。

禁止改变的已有行为：原版默认空；退化行回默认窗口尺寸；未知 TypeName 行保留；
`SaveWindowSizeItem` 不碰 `MainGirdHeight*`/orientation；其它 UiItem 不动。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain/src/settings.rs:489`（`Vec<WindowState>`，默认空 `:514`）+
  `domain/src/entities.rs:278-291`（TypeName/Width/Height + 布局 extras）。
- DTO：`bridge_api` UiItem 段 + `WindowSizeItemDto`（`:368-396`）+ FRB wire
  （`frb_generated.dart:7428/9085/11007/13186`）。
- 调用链：`get/save_window_size` 仅被同文件单测调用（`:382-408`）；
  真实窗口几何走 `window_manager`/runner INI，不读 canonical 组
  （CSV current_gap：`runner uses INI rather than canonical WindowSizeItem`）。
- 缺口：resize→保存→重开→同名窗口恢复的端到端验收未跑。

测试夹具和原版预期：合成几何组；正向 resize→重开同尺寸、退化行→默认尺寸；
负向 坏行→拒绝；多窗口 TypeName 隔离。

本次必须通过的命令/真实场景（SP-28，未运行）：`cargo test -p application --locked`
（settings 单测）+ 真实窗口 resize→保存→重开观察（含 DPI clamp）。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-082.md`。

完成条件：保存、重开和真实窗口几何三者一致；仅落盘/仅单测不算。

发现接口缺口时的处理：窗口几何联动缺口已登记；归属 SP-28。
