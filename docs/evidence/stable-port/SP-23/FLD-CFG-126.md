# SP-23.FLD-CFG-126 — HysteriaItem.UpMbps

状态：implemented（实例登记完成；对应 Hysteria 版本真实会话验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-126（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：Hysteria 设置改上行带宽（HysteriaItem.UpMbps，
int，默认 100）→保存→restart_core→同 revision plan→hysteria
`up_mbps` 落值，对应版本真实会话验证（不跨 Hysteria2 臆造兼容）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-126；leaf；platform_scope=all；original_type=int；original_default=100。
关联：FLD-CFG-127/128（同 Hysteria 组，本批已登记），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: HysteriaItem.UpMbps`；
只读核对原版单位语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `36e7472`（全 `36e7472b3b75936d2e9297c53755ce7f7e803abf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法正 int；
单位/零值/缺省→对应版本语义。持久化 save；生效 restart_core
（`settings_timing.rs:150`）。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs`
（`:402` 投影 `base.hysteria.up_mbps`）、`crates/bridge_api/src/api/settings.rs`
（`:641-663` Hysteria DTO）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 100；未知键保留；
stale revision 拒绝；保存/运行分离；profile 级 proto_extra 上下行不动。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain` HysteriaItem.up_mbps（`settings.rs:632/644`，默认 100）。
- DTO：`api/settings.rs:641/650/661` 往返在位 + FRB wire
  （`frb_generated.rs:7283/11981/15202`）。
- 投影：`codegen.rs:402` 落 `Some(up_mbps)`（settings 级→base.hysteria；
  与 profile 级 `proto_extra.up_mbps` 系不同通道，`codegen.rs:117-118`）。
- 单测：`codegen.rs:1199/1203`（置 55→落 Some(55) 断言在位）+
  `persistence/tests/edge_cases.rs:104`（UpMbps=10 往返）。
- 缺口：对应 Hysteria 版本真实会话验收未跑（CSV current_gap）。

测试夹具和原版预期：合成带宽值；正向 55→plan 落 Some(55)；
负向 0/缺省→版本语义（待验）。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（hysteria codegen）+ 正式入口→FRB→保存→同 revision plan→真实会话→重开。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-126.md`。

完成条件：保存、plan 落值和真实会话三者一致；仅投影单测不算。

发现接口缺口时的处理：版本会话验收缺口已登记；归属 SP-24。
