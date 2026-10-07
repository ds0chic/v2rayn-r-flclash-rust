# SP-23.FLD-CFG-174 — SimpleDNSItem.Hosts

状态：implemented（实例登记完成；端到端正式路径验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-174（主 owner SP-23；消费者归属 SP-24 DNS codegen 方向）。

本次唯一用户流程：DNS 设置改用户自定义 hosts（SimpleDNSItem.Hosts，
string?，默认 null）→保存→restart_core→同 revision plan→hosts 解析合并落值
（xray `:706`；singbox `dns.rs:137/233`、`routing.rs:157`）→重开仍为该值。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02; 活动 DNS/routing 优先级；core 版本能力`。

对应 ID：FLD-CFG-174；leaf；platform_scope=all；original_type=string?；original_default=null。
关联：FLD-CFG-159/160（同 hosts 合并组，本批已登记），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: SimpleDNSItem.Hosts`；
只读核对原版 hosts 解析语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `8b533f0`（全 `8b533f0f61af608564d3e0a262e2da7df5e4d9ee`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 hosts 文本
（多行/列表/IPv6）；空→跳过合并（`:689` is_empty 门控）；非法行→冻结处理。
持久化 save；生效 restart_core（`settings_timing.rs:213`）。
取消/错误语义按 SP-02/SP-12。无平台写入；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/config_codegen/src/xray/dns.rs`
（`:706` parse_hosts_to_dictionary）、`crates/config_codegen/src/singbox/dns.rs`
（`:137/233`）、`crates/config_codegen/src/singbox/routing.rs`（`:157`）、
`crates/domain/src/settings.rs`（Hosts 字段与默认）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 null；与 159/160 合并优先级；
未知 raw 键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain` simple_dns_item.hosts（SimpleDNSItem 结构
  `settings.rs:803` 起 18 属性之一）。
- DTO：`bridge_api` SimpleDNS DTO hosts 往返在位 + FRB wire
  （`frb_generated.rs:9483/13516/16776` 同族）。
- 发射：xray `dns.rs:706`
 （`parse_hosts_to_dictionary(simple.hosts.as_deref())`）；singbox
  `dns.rs:137/233`、`routing.rs:157`（hosts 字典遍历）。
- 缺口：CSV current_gap——端到端正式验收未跑；非法行/冲突语义待验。

测试夹具和原版预期：合成 hosts 文本；正向合法多行与 IPv6 各一；
负向非法行/空→冻结处理与门控跳过（待验）。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p config_codegen --locked`
+ 正式窗→FRB→保存→restart_core→core JSON diff→独立重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-174.md`。

完成条件：保存、双核 plan 落值和重开三者一致；仅解析函数在位不算。

发现接口缺口时的处理：非法行/冲突语义与端到端缺口已登记；归属 SP-24。
