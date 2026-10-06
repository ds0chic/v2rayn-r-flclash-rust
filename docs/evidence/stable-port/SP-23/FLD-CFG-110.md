# SP-23.FLD-CFG-110 — SpeedTestItem.IPAPIUrl

状态：implemented（实例登记完成；合成 schema/错误 URL 真实 IP 显示验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-110（主 owner SP-23；消费者归属 SP-29）。

本次唯一用户流程：设置窗改 IP 检查 URL（SpeedTestItem.IPAPIUrl，
string，默认 null）→保存→immediate→IP 检查经 HttpPolicy 请求，
按 JSONPath 解析 IP/geo 字段，实际 UI 显示，不读用户节点。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`；关联 SD-10（HttpPolicy）。

对应 ID：FLD-CFG-110；leaf；platform_scope=all；original_type=string；original_default=null。
关联：FLD-CFG-107/108/109（同测速组，本批已登记），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: SpeedTestItem.IPAPIUrl`；
只读核对原版 IP 检查语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `be408da`（全 `be408da40487e0a8e3bbbe812ee9431e9484c4bf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 URL / null（缺省）；
空串→过滤（`speedtest.rs:419` trim 语义）；坏 schema/错误 URL→错误显示。
正式入口 `option_setting_window.dart:1014-1016`
（`_str/_set('SpeedTestItem','IPAPIUrl')`）。持久化 save；
生效 immediate。平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测；
不读用户节点秘密。

允许修改的模块（SP-29）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:1014-1016`）、`crates/application/src/speedtest.rs`
（`:72/:118/:1083` `ipapi_url` 消费）、
`crates/bridge_api/src/api/speedtest.rs`（`:408/:419`）、
`crates/domain/src/settings.rs`（`:550/:568`）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 null；空串过滤语义；未知键保留；
stale revision 拒绝；保存/运行分离；秘密不入日志。

当前 provider / caller / DTO（HEAD 实测，rg 只读）：
- 存储：`settings_defaults.dart:139 'IPAPIUrl': null`；
  `domain/src/settings.rs:550/568`。
- DTO：`bridge_api` `ipapi_url`（`settings.rs:537/551/567`、
  `speedtest.rs:408/419`）+ FRB wire
  （`frb_generated.rs:4422/9606/13626/16790` 在位）。
- 调用链：正式窗 `_set`→保存→`speedtest.rs:1083` 读 `settings.ipapi_url`→
  请求/解析→UI 显示。
- 单测：`speedtest.rs:1026`（`ipapi_url.is_none()` 缺省断言）。
- 缺口：合成正确/坏 schema/IP 与 geo 字段/错误 URL 真实 UI 显示验收未跑
  （CSV current_gap）。

测试夹具和原版预期：合成 URL；正向正确 schema→UI 显示 IP/geo；
负向坏 schema/错误 URL→错误显示（待验）。

本次必须通过的命令/真实场景（SP-29，未运行）：`cargo test -p application --locked`
（speedtest）+ `flutter test`；补正式窗→FRB→保存→合成 IP 源真实请求→
UI 显示→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-110.md`。

完成条件：保存、真实请求解析和 UI 显示三者一致；仅落盘不算。

发现接口缺口时的处理：真实 IP 显示验收缺口已登记；归属 SP-29。
