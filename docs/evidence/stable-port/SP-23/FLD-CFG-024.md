# SP-23.FLD-CFG-024 — SimpleDNSItem（container）

状态：preserved_only（组保留/typed view 已登记；DNS 生成器联动由 R4-13.S11/S19
与 T11 覆盖，端到端验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-024（主 owner SP-23；消费者归属 SP-24/SD-07 DNS 链路）。

本次唯一用户流程：读取/迁移 `SimpleDNSItem` 整组→typed view 供 DNS 窗与
xray/sing-box DNS 生成器（容器为读取/迁移其结构，不造新 DNS 开关）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04`。

对应 ID：FLD-CFG-024；container；platform_scope=all；original_type=object；
original_default=null。
关联：`application/src/dns.rs`、`config_codegen/src/xray/dns.rs`、`input.rs:541`，
SD-02 可恢复提交；CP-SET-01/02/03 适用。

必读上游文件、符号和固定 commit：`Config.cs :: Config.SimpleDNSItem`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/Config.cs:36`
（`SimpleDNSItem SimpleDNSItem`）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `176fd39`
（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法 DNS 组/missing
（→`builtin`+`apply_load_defaults` 回填，`settings.rs:1006/1067-1086`）；
坏类型→拒绝且旧组不变。持久化 save；生效 save（冻结台账；
`domain/src/settings_timing.rs:70`）。stale revision 拒绝。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SD-01/SP-24）：`crates/domain/src/settings.rs`
（`simple_dns_item:973`、`SimpleDnsItem:806`、回填 `:1067-1086`）、
`crates/bridge_api/src/api/settings.rs`（DTO `:984` 段）、
`crates/bridge_api/src/api/dns.rs`（`:41/:212` SimpleDNS 窗半区）。
本卡未改生产代码。

禁止改变的已有行为：原版 nullable/缺省语义（FakeIP/globalFakeIP 等 `Option`
回填值）；未知键保留；其它 Config 组不动；不把 container 当开关。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储/回填：`domain/src/settings.rs:973/1006/1067-1086`。
- 调用链：DNS 窗 `load_simple_dns/save_simple_dns`（`api/dns.rs:27-31`）→
  `application/src/dns.rs`（`:407` EnableHappyEyeballs 读取、`:274-278`
  外部 DNS 组合）→`config_codegen/src/xray/dns.rs` SimpleDns 块
  （R4-13.S11 `:1316`、S19 注释行）。
- 单测指针：`application/tests/t11_routing_dns.rs:692`（整树保存保留
  `SimpleDNSItem.global_fake_ip`，未运行）。
- 缺口：正式 DNS 窗→保存→重开→真实 core DNS 块端到端验收未跑
  （CSV current_gap）。

测试夹具和原版预期：合成 DNS 组；正向 缺组→builtin 回填、改远端 DNS→生成器输出变化；
负向 坏类型→拒绝；重开→组一致。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（t11）+ `cargo test -p config_codegen --locked`；正式窗→FRB→保存→生成器
diff→真实校验→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-024.md`。

完成条件：组保留/回填 + 生成器输出 + 重开三者一致；容器证据不能替叶子生效。

发现接口缺口时的处理：DNS 端到端缺口已登记；归属 SP-24。
