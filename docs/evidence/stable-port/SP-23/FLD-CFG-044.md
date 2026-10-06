# SP-23.FLD-CFG-044 — Inbound[0].Pass

状态：implemented（实例登记完成；正式入口/真实认证验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-044（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改入站认证 Pass→保存→独立重开→值一致→
同 revision plan 生成 LAN inbound 认证密码；正确/错误认证、清空与备份恢复按冻结语义；
值永不进日志/receipt/证据（本卡亦不写真实值）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-044；leaf；platform_scope=all；original_type=string；original_default=null。
关联：FLD-CFG-043（User 成对）/042/045、FLD-CFG-035..041、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:37 :: InItem.Pass`；
只读核对原版 LAN 入站认证密码语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `2ea50bd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法密码 / 清空（与 User 组合判定）；
显式 null 按空处理（`entities.rs:387`）；备份恢复后仍一致；任何日志不含密码。
持久化 save；生效 restart_core（`settings_timing.rs:155`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测；秘密不落日志、不读用户真实凭据。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs` 入站投影（同 `:418-424` 区）、
xray `inbound.rs:158` accounts 装配。
本卡未改生产代码。

禁止改变的已有行为：编辑框可用性受 NewPort4LAN 门控
（`option_setting_window.dart:604-615`，禁用只灰显不丢值）；
Dart 空串默认（`:209-210`）与 Rust null-as-empty 等价；stale 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/entities.rs:372`（`pass: String`，null-as-empty `:387`）。
- DTO：`bridge_api/src/api/settings.rs:882,899,918`（InItem 区 `:880-919`）+ FRB wire；
  Dart 侧 InItem `pass` + wire。
- 正式入口：`option_setting_window.dart:614-616`（受 NewPort4LAN 门控）。
- 投影：入站整体投影 `codegen.rs:418-424`；xray accounts `inbound.rs:158`。
- 单测：`entities.rs:387 inbound_user_pass_accept_explicit_null_as_empty`；
  `codegen.rs:1211 r4_13_s13_inbound_reaches_codegen`（入组覆盖）。
- 缺口：正式入口→FRB→持久化→重开→正确/错误认证与备份恢复完整验收未跑（CSV current_gap）。

测试夹具和原版预期：合成密码（非秘密、仅夹具）；正向 正确→认证成功；
负向 错误→拒绝、清空→组合判定；全程日志无值。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p domain --locked`
（`inbound_user_pass_accept_explicit_null_as_empty`）；
正式设置窗→FRB→保存→重开→同 revision plan→真实认证→备份恢复。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-044.md`。

完成条件：保存、重开、真实认证、备份恢复四处一致；仅序列化单测不算。

发现接口缺口时的处理：真实认证验收缺口已登记；归属 SP-24。
