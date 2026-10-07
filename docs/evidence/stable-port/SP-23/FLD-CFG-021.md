# SP-23.FLD-CFG-021 — Inbound（container）

状态：preserved_only（容器结构/未知键保留与迁移闭环已登记；容器本身无新增用户功能，子字段 FLD-CFG-035..045 按各自 ID 验收，不写 verified）。
任务 ID：SP-23.FLD-CFG-021（主 owner SP-23；叶子消费者归属 SP-24/SD-07）。

本次唯一用户流程：读取/迁移 `Inbound`（`List<InItem>`）整组结构→typed view 供该组叶子消费者；
组 patch 不覆盖其它组；missing/null/empty/bad type/未知键保留（不造控件）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04`。

对应 ID：FLD-CFG-021；container；platform_scope=all；original_type=list<InItem>；original_default=null。
关联：FLD-CFG-035..045（组内叶子），SD-02 可恢复提交；CP-SET-01/02/03 适用。

必读上游文件、符号和固定 commit：`Config.cs :: Config.Inbound`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/Config.cs:33`
（`List<InItem> Inbound`）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `176fd39`
（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。

输入、输出、错误、取消、权限、持久化及生效语义：组缺失→默认单入站回填
（`domain/src/settings.rs:1035-1038`）；坏类型/未知键保留（flatten `extra`，
`domain/src/entities.rs:374-375`）。持久化 save；生效 save（冻结台账；
`domain/src/settings_timing.rs:63`）。stale revision 拒绝；保存/运行分离。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SD-01）：`crates/domain/src/settings.rs`（`inbound:960-963`、
默认 `:1003`、回填 `:1035-1038`）、`crates/domain/src/entities.rs`
（`InboundListener:352-376`）。
本卡未改生产代码。

禁止改变的已有行为：整树 `unwrap_or_default` 不得回退；未知键保留；
子字段生效时机与nullable语义不动；不把 container 当开关；不削台账。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain/src/settings.rs` 根 `inbound`（默认 `default_inbound_list:157-158`）。
- DTO/调用链：叶子行经各自 consumer（035 LocalPort / 036 Protocol / … / 045
  SecondLocalPortEnabled）；容器本身只提供组校验与 typed view。
- 缺口：组级 missing/null/empty/bad-type/未知键保留的端到端用例未跑
  （CSV current_gap）。

测试夹具和原版预期：合成 guiNConfig：正向 缺组→默认回填、未知键→保留往返；
负向 坏类型组→拒绝且旧值不变；子字段按各自 ID 验收。

本次必须通过的命令/真实场景（SD-01/SD-02，未运行）：`cargo test -p domain --locked`
+ `cargo test -p persistence --locked`（组保留/回填）；正式入口→保存→重开→
组结构一致。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-021.md`。

完成条件：组结构/未知键/迁移闭环三者一致；容器证据不能替子字段生效。

发现接口缺口时的处理：组保留用例缺口已登记；归属 SD-01/SD-02。
