# SP-23.FLD-CFG-028 — CoreBasicItem.DefFingerprint

状态：implemented（实例登记完成；正式入口/真实握手验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-028（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改默认 TLS 指纹→保存→出站 TLS/Reality 默认指纹回退生效；
节点显式值按冻结优先级覆盖默认；不支持的 core 组合与真实握手对照，未知值按冻结处理。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-028；leaf；platform_scope=all；original_type=string；original_default=null。
关联：FLD-CFG-029（默认 UA）/026/027、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:10 :: CoreBasicItem.DefFingerprint`；
只读核对原版默认指纹语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `467607e`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 指纹名/空（空=不盖章）；
节点显式 fingerprint 优先于默认；非 Reality 不盖章（`custom.rs:510`）。
持久化 save；生效 restart_core（`settings_timing.rs:86`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:374`（投影）、
`crates/application/src/custom.rs:289-304`（`apply_reality_fingerprint_default`）、
`crates/application/src/engine.rs:829-933`（`default_reality_fingerprint` 读取与落盘）。
本卡未改生产代码。

禁止改变的已有行为：节点显式值优先；空默认不碰节点值；非 Reality 不盖章；
未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:228`（`Option<String>`，默认 None，`:251`）。
- DTO：`bridge_api/src/api/settings.rs:56,71,88` + FRB wire（`:6623,11070,14577`）；
  Dart 侧 `api/settings.dart:212` + wire。
- 正式入口：`option_setting_window.dart:642-643`（“默认 TLS 指纹”，core tab `:499`）。
- 投影/消费者：`codegen.rs:374`；`custom.rs:291-304` 回退规则；
  `engine.rs:832` 读取设置、`:850/:874` 与 `:905/:933` 出站落盘盖章。
- 单测：`custom.rs:496 reality_empty_fingerprint_freezes_the_default`、
  `:502 reality_explicit_fingerprint_is_kept`、
  `:510 non_reality_empty_fingerprint_is_not_stamped`、
  `:518 empty_default_leaves_fingerprint_untouched`。
- 缺口：默认/显式/不支持 core 组合的真实握手对照未跑（CSV current_gap）。

测试夹具和原版预期：合成设置修订 + 合成 Reality/非 Reality 出站；
正向 默认 chrome→Reality 缺省盖章；负向 空默认不碰、显式值保留、非 Reality 不盖章。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`reality_empty_fingerprint_freezes_the_default`、
`reality_explicit_fingerprint_is_kept`、`empty_default_leaves_fingerprint_untouched`）；
正式设置窗→FRB→保存→同 revision plan→真实握手→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-028.md`。

完成条件：plan→真实握手闭环 + 重开；仅投影/规则单测不算。

发现接口缺口时的处理：真实握手验收缺口已登记；归属 SP-24。
