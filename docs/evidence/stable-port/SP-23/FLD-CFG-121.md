# SP-23.FLD-CFG-121 — Mux4RayItem.XudpConcurrency

状态：implemented（实例登记完成；边界/禁用组合真实 UDP 路径验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-121（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：mux 设置改 xudp 并发（Mux4RayItem.XudpConcurrency，
int?，默认 16）→保存→restart_core→同 revision plan→xray mux
`xudpConcurrency` 落值，边界/禁用组合走真实 UDP 路径，
缺 core 支持显式诊断。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-121；leaf；platform_scope=all；original_type=int?；original_default=16。
关联：FLD-CFG-120（同 mux 组，已有证据）、122（本批已登记），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: Mux4RayItem.XudpConcurrency`；
只读核对原版 xudp 并发语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `be408da`（全 `be408da40487e0a8e3bbbe812ee9431e9484c4bf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法正 int / null（缺省 16）；
禁用组合→冻结语义；缺支持→显式诊断。持久化 save；生效 restart_core。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs`
（`:383` `base.mux4_ray.xudp_concurrency = mux_ray.xudp_concurrency.unwrap_or(16)`）、
`crates/bridge_api/src/api/settings.rs`（`:580/:589/:600` Mux DTO）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 16；缺省回退语义；未知键保留；
stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，rg 只读）：
- 存储：`domain` settings Mux4Ray 段（`xudp_concurrency: Option<i32>`）。
- DTO：`bridge_api` Mux DTO + FRB wire `xudp_concurrency`
  （`frb_generated.rs:8107/8112/12306/15793` 编解码在位）。
- 投影：`codegen.rs:383` 落值（含 `unwrap_or(16)`）。
- 单测：`codegen.rs:1265-1278`（置 9→落 9；None→回退 16 断言在位）。
- 缺口：边界/禁用组合与实际 UDP 路径 + 缺 core 支持诊断验收未跑
  （CSV current_gap）。

测试夹具和原版预期：合成并发值；正向 9→plan 落 9；
负向 null→回退 16、禁用组合→冻结语义（待验）。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（mux codegen 合同）+ `cargo test -p config_codegen --locked`（mux transport）；
补正式入口→FRB→保存→同 revision plan→真实 UDP 路径→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-121.md`。

完成条件：保存、plan 落值和真实 UDP 路径三者一致；仅投影单测不算。

发现接口缺口时的处理：真实路径验收缺口已登记；归属 SP-24。
