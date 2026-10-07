# SP-23.FLD-CFG-125 — Mux4SboxItem.Padding

状态：implemented（实例登记完成；开/关组合真实会话验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-125（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：mux 设置改 sing-box padding 开关（Mux4SboxItem.Padding，
bool?，默认 null）→保存→restart_core→同 revision plan→
`multiplex.padding` 落值，开/关/mux 禁用组合走真实 session。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-125；leaf；platform_scope=all；original_type=bool?；original_default=null。
关联：FLD-CFG-123/124（同 mux-sbox 组，本批已登记），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: Mux4SboxItem.Padding`；
只读核对原版 nullable 开关语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `36e7472`（全 `36e7472b3b75936d2e9297c53755ce7f7e803abf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：true/false/null（缺省）；
mux 禁用组合→冻结语义。持久化 save；生效 restart_core
（`settings_timing.rs:178`）。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs`
（`:391` 投影）、`crates/config_codegen/src/singbox/outbound.rs`
（`:811` `put_opt_bool` 落 padding）。
本卡未改生产代码。

禁止改变的已有行为：原版 nullable 缺省语义；未知键保留；
stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain` Mux4SboxItem.padding（`settings.rs:602-620` 结构段）。
- DTO：`bridge_api` Mux4SboxItemDto（`api/settings.rs:607-631`）+ FRB wire 在位。
- 投影：`codegen.rs:391`→`outbound.rs:811`（Option 语义：None 不输出）。
- 单测：`codegen.rs:1287/1291`（置 Some(true)→落 Some(true) 断言在位）。
- 缺口：开/关与 mux 禁用组合真实 session 验收未跑（CSV current_gap）。

测试夹具和原版预期：合成 true/false/null；正向 true→plan 含 padding；
负向 null→不输出、禁用组合→冻结语义（待验）。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（mux sbox codegen）+ 正式入口→FRB→保存→同 revision plan→真实会话→重开。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-125.md`。

完成条件：保存、plan 落值和真实会话三者一致；仅投影单测不算。

发现接口缺口时的处理：组合会话验收缺口已登记；归属 SP-24。
