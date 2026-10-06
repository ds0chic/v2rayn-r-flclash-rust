# SP-23.FLD-CFG-122 — Mux4RayItem.XudpProxyUDP443

状态：implemented（实例登记完成；冻结值组合真实 UDP443 分流验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-122（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：mux 设置改 xudp 443 策略（Mux4RayItem.XudpProxyUDP443，
string 枚举，默认 `reject`）→保存→restart_core→同 revision plan→
xray mux `xudpProxyUDP443` 落值，reject/allow/skip 等冻结值走
合成 UDP443 分流，错误 enum 拒绝。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-122；leaf；platform_scope=all；original_type=string；
original_default=`reject`。
关联：FLD-CFG-120/121（同 mux 组，120 已有证据、121 本批），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: Mux4RayItem.XudpProxyUDP443`；
只读核对原版枚举语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `be408da`（全 `be408da40487e0a8e3bbbe812ee9431e9484c4bf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：冻结枚举值
（reject/allow/skip 等）/ 错误 enum→拒绝且旧值不变。持久化 save；
生效 restart_core。平台写入仅授权隔离机；10808 禁占，
测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs`
（`:384-385` `xudp_proxy_udp443` 落值 + 缺省 `reject`）、
`crates/bridge_api/src/api/settings.rs`（`:581/:590/:601` Mux DTO）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 `reject`；冻结枚举集合；未知键保留；
stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，rg 只读）：
- 存储：`domain` settings Mux4Ray 段（`xudp_proxy_udp443: Option<String>`）。
- DTO：`bridge_api` Mux DTO + FRB wire `xudp_proxy_udp443`
  （`frb_generated.rs:8108/8113/12307/15794` 编解码在位）。
- 投影：`codegen.rs:384-385` 落值（含缺省 `reject`）。
- 单测：`codegen.rs:1266-1278`（置 `skip`→落 `skip`；None→回退 `reject` 在位）。
- 缺口：冻结值组合合成 UDP443 分流 + 错误 enum 拒绝验收未跑
  （CSV current_gap）。

测试夹具和原版预期：合成枚举值；正向 skip→plan 落 skip；
负向错误 enum→拒绝、null→回退 reject（待验）。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（mux codegen 合同）+ `cargo test -p config_codegen --locked`（mux transport）；
补正式入口→FRB→保存→同 revision plan→合成 UDP443 分流→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-122.md`。

完成条件：保存、plan 落值和真实分流三者一致；仅投影单测不算。

发现接口缺口时的处理：真实分流验收缺口已登记；归属 SP-24。
