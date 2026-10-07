# SP-23.FLD-CFG-160 — SimpleDNSItem.AddCommonHosts

状态：implemented（实例登记完成；端到端正式路径验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-160（主 owner SP-23；消费者归属 SP-24 DNS codegen 方向）。

本次唯一用户流程：DNS 设置改公共 hosts 合并开关（SimpleDNSItem.AddCommonHosts，
bool?，默认 true）→保存→restart_core→同 revision plan→core DNS hosts
含预置公共 hosts（`add_common_hosts` 分支）→重开仍为该值。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02; 活动 DNS/routing 优先级；core 版本能力`。

对应 ID：FLD-CFG-160；leaf；platform_scope=all；original_type=bool?；original_default=true。
关联：FLD-CFG-159/174（同 hosts 合并组，本批已登记），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: SimpleDNSItem.AddCommonHosts`；
只读核对原版公共 hosts 合并语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `8b533f0`（全 `8b533f0f61af608564d3e0a262e2da7df5e4d9ee`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false/null
（null→默认 true）；关闭时跳过公共 hosts 合并。持久化 save；
生效 restart_core（`settings_timing.rs:191`）。取消/错误语义按 SP-02/SP-12。
无平台写入；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/config_codegen/src/xray/dns.rs`
（`:687/694` 合并门控与公共 hosts 分支）、
`crates/config_codegen/src/singbox/dns.rs`（`:123`）、
`crates/domain/src/dns.rs`（`:26/332` predefined hosts）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 true；与自定义 Hosts 冲突时合并顺序；
未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain` simple_dns_item.add_common_hosts（`settings.rs:810/852`，
  默认 Some(true)；`settings.rs:1440` 断言）。
- 透传：`application/src/codegen.rs:607`（unwrap_or(false) 投影默认值注意：
  上游默认 true，缺省投影路径待端到端确认，见缺口）。
- DTO：`bridge_api/src/api/dns.rs:45/114/137`
  + `api/settings.rs:988/1012/1038` 往返在位 + FRB wire。
- 发射：xray `dns.rs:694` 公共 hosts 分支；singbox `dns.rs:123`；
  domain `dns.rs:332` Global.PredefinedHosts 门控。
- 缺口：CSV current_gap——端到端正式验收未跑；codegen 缺省投影
  false 与上游默认 true 的差异需正式路径确认。

测试夹具和原版预期：合成 DNS profile；正向 true→含公共 hosts、
false→不含；负向 null→默认 true（待正式路径确认）。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p config_codegen --locked`
+ 正式窗→FRB→保存→restart_core→core JSON diff→独立重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-160.md`。

完成条件：保存、plan hosts 落值和重开三者一致；仅 DTO 往返不算。

发现接口缺口时的处理：缺省投影差异与端到端缺口已登记；归属 SP-24。
