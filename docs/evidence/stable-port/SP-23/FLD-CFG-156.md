# SP-23.FLD-CFG-156 — UiItem.WindowSizeItem[].TypeName

状态：implemented（实例登记完成；canonical 行→真实窗口映射验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-156（主 owner SP-23；消费者归属 SP-28）。

本次唯一用户流程：窗口打开/关闭/备份恢复→按 TypeName 查找几何行
（`GetWindowSizeItem` 语义）→内部类型键定位其 Width/Height；
未知 TypeName 行保留但不参与定位（内部字段走相应 identity 流程，不造控件）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI原版布局/窗口消费者；单次legacy迁移`；
另涉 SD-18/17。

对应 ID：FLD-CFG-156；internal（组内子键）；platform_scope=all；
original_type=string；original_default=null。
关联：FLD-CFG-082（父组）、157（Width）、158（Height），SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: WindowSizeItem.TypeName`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/ConfigItems.cs:266-268`
（`class WindowSizeItem` / `string TypeName`）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `176fd39`
（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法窗口类型键
（`MainWindow`、对话框名…）/未知键（保留，不定位）；退化尺寸行视为缺失→
回默认（`application/src/settings.rs:170-176`）。持久化 save；生效 immediate
（`domain/src/settings_timing.rs:316`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-28）：`crates/application/src/settings.rs`
（`get_window_size:170-176` 按 TypeName 查找）、
`crates/domain/src/entities.rs`（`WindowState.type_name:280`）、
`crates/persistence/src/upstream_config.rs`（`:261` 组读）。
本卡未改生产代码。

禁止改变的已有行为：按名精确匹配（大小写按冻结语义）；退化行回默认；
未知 TypeName 行保留；其它 WindowSizeItem 子键不动。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain/src/entities.rs:280`（`String`，serde PascalCase）。
- 调用链：`get_window_size(settings, type_name)`→行定位（仅同文件单测调用
  `:385/:391/:398-400`，含 `UnknownWindow` 缺失断言）。
- 缺口：真实窗口打开→TypeName 定位→几何应用端到端验收未跑
  （CSV current_gap：`canonical column/geometry children stored but not
  consumed by actual UI`）。

测试夹具和原版预期：合成几何组；正向 已知键→定位行、未知键→None 回默认；
负向 退化行→None；备份恢复→行保留。

本次必须通过的命令/真实场景（SP-28，未运行）：`cargo test -p application --locked`
（settings 单测）+ 真实多窗口打开→几何恢复观察。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-156.md`。

完成条件：存储、真实定位和重开三者一致；仅单测定位不算。

发现接口缺口时的处理：真实窗口消费缺口已登记；归属 SP-28。
