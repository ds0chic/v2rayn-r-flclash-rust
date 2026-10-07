# SP-23.FLD-CFG-166 — SimpleDNSItem.DirectDNS

状态：implemented（实例登记完成；端到端正式路径验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-166（主 owner SP-23；消费者归属 SP-24 DNS codegen 方向）。

本次唯一用户流程：DNS 设置改直连 DNS（SimpleDNSItem.DirectDNS，
string?，默认 "119.29.29.29"）→保存→restart_core→同 revision plan→
直连 DNS 服务器落值（xray `:476` parse_dns_addresses）→重开仍为该值。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02; 活动 DNS/routing 优先级；core 版本能力`。

对应 ID：FLD-CFG-166；leaf；platform_scope=all；original_type=string?；
original_default="119.29.29.29"。
关联：FLD-CFG-167/168（Remote/Bootstrap DNS，同服务器组，本批已登记），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: SimpleDNSItem.DirectDNS`；
只读核对原版直连 DNS 语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `8b533f0`（全 `8b533f0f61af608564d3e0a262e2da7df5e4d9ee`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 DNS 地址/列表串；
空/非法→默认回退（DOMAIN_DIRECT_DNS_DEFAULT）。持久化 save；
生效 restart_core（`settings_timing.rs:199`）。取消/错误语义按 SP-02/SP-12。
无平台写入；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/config_codegen/src/xray/dns.rs`
（`:476` direct 解析）、`crates/domain/src/settings.rs`
（DirectDNS 字段与默认）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 119.29.29.29；双 DNS 分工（直连/代理）；
未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain` simple_dns_item.direct_dns（SimpleDNSItem 结构
  `settings.rs:803` 起 18 属性之一）。
- DTO：`bridge_api` SimpleDNS DTO direct_dns 往返在位 + FRB wire
  （`frb_generated.rs:9475/9507-9536/13508/16768` 同族）。
- 发射：xray `dns.rs:476`
 （`parse_dns_addresses(simple.direct_dns.as_ref(), DOMAIN_DIRECT_DNS_DEFAULT)`）；
  singbox 侧直连服务器组函数内消费（逐叶断言未跑，见缺口）。
- 缺口：CSV current_gap——端到端正式验收未跑。

测试夹具和原版预期：合成 DNS profile；正向默认与自定义服务器各一；
负向空串/非法→默认回退（待验）。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p config_codegen --locked`
+ 正式窗→FRB→保存→restart_core→core JSON diff→独立重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-166.md`。

完成条件：保存、plan 服务器落值和重开三者一致；仅 DTO 往返不算。

发现接口缺口时的处理：端到端验收缺口已登记；归属 SP-24。
