# SP-23.FLD-CFG-025 — HappyEyeballs4RayItem（container）

状态：preserved_only（组保留/typed view 已登记；happy-eyeballs 发射由
R4-13.S11 与 `sp24_happy_fragment` 覆盖，端到端验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-025（主 owner SP-23；消费者归属 SP-24/SD-07 DNS 链路）。

本次唯一用户流程：读取/迁移 `HappyEyeballs4RayItem` 整组→typed view 供
xray DNS happy-eyeballs 块（容器为读取/迁移其结构，不造新开关）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04`。

对应 ID：FLD-CFG-025；container；platform_scope=all；original_type=object；
original_default=null。
关联：`config_codegen/src/xray/dns.rs:359-381`（仅当
`SimpleDNSItem.EnableHappyEyeballs==true` 发射），SD-02 可恢复提交；
CP-SET-01/02/03 适用。

必读上游文件、符号和固定 commit：`Config.cs :: Config.HappyEyeballs4RayItem`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/Config.cs:37`
（`HappyEyeballs4RayItem HappyEyeballs4RayItem`）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `176fd39`
（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法组/missing
（→`HappyEyeballs4RayItem::default`，`settings.rs:1007`；`try_delay_ms`
持久化缺省 `Some(250)` 见 `:1441` 断言）；坏类型→拒绝且旧组不变。
持久化 save；生效 save（冻结台账；`domain/src/settings_timing.rs:61`）。
stale revision 拒绝。平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SD-01/SP-24）：`crates/domain/src/settings.rs`
（`happy_eyeballs4_ray_item:975`、`HappyEyeballs4RayItem:877`）、
`crates/config_codegen/src/input.rs`（`:306-309/:445`）、
`crates/bridge_api/src/api/settings.rs`（DTO `:1060-1083`）。
本卡未改生产代码。

禁止改变的已有行为：仅开关开时发射 happy-eyeballs 块（`xray/dns.rs:381` 注释）；
关时不发射；未知键保留；其它 Config 组不动；不把 container 当开关。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain/src/settings.rs:975/1007`。
- 调用链：`codegen.rs:1180`（R4-13.S11 注释）→`xray/dns.rs:359-381` 条件发射。
- 单测指针：`config_codegen/tests/sp24_happy_fragment.rs`（`:6` 开关真值、
  `:37-38` 参数、`:152` 关时回退；未运行）。
- 缺口：正式入口→保存→重开→真实 DNS 块端到端验收未跑（CSV current_gap）。

测试夹具和原版预期：合成组；正向 开+参数→发射块、关→不发射；
负向 坏类型→拒绝；重开→组一致。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p config_codegen --locked`
（`sp24_happy_fragment`）+ 真实 xray 配置校验→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-025.md`。

完成条件：组保留 + 条件发射 + 重开三者一致；容器证据不能替开关生效。

发现接口缺口时的处理：端到端缺口已登记；归属 SP-24。
