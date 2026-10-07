# SP-23.FLD-CFG-005 — KcpItem（container）

状态：implemented（实例登记完成；组级保留/隔离正式验收未跑，不写 verified；容器只计台账覆盖，不替子字段生效）。
任务 ID：SP-23.FLD-CFG-005（主 owner SP-23；叶子消费者归属 SP-24）。

本次唯一用户流程：读取/迁移 guiNConfig 整树 `KcpItem` 结构（6 属性；原版 UI tab 已注释掉，
无正式编辑入口）→组 patch 提交→未知键保留；子字段效果按 FLD-CFG-046–051 各自验收。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`逐ID冻结schema/default/nullable; SD-02可恢复提交`。

对应 ID：FLD-CFG-005；container；platform_scope=all；original_type=object；original_default=null。
关联：FLD-CFG-046/047/048/049/050/051（本组叶子）、SD-01/SD-02/04。

必读上游文件、符号和固定 commit：`Config.cs:17 :: Config.KcpItem`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/Config.cs:17`
（类定义 `ConfigItems.cs:42`）。`work/` 仅只读核对，未改动。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `176fd39`（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法对象/缺失（回默认
`mtu:1350/tti:50/uplink:12/downlink:100/cwnd:1/maxWin:2MiB`）/null/空对象/坏类型/未知键保留。
持久化 save；生效 save（`settings_timing.rs:65`）。取消提交前不写；stale revision 拒绝。
无正式 UI 入口，不虚构编辑控件；无平台写入；10808 禁占。

允许修改的模块（SD-01 方向）：`crates/domain/src/settings.rs`
（`:315-330` 结构，`:332-344` Default，`:920` AppSettings 组字段）、
`crates/bridge_api/src/api/settings.rs`（`:157` KcpItemDto）。
本卡未改生产代码。

禁止改变的已有行为：原版 6 属性形态与缺省；未知键保留；组 patch 不覆盖其它组；
stale 拒绝；保存/运行分离；不因无 UI 入口而删除该组。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain` KcpItem（`settings.rs:315`，Default `:332`，组字段 `:920`）。
- DTO：KcpItemDto 往返在位（`bridge_api/.../settings.rs:157`）+ FRB wire
  （`frb_generated.rs:7451/9344/9383/12210/15376/16699`）+ Dart 侧 `api/settings.dart:617`。
- 时机：`settings_timing.rs:65` Save。
- 组 patch 形状证据：`persistence/tests/edge_cases.rs:94`
 （`"KcpItem":{"Mtu":1350,"Tti":20}`）。
- 缺口：CSV current_gap——组级五态保留/隔离正式验收未跑；mKCP 真实会话按子字段卡验收；
  整树 `unwrap_or_default` 移除与 CP-SET-01/02/03 为前置。

测试夹具和原版预期：合成 guiNConfig；正向两组不同有效 KcpItem→patch→重开一致；
负向 missing/null/empty/bad-type/未知键按冻结语义。

本次必须通过的命令/真实场景（SD-01 方向，未运行）：`cargo test -p persistence --locked`
+ 组往返与未知键保留断言（经 FRB/DTO 通道，不造 UI 入口）。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-005.md`。

完成条件：组 patch 往返、未知键保留、它组不受影响三项闭环 + 重开；
子字段生效仍按 046–051 各卡验收，本容器不替代。

发现接口缺口时的处理：组级验收缺口已登记；归属 SD-01/SD-02，不私定模块。
