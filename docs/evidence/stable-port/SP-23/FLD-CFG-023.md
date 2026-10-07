# SP-23.FLD-CFG-023 — CoreTypeItem（container）

状态：preserved_only（组保留/`init_core_type_items` 语义已登记；按行 core 选择
效果由 R4-13.S06 覆盖，端到端验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-023（主 owner SP-23；消费者归属 SP-24/SD-07 core 选择）。

本次唯一用户流程：读取/迁移 `CoreTypeItem`（`List<CoreTypeItem>`）整组→
`init_core_type_items` 补全→`core_for(configType)` 逐行 core 选择
（容器为读取/迁移其结构，不造新 core 开关）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04`。

对应 ID：FLD-CFG-023；container；platform_scope=all；
original_type=list<CoreTypeItem>；original_default=null。
关联：`engine.rs` `GetCoreType` 语义（`:4736/:4820/:4840/:6511`），SD-02 可恢复提交；
CP-SET-01/02/03 适用。

必读上游文件、符号和固定 commit：`Config.cs :: Config.CoreTypeItem`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/Config.cs:35`
（`List<CoreTypeItem> CoreTypeItem`）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `176fd39`
（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法 (configType,coreType) 行/
null（→`init_core_type_items` 补全，`domain/src/settings.rs:1139-1150`）；
坏行→拒绝且旧组不变。持久化 save；生效 save（冻结台账；
`domain/src/settings_timing.rs:56`）。stale revision 拒绝。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SD-01/SP-24）：`crates/domain/src/settings.rs`
（`core_type_item:967`、默认 `None:1005`、`init_core_type_items:1139-1150`）、
`crates/domain/src/entities.rs`（`CoreTypeBinding:321-336`）、
`crates/bridge_api/src/api/settings.rs`（DTO `:962` 段）。
本卡未改生产代码。

禁止改变的已有行为：原版按配置类型选 core 的冻结优先级（节点显式优先，
`engine.rs:4736` 注释）；未知键保留；其它 Config 组不动；不把 container 当开关。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain/src/settings.rs:967`（`Option<Vec<CoreTypeBinding>>`）。
- 调用链：`core_for(ConfigType)`→行绑定→core 选择（`codegen.rs:1135-1151`
  R4-13.S06 单测：Vless 改 SingBox，其余保持 Xray）。
- 缺口：正式入口→保存→重开→真实 core 启动的端到端验收未跑
  （CSV current_gap；单测未运行）。

测试夹具和原版预期：合成绑定组；正向 null→补全默认、改 Vless 行→该类型走新 core；
负向 坏行→拒绝；重开→选择一致。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s06_core_type_item_reaches_core_selection`）+ 真实 core 配置校验→重开。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-023.md`。

完成条件：组保留/补全 + 按行选择 + 重开三者一致；容器证据不能替行生效。

发现接口缺口时的处理：端到端 core 选择缺口已登记；归属 SP-24。
