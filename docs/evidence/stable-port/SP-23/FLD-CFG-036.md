# SP-23.FLD-CFG-036 — Inbound[0].Protocol

状态：implemented（实例登记完成；正式入口缺少独立编辑器、真实协议行为验收未跑，
不写 verified）。
任务 ID：SP-23.FLD-CFG-036（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：入站协议（冻结内部值：socks/http/mixed 等）→保存→同 revision 生成
runtime plan→各 core 入站协议/类型及端口偏移选择生效；真实 socks 与 HTTP 请求对照；
它不是“新协议开关”，只是既有入站的类型选择。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-036；leaf；platform_scope=all；original_type=string；
original_default="socks"。
关联：FLD-CFG-035（端口）/037/038/039/040/041（同入站组）、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:29 :: InItem.Protocol`；
只读核对原版入站协议语义（默认 "socks"）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `467607e`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 冻结内部协议值；
未知值按冻结处理。混合/分开入站按协议装配端口偏移。
持久化 save；生效 restart_core（`settings_timing.rs:156`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:524`
（`inbound_protocol_token` 映射）、`:418-430`（入站投影）、
`domain/src/enums.rs:538-572`（`InboundProtocol` 枚举/数值）。
本卡未改生产代码。

禁止改变的已有行为：未知协议值不静默映射；不虚构新协议；未知入站键保留；
stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/entities.rs:354`（`protocol: InboundProtocol`）；
  枚举 `domain/src/enums.rs:538-546`（`Mixed=6`，`:559-572`）。
- DTO：`bridge_api/src/api/settings.rs` InItemDto + FRB wire；
  Dart 侧 `api/settings.dart` + wire。
- 正式入口缺口：`option_setting_window.dart` 入站区（`:499-615`）无 Protocol 独立编辑器，
  仅 `:153 'Protocol': 0` 默认——正式入口待 SP-24 补齐，已登记。
- 投影：`codegen.rs:524 inbound_protocol_token` + `:418-430`。
- 单测：`codegen.rs:1211 r4_13_s13_inbound_reaches_codegen`
 （`:1223 protocol=Mixed`、`:1234` 断言 "mixed"）。
- 缺口：正式入口编辑器缺失；真实 socks/HTTP 请求对照、混合/分开入站验收未跑
 （CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 socks/mixed 等→plan→core 校验→真实请求；
负向 未知协议按冻结处理。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s13_inbound_reaches_codegen`）；
正式入口（待补）→FRB→保存→同 revision plan→真实 socks/HTTP→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-036.md`。

完成条件：正式编辑器 + plan→真实协议行为闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：正式入口编辑器缺失已登记；归属 SP-24，不私定模块。
