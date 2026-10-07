# SP-23.FLD-CFG-128 — HysteriaItem.HopInterval

状态：implemented（实例登记完成；有 hop/无 hop 真实会话验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-128（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：Hysteria 设置改端口跳跃间隔（HysteriaItem.HopInterval，
int，默认 30）→保存→restart_core→同 revision plan→hysteria
`hop_interval` 落值，有 hop/无 hop 真实会话验证。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-128；leaf；platform_scope=all；original_type=int；original_default=30。
关联：FLD-CFG-126/127（同 Hysteria 组，本批已登记），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: HysteriaItem.HopInterval`；
只读核对原版间隔语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `36e7472`（全 `36e7472b3b75936d2e9297c53755ce7f7e803abf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法正 int；
interval 格式/单位/无 hop/有 hop→当前 core 能力诊断。持久化 save；
生效 restart_core（`settings_timing.rs:149`）。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs`
（`:404` 投影 `base.hysteria.hop_interval`）、`crates/domain/src/settings.rs`
（`:149` default_hop_interval、`:635-646` 存储/默认）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 30；未知键保留；
stale revision 拒绝；保存/运行分离；profile 级 hop_interval 字符串通道不动
（`profile.rs:59`、`custom.rs:204`）。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain` HysteriaItem.hop_interval（`settings.rs:635-646`，默认 30）。
- DTO：`api/settings.rs:643/652/663` 往返在位 + FRB wire
  （`frb_generated.rs:7285/11983`）。
- 投影：`codegen.rs:404` 落 hop_interval（settings 级 int 通道）。
- 单测：`codegen.rs:1201/1205`（置 42→落 42 断言在位）+
  `persistence/tests/edge_cases.rs:104`（HopInterval=30 往返）。
- 缺口：有 hop/无 hop 真实会话与当前 core 能力诊断未跑（CSV current_gap）。

测试夹具和原版预期：合成间隔值；正向 42→plan 落 42；
负向 0/无 hop→core 能力诊断（待验）。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（hysteria codegen）+ 正式入口→FRB→保存→同 revision plan→真实会话→重开。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-128.md`。

完成条件：保存、plan 落值和真实会话三者一致；仅投影单测不算。

发现接口缺口时的处理：会话/core 能力验收缺口已登记；归属 SP-24。
