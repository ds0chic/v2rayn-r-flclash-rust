# SP-23.FLD-CFG-111 — SpeedTestItem.UdpTestTarget

状态：implemented（实例登记完成；合成 UDP 回显/范围/超时/取消结果分类验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-111（主 owner SP-23；消费者归属 SP-29）。

本次唯一用户流程：设置窗改 UDP 测试目标（SpeedTestItem.UdpTestTarget，
string，默认 `ntp:pool.ntp.org`）→保存→immediate→Rust UDP target
parser 解析→对应 core 真实 UDP 会话，结果分类准确。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-111；leaf；platform_scope=all；original_type=string；
original_default=`ntp:pool.ntp.org`。
关联：FLD-CFG-109/110（同测速组，本批已登记），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: SpeedTestItem.UdpTestTarget`；
只读核对原版 UDP 目标语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `be408da`（全 `be408da40487e0a8e3bbbe812ee9431e9484c4bf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法目标串 / 空串→过滤；
端口范围/超时/取消→结果分类。正式入口
`option_setting_window.dart:1008-1010`
（`_str/_set('SpeedTestItem','UdpTestTarget')`）。持久化 save；
生效 immediate。平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-29）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:1008-1010`）、`crates/application/src/speedtest.rs`
（`:73/:119/:1028` parser 与会话）、
`crates/bridge_api/src/api/speedtest.rs`（`:409/:420`）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 `ntp:pool.ntp.org`；空串过滤；
未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，rg 只读）：
- 存储：`settings_defaults.dart:140 'UdpTestTarget': 'ntp:pool.ntp.org'`。
- DTO：`bridge_api` `udp_test_target`（`settings.rs:538/552/568`、
  `speedtest.rs:409/420`）+ FRB wire
  （`frb_generated.rs:4423/9617/13627/16791` 在位）。
- 调用链：正式窗 `_set`→保存→`speedtest.rs:119 non_empty`→`:1028` 会话。
- 单测：`codegen.rs:1354/1368`（`udp_test_target` 透传断言在位）。
- 缺口：合成 UDP 回显 server、端口范围/超时/取消真实结果分类验收未跑
  （CSV current_gap）。

测试夹具和原版预期：合成目标；正向回显 server→成功分类；
负向超时/取消→对应分类（待验）。

本次必须通过的命令/真实场景（SP-29，未运行）：`cargo test -p application --locked`
（speedtest UDP）；补正式窗→FRB→保存→合成回显真实会话→取消→重开。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-111.md`。

完成条件：保存、真实 UDP 会话和结果分类三者一致；仅落盘不算。

发现接口缺口时的处理：真实 UDP 会话验收缺口已登记；归属 SP-29。
