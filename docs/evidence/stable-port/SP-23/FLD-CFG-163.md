# SP-23.FLD-CFG-163 — SimpleDNSItem.FakeIPRange

状态：implemented（实例登记完成；逐叶 CIDR 落值验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-163（主 owner SP-23；消费者归属 SP-24 DNS codegen 方向）。

本次唯一用户流程：DNS 设置改 FakeIP 池 CIDR（SimpleDNSItem.FakeIPRange，
string?，默认 "198.18.0.0/15"）→保存→restart_core→同 revision plan→
fakeip 池 CIDR 落值→重开仍为该值。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02; 活动 DNS/routing 优先级；core 版本能力`。

对应 ID：FLD-CFG-163；leaf；platform_scope=all；original_type=string?；
original_default="198.18.0.0/15"。
关联：FLD-CFG-161/162（同 fakeip 组，本批已登记），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: SimpleDNSItem.FakeIPRange`；
只读核对原版池 CIDR 语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `8b533f0`（全 `8b533f0f61af608564d3e0a262e2da7df5e4d9ee`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 IPv4/IPv6 CIDR；
非法/空→默认回填（`settings.rs:1070-1071` FAKE_IP_RANGE）。
持久化 save；生效 restart_core（`settings_timing.rs:211`）。
取消/错误语义按 SP-02/SP-12。无平台写入；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/config_codegen/src/xray/dns.rs`
与 `crates/config_codegen/src/singbox/dns.rs` fakeip 池组函数、
`crates/domain/src/settings.rs`（`:816` 字段、`:1070-1071` 默认回填）。
本卡未改生产代码。

禁止改变的已有行为：原版默认池；非法 CIDR 回退语义；
未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain` simple_dns_item.fake_ip_range（`settings.rs:815-816/855`，
  缺省回填 `:1070-1071`）。
- DTO：`bridge_api/src/api/dns.rs:48/117/140`
  （空串过滤）+ `api/settings.rs:991/1015` 往返在位 + FRB wire。
- 发射：xray/singbox fakeip 池组函数内消费（与 161 同分支族；
  本叶 CIDR 逐值落值断言未跑，见缺口）。
- 缺口：CSV current_gap——端到端正式验收未跑；逐叶 CIDR 落值
  （合法/非法/IPv6）断言缺失。

测试夹具和原版预期：合成 DNS profile；正向默认池与自定义 CIDR 各一；
负向非法 CIDR/空串→默认回填（待验）。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p config_codegen --locked`
+ 正式窗→FRB→保存→restart_core→core JSON diff→独立重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-163.md`。

完成条件：保存、plan 池 CIDR 落值和重开三者一致；仅回填逻辑不算。

发现接口缺口时的处理：逐叶落值断言缺口已登记；归属 SP-24。
