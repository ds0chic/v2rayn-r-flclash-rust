# SP-23.FLD-CFG-170 — SimpleDNSItem.Strategy4Proxy

状态：implemented（实例登记完成；端到端正式路径验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-170（主 owner SP-23；消费者归属 SP-24 DNS codegen 方向）。

本次唯一用户流程：DNS 设置改代理出站域名策略（SimpleDNSItem.Strategy4Proxy，
string?，默认 null）→保存→restart_core→同 revision plan→代理侧
targetStrategy 落值（xray `:234-246`）→重开仍为该值。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02; 活动 DNS/routing 优先级；core 版本能力`。

对应 ID：FLD-CFG-170；leaf；platform_scope=all；original_type=string?；original_default=null。
关联：FLD-CFG-169/171（Strategy 同族，本批已登记），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: SimpleDNSItem.Strategy4Proxy`；
只读核对原版代理策略语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `8b533f0`（全 `8b533f0f61af608564d3e0a262e2da7df5e4d9ee`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法策略枚举串；
空/AsIs→不下发（`:238` AsIs 跳过）。持久化 save；生效 restart_core
（`settings_timing.rs:222`）。取消/错误语义按 SP-02/SP-12。
无平台写入；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/config_codegen/src/xray/dns.rs`
（`:234-246` proxy 策略→targetStrategy）、`crates/domain/src/settings.rs`
（Strategy4Proxy 字段与默认）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 null（不下发）；AsIs 跳过语义；
未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain` simple_dns_item.strategy4_proxy（SimpleDNSItem 结构
  `settings.rs:803` 起 18 属性之一）。
- DTO：`bridge_api` SimpleDNS DTO strategy4_proxy 往返在位 + FRB wire
  （`frb_generated.rs:9460/9479/13512/16772` 同族）。
- 发射：xray `dns.rs:234-246`（非空且非 AsIs→`targetStrategy` 落值）。
- 缺口：CSV current_gap——端到端正式验收未跑。

测试夹具和原版预期：合成 DNS profile；正向有效策略→targetStrategy 落值、
null/AsIs→不下发；负向非法串→冻结处理（待验）。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p config_codegen --locked`
+ 正式窗→FRB→保存→restart_core→core JSON diff→独立重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-170.md`。

完成条件：保存、plan 策略落值和重开三者一致；仅 DTO 往返不算。

发现接口缺口时的处理：端到端验收缺口已登记；归属 SP-24。
