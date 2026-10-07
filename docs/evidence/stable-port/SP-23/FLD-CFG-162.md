# SP-23.FLD-CFG-162 — SimpleDNSItem.GlobalFakeIp

状态：implemented（实例登记完成；端到端正式路径验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-162（主 owner SP-23；消费者归属 SP-24 DNS codegen 方向）。

本次唯一用户流程：DNS 设置改全局 FakeIP 范围（SimpleDNSItem.GlobalFakeIp，
bool?，默认 true）→保存→restart_core→同 revision plan→fakeip 全局/
非全局分支切换（`global_fake_ip != Some(false)` 门控）→重开仍为该值。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02; 活动 DNS/routing 优先级；core 版本能力`。

对应 ID：FLD-CFG-162；leaf；platform_scope=all；original_type=bool?；original_default=true。
关联：FLD-CFG-161/163（同 fakeip 组，本批已登记），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: SimpleDNSItem.GlobalFakeIp`；
只读核对原版全局 fakeip 语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `8b533f0`（全 `8b533f0f61af608564d3e0a262e2da7df5e4d9ee`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false/null
（null→默认 true，`settings.rs:1073-1074` 回填）；Some(false)→非全局分支。
持久化 save；生效 restart_core（`settings_timing.rs:212`）。
取消/错误语义按 SP-02/SP-12。无平台写入；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/config_codegen/src/xray/dns.rs`
（`:604` 全局门控）、`crates/config_codegen/src/singbox/dns.rs`
（`:297/472/493` 全局/非全局分支）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 true；FakeIP 总开关语义；
未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain` simple_dns_item.global_fake_ip（`settings.rs:814/854`，
  默认 Some(true)；缺省回填 `:1073-1074`）。
- DTO：`bridge_api/src/api/dns.rs:47/116/139/253-254`
  + `api/settings.rs:990/1014` 往返在位 + FRB wire。
- 发射：xray `dns.rs:604`（global_fake_ip != Some(false) 门控）；
  singbox `dns.rs:297/472/493`（== Some(false) 非全局分支）。
- 单测：`bridge_api/src/api/dns.rs:410-470` FIX-08
  （save_simple_dns 缺省不擦除 global_fake_ip，`:424/449/464/470`）。
- 缺口：CSV current_gap——端到端正式验收未跑。

测试夹具和原版预期：合成 DNS profile；正向 true→全局分支、
Some(false)→非全局分支；负向 null→回填 true。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p config_codegen --locked`
+ `cargo test -p bridge_api --locked`（FIX-08）+ 正式窗→FRB→保存→
restart_core→core JSON diff→独立重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-162.md`。

完成条件：保存、plan 分支落值和重开三者一致；仅 DTO 单测不算。

发现接口缺口时的处理：端到端验收缺口已登记；归属 SP-24。
