# SP-23.FLD-CFG-176 — SimpleDNSItem.EnableHappyEyeballs

状态：implemented（实例登记完成；正式入口/最终效果验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-176（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：DNS 窗关闭 Happy Eyeballs 开关→一次保存→restart_core→
xray 出站 `streamSettings/sockopt` 不再出现 `happyEyeballs` 块。
容器 FLD-CFG-025 只做结构/保留验收，不当开关。

前置任务及已验证证据：SP-00（合同）；SP-01/02（损坏恢复/可恢复提交，未 verified，
跨领域前置）；SP-12（保存/生效重试，未 verified）。CSV dependencies：
`SD-07 DNS策略/IPv6; 冻结V2rayDnsService enabled/skip分支`。均未验证，不假设完成。

对应 ID：FLD-CFG-176；leaf；platform_scope=all；original_type=bool?；original_default=false。
关联：FLD-CFG-177/178/179/180（同发射点参数），SD-08。

必读上游文件、符号和固定 commit：`v2rayN/ServiceLib/Models/Configs/ConfigItems.cs ::
SimpleDNSItem.EnableHappyEyeballs`；发射语义对照 `V2rayDnsService.cs:543-547`
（先查开关，见审计 CP-SET-05）；固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `7466bee13f2c82241efb41ebad2cf74368ae09ab`。
CSV owner SD-08（SETTINGS_DATA_UPDATE_SOLUTION 分册语义：SD-08 DNS/Happy 组）。

输入、输出、错误、取消、权限、持久化及生效语义：
合法值 true/false/null（null 按 domain 默认 false，`domain/src/settings.rs:1085-1086`）。
UI 草稿：`apps/desktop/lib/features/settings/option_setting_window.dart:1057-1058`、
`features/routing/dns_window.dart:751`、`dns_controller.dart:94`。
newRevision 经 SP-02 提交（expectedRevision + datasetEpoch + mutationId；
CommitUnknown/RecoveryRequired 先恢复查询，不重放）。
持久化 save（冻结台账）；生效 restart_core（`domain/src/settings_timing.rs:207` 同组）。
取消：提交前不写；已提交的 core apply 不可假称未改变。
错误：未知结果先 query mutation/operation。

允许修改的模块（SP-24 实施，须预约 writer）：`crates/config_codegen/src/xray/dns.rs`
（`set_sockopt_domain_strategy:350-383` 接开关门控）、`crates/application/src/codegen.rs`
（调用点传 enable 上下文）。FRB/DTO 变更由整合者生成。本卡未改生产代码。

禁止改变的已有行为：原版 `V2rayDnsService` enabled/skip 真值表；
Strategy4Freedom/Proxy/Dial 既有发射；stale 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测 `rg`）：
- 存储/provider：`crates/domain/src/settings.rs:842`（`SimpleDNSItem.enable_happy_eyeballs`）；
  读取：`crates/application/src/dns.rs:407`（`get_bool(EnableHappyEyeballs)`）；
  透传：`crates/application/src/codegen.rs:578`（进 `CodegenSettings`）。
- DTO：`crates/bridge_api/src/api/settings.rs:1004`、`api/dns.rs:61,130,153`；FRB 生成物跟随。
- 最终消费者缺口 G-01：`crates/config_codegen/src/input.rs:562 enable_happy_eyeballs`
  在 `config_codegen/src` 内零读者；`dns.rs:222-266` 两处调用传 `Some(params)`，
  发射条件仅为 `params != default`（`dns.rs:374-375`）。开关关+参数非默认仍输出。

测试夹具和原版预期：合成 dualstack 配置；正向 off+默认参数（无块）/ on+非默认
（有块）；负向 off+非默认（当前复现：仍有块，错；修后应无块）；
skip×enable 真值表对照 `V2rayDnsService.cs:543-547`。

本次必须通过的命令/真实场景（SP-24，尚未运行）：
`cargo test -p config_codegen --locked`（happy 开关真值表用例）；
正式 DNS 窗→FRB→保存→restart_core→生成 JSON diff（off/on）→真实 xray 校验→
独立重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-176.md`；真实日志禁秘密；平台 OS 侧写未验证。

完成条件：关→开→关三次生成 diff 与 xray 校验一致，重开保持；只 round-trip 不算。

发现接口缺口时的处理：G-01 已登记（开关门控缺失），SP-24 修发射点，不削需求。
