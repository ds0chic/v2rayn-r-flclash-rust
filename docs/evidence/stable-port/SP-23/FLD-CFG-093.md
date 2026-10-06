# SP-23.FLD-CFG-093 — CoreTypeItem[].ConfigType

状态：implemented（实例登记完成；各协议/未知 enum/删改绑定正式启动选核验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-093（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：核心类型绑定改协议键（CoreTypeItem[].ConfigType，
protocol-to-core 绑定键，enum，默认 null）→保存→`resolve_target_core`
按绑定选核，正式启动选择正确 exe，不只序列化。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-093；leaf；platform_scope=all；original_type=enum；original_default=null。
关联：FLD-CFG-094（同绑定组，本批已登记），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: CoreTypeItem.ConfigType`；
只读核对原版协议绑定键语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `be408da`（全 `be408da40487e0a8e3bbbe812ee9431e9484c4bf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法协议 enum / 未知 enum→
冻结处理；删改绑定→重开仍一致。持久化 save。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/engine.rs`
（`:4738 resolve_target_core`、`:4747` 读绑定）、
`crates/bridge_api/src/api/settings.rs`（`:1119-1191` CoreTypeBindingDto）。
本卡未改生产代码。

禁止改变的已有行为：冻结绑定优先级；未知 enum 处理；未知键保留；
stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，rg 只读）：
- 存储：`domain` settings `core_type_item`（`init_core_type_items` 初始化，
  见 `codegen.rs:1137-1141` 测试播种）。
- DTO：`bridge_api` `CoreTypeBindingDto` + FRB wire `core_type_item`
  （`frb_generated.rs:9369/13374/16647` 编解码在位）。
- 调用链：保存→`engine.rs:4738 resolve_target_core` 查 `settings.core_type_item`
  →选核。
- 单测：`codegen.rs:1137 r4_13_s06_core_type_item_reaches_core_selection`
  （绑定到达核心选择）；`engine.rs:6501/6524` 绑定置空对照。
- 缺口：各协议/默认/未知 enum/删改绑定 + 重开 + 正式启动选正确 exe
  完整验收未跑（CSV current_gap）。

测试夹具和原版预期：合成绑定；正向多协议绑定→各选正确 core；
负向未知 enum→冻结处理、删改→重开一致（待验）。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s06_core_type_item_reaches_core_selection`）；补正式入口→FRB→
保存→`resolve_target_core`→正式启动选核→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-093.md`。

完成条件：保存、重开和正式启动选核三者一致；仅序列化落盘不算。

发现接口缺口时的处理：正式选核验收缺口已登记；归属 SP-24。
