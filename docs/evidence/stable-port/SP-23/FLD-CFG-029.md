# SP-23.FLD-CFG-029 — CoreBasicItem.DefUserAgent

状态：implemented（实例登记完成；正式入口/真实请求 header 验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-029（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改默认 User-Agent→保存→出站 HTTP/WS 等默认 header 生成生效；
节点显式 header 优先；空/自定义/节点覆盖组合经多 transport 真实请求 header 对照；
凭据不入证据。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-029；leaf；platform_scope=all；original_type=string；original_default=null。
关联：FLD-CFG-028（默认指纹）/026/027、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:12 :: CoreBasicItem.DefUserAgent`；
只读核对原版默认 UA 语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `467607e`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 UA 字符串/空（空=不注入）；
节点显式 header 优先于默认；UA 别名按 `raw_user_agent_value` 归一
（`config_codegen/src/util.rs:567`）。持久化 save；
生效 restart_core（`settings_timing.rs:87`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测；凭据/secret 不入日志与证据。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:375`（投影）、
`crates/config_codegen/src/xray/outbound.rs:669`（xray 消费）、
`crates/config_codegen/src/singbox/outbound.rs:944`（sing-box 消费）。
本卡未改生产代码。

禁止改变的已有行为：节点显式 header 优先；空默认不注入；未知键保留；
stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:230`（`Option<String>`，默认 None，`:252`）。
- DTO：`bridge_api/src/api/settings.rs:57,72,89` + FRB wire（`:6624,11071,14578`）；
  Dart 侧 `api/settings.dart:213` + wire。
- 正式入口：`option_setting_window.dart:647-648`（DefUserAgent，core tab `:499`）。
- 投影/消费者：`codegen.rs:375`；`config_codegen/src/input.rs:190` 输入；
  `xray/outbound.rs:669`（`:844-854` grpc UA 落盘）、`singbox/outbound.rs:944-947`。
- 单测：`config_codegen/tests/xray_transport_security.rs:24,73`、
  `config_codegen/tests/singbox_transport_security.rs:21`
 （`def_user_agent="chrome"/"curl"` 组合）；`gen_matrix.rs:222` 矩阵用例。
- 缺口：空/自定义/节点覆盖的多 transport 真实请求 header 对照未跑（CSV current_gap）。

测试夹具和原版预期：合成设置修订 + 合成出站；正向 默认 chrome→出站 header；
负向 空不注入、节点显式覆盖、grpc/sing-box 分支各自对照。

本次必须通过的命令/真实场景（SP-24，未运行）：
`cargo test -p config_codegen --locked`（transport_security 两组）；
正式设置窗→FRB→保存→同 revision plan→多 transport 真实请求 header→重开。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-029.md`。

完成条件：plan→真实请求 header 闭环 + 重开；仅投影/生成器单测不算。

发现接口缺口时的处理：真实请求验收缺口已登记；归属 SP-24。
