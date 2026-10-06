# SP-23.FLD-CFG-094 — CoreTypeItem[].CoreType

状态：implemented（实例登记完成；锁定 core/缺核提示/真实会话验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-094（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：核心类型绑定改目标 core（CoreTypeItem[].CoreType，
enum，默认 `Xray`）→保存→restart_core→`resolve_target_core` 及 core
availability/能力校验，锁定对应 core/native 配置，缺核提示，
当前支持真实会话（App=99 不当 proxy core）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-094；leaf；platform_scope=all；original_type=enum；
original_default=`Xray`。
关联：FLD-CFG-093（同绑定组，本批已登记），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: CoreTypeItem.CoreType`；
只读核对原版目标 core 语义（默认 Xray）。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `be408da`（全 `be408da40487e0a8e3bbbe812ee9431e9484c4bf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 core enum；
缺核→提示，能力不支持→显式诊断。持久化 save；生效 restart_core。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/engine.rs`
（`:4738 resolve_target_core`、`:4747` 读绑定、`:4844/:4881` 调用链）、
`crates/bridge_api/src/api/settings.rs`（`:1119-1191` DTO）。
本卡未改生产代码。

禁止改变的已有行为：默认 Xray；App=99 不当 proxy core；未知键保留；
stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，rg 只读）：
- 存储：`domain` settings `core_type_item`（同 093）。
- DTO：`CoreTypeBindingDto` + FRB wire（同 093）。
- 调用链：保存→`resolve_target_core`→availability/能力校验→正式启动。
- 单测：`codegen.rs:1137 r4_13_s06_core_type_item_reaches_core_selection`
  同组；Dart 侧 `fix08b_dns_apply_test.dart:94-202` 按 coreType 分流对照。
- 缺口：锁定对应 core/native 配置/缺核提示/真实会话完整验收未跑
  （CSV current_gap）。

测试夹具和原版预期：合成绑定；正向 Xray/sing-box 切换→对应配置/会话；
负向缺核→提示、不把 App=99 当 proxy core（待验）。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（core 选择 + availability）；补正式入口→FRB→保存→restart_core→
真实会话→缺核提示→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-094.md`。

完成条件：保存、restart_core 和真实会话/缺核提示三者一致；仅落盘不算。

发现接口缺口时的处理：真实会话验收缺口已登记；归属 SP-24。
