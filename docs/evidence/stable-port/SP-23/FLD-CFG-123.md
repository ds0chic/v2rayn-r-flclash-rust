# SP-23.FLD-CFG-123 — Mux4SboxItem.Protocol

状态：implemented（实例登记完成；正式入口→真实会话验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-123（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：mux 设置改 sing-box 复用协议（Mux4SboxItem.Protocol，
string，默认 "h2mux"）→保存→restart_core→同 revision plan→
sing-box outbound `multiplex.protocol` 落值，真实会话验证。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-123；leaf；platform_scope=all；original_type=string；original_default="h2mux"。
关联：FLD-CFG-124/125（同 mux-sbox 组，本批已登记），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: Mux4SboxItem.Protocol`；
只读核对原版协议语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `36e7472`（全 `36e7472b3b75936d2e9297c53755ce7f7e803abf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法协议串 / null
（缺省回退 "h2mux"）；冻结协议枚举与 core 支持，不臆造兼容。
持久化 save；生效 restart_core（`settings_timing.rs:179`）。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs`
（`:388-391` 投影，`unwrap_or("h2mux")`）、
`crates/config_codegen/src/singbox/outbound.rs`（`:799-813` 落 multiplex）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 "h2mux"；缺省回退语义；未知键保留；
stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain` Mux4SboxItem 段（`settings.rs:602-620`，默认见 `:616-620`）。
- DTO：`bridge_api` Mux4SboxItemDto（`api/settings.rs:607-631`）+
  FRB wire（`frb_generated.rs:8119-8128/12325-12343/15799-15803` 编解码在位）。
- 投影：`codegen.rs:389` 落 protocol（含回退）→`outbound.rs:809` 写
  `multiplex.protocol`（mux_enabled 且非空才输出）。
- 单测：`codegen.rs:1281-1295`（置 smux→落 smux；None→回退 h2mux 断言在位）+
  `application/tests/t18_settings_chain.rs:77/139`（Protocol 进 settings 链）。
- 缺口：正式入口→FRB→保存→同 revision plan→真实会话验收未跑
  （CSV current_gap）。

测试夹具和原版预期：合成协议值；正向 smux→plan 落 smux；
负向 null→回退 h2mux、core 不支持→显式诊断（待验）。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（mux sbox codegen）+ 正式入口→FRB→保存→同 revision plan→真实会话→重开。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-123.md`。

完成条件：保存、plan 落值和真实会话三者一致；仅投影单测不算。

发现接口缺口时的处理：真实会话验收缺口已登记；归属 SP-24。
