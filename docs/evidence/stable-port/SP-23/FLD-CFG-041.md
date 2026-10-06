# SP-23.FLD-CFG-041 — Inbound[0].AllowLANConn

状态：implemented（实例登记完成；正式入口/真实 LAN 可达验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-041（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改 AllowLANConn→保存→同 revision 生成 runtime plan→
入站监听地址/远程 LAN 访问权限生效（loopback/all-interface 切换）；
隔离环境验证真实 LAN 可达性与拒绝，不改宿主网络。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-041；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：FLD-CFG-035（端口）/042（NewPort4LAN）/036/037/038/039/040、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:34 :: InItem.AllowLANConn`；
只读核对原版局域网访问语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `467607e`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
开→监听切 all-interface（LAN 可达）；关→仅 loopback。
持久化 save；生效 restart_core（`settings_timing.rs:151`）。平台写入仅授权隔离机；
宿主网络不动；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:423,486`
（`allow_lan_conn`→`allow_lan` 装配）、入站监听地址合成。
本卡未改生产代码。

禁止改变的已有行为：关不断已建本地会话；未知入站键保留；stale revision 拒绝；
保存/运行分离；不改宿主网络/防火墙。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/entities.rs:362`（默认 false，`domain/src/settings.rs:169`）。
- DTO：`bridge_api/src/api/settings.rs:879` + FRB wire（allow_lan_conn 段）；
  Dart 侧 `api/settings.dart:537` + wire。
- 正式入口：`option_setting_window.dart:592`（AllowLANConn，core tab `:499`）。
- 投影：`codegen.rs:423` + `:486 allow_lan`。
- 单测：`codegen.rs:1211 r4_13_s13_inbound_reaches_codegen`
 （`:1110-1118` allow_lan 装配 + 测试端口 11808 断言）。
- 缺口：隔离环境真实 LAN 可达/拒绝、loopback 切换验收未跑（CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 开/关→plan 监听地址→core 校验→隔离 LAN 对照；
负向 关→LAN 拒绝且本地 loopback 照常。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s13_inbound_reaches_codegen`）；
正式设置窗→FRB→保存→同 revision plan→隔离 LAN 可达/拒绝→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-041.md`。

完成条件：plan→隔离 LAN 效果闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：隔离 LAN 验收缺口已登记；归属 SP-24。
