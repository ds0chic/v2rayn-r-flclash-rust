# SP-23.FLD-CFG-136 — ClashUIItem.ConnectionsColumnItem

状态：implemented（实例登记完成；canonical→真实连接表联动验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-136（主 owner SP-23；消费者归属 SP-28/monitor）。

本次唯一用户流程：连接表列设置（Name/Width/Index）→保存→重开→
connections 真实表按 canonical 恢复列宽/排序；未知列保留。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI原版布局/窗口消费者；单次legacy迁移`；
另涉 SD-13/17（monitor/profiles）。

对应 ID：FLD-CFG-136；leaf；platform_scope=all；original_type=list<ColumnItem>；
original_default=[]。
关联：`application/src/monitor.rs:891/925` 列归一化，SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: ClashUIItem.ConnectionsColumnItem`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/ConfigItems.cs:219`
（`List<ColumnItem> ConnectionsColumnItem`）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `176fd39`
（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法列行/空（→默认列）；
坏行→拒绝且旧组不变。持久化 save；生效 immediate
（`domain/src/settings_timing.rs:30`）。取消提交前不写。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-28/monitor）：`apps/desktop/lib/features/monitor/clash_ui_config.dart`
（`:36` 行形状契约、`:70/:74` 组读写）、`crates/application/src/monitor.rs`
（`:891` 宽>0 才应用注释、`:925` `normalize_connection_columns`、
`:1641-1653` 重开归一化）、`crates/persistence/src/upstream_config.rs`
（`:283` 组读写）。
本卡未改生产代码。

禁止改变的已有行为：原版默认空；未知列保留；宽≤0 行不覆盖默认列宽；
其它 ClashUIItem 不动。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain/src/settings.rs:671`（`Vec<ColumnDefinition>`，默认空 `:686`）。
- DTO：`bridge_api/src/api/settings.rs:679/:693-694/:713-714` + FRB wire
  （`frb_generated.rs:6672/11208/14853`）。
- 调用链：`clash_ui_config` 读组→`normalize_connection_columns`→连接表；
  CSV current_gap 称 `connections table DataColumns fixed; canonical column
  width/order ignored`——HEAD 已有归一化与重开路径（`:1641-1653`），
  但真实列宽/排序效果仍未验收，诚实记 implemented。
- 缺口：列改→保存→重开→真实连接表端到端验收未跑。

测试夹具和原版预期：合成列组；正向 改宽/排序→重开表一致；
负向 坏行→拒绝、宽≤0→默认；未知列保留。

本次必须通过的命令/真实场景（SP-28，未运行）：`flutter test`
（monitor/表合同）+ `cargo test -p application --locked`（归一化）；
补正式入口→保存→重开→真实连接表观察。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-136.md`。

完成条件：保存、重开和真实连接表三者一致；仅归一化函数存在不算。

发现接口缺口时的处理：连接表联动缺口已登记；归属 SP-28/monitor。
