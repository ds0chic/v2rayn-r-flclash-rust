# SP-23.FLD-CFG-004 — TunModeItem（container）

状态：implemented（实例登记完成；组级保留/隔离正式验收未跑，不写 verified；容器只计台账覆盖，不替子字段生效）。
任务 ID：SP-23.FLD-CFG-004（主 owner SP-23；叶子消费者归属 SP-24）。

本次唯一用户流程：读取/迁移 guiNConfig 整树 `TunModeItem` 结构（11 属性）→组 patch 提交→
未知键保留、其它组不受影响；子字段效果按 FLD-CFG-095–105 各自验收，不造容器开关。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`逐ID冻结schema/default/nullable; SD-02可恢复提交`。

对应 ID：FLD-CFG-004；container；platform_scope=all；original_type=object；original_default=null。
关联：FLD-CFG-095/096/097/098/099/100/101/102/103/104/105（本组叶子）、SD-01/SD-02/04。

必读上游文件、符号和固定 commit：`Config.cs:16 :: Config.TunModeItem`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/Config.cs:16`
（类定义 `ConfigItems.cs:141`）。`work/` 仅只读核对，未改动。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `176fd39`（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法对象/缺失（`deserialize_null_default` 回默认，
默认 `mtu:9000/auto_route:true/strict_route:true`）/null/空对象/坏类型/未知键（`extra` 保留）。
持久化 save；生效 save（`settings_timing.rs:74`）。取消提交前不写；stale revision 拒绝。
TUN 真实会话只授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SD-01 方向）：`crates/domain/src/settings.rs`
（`:266-291` 结构，`:293-310` Default，`:918` AppSettings 组字段）、
`crates/bridge_api/src/api/settings.rs`（`:102` TunModeItemDto）。
本卡未改生产代码。

禁止改变的已有行为：原版 11 属性 PascalCase 形态（含 `EnableIPv6Address`/`IPv4Address`/`IPv6Address`
重命名）与缺省；未知键保留；组 patch 不覆盖其它组；stale 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain` TunModeItem（`settings.rs:266`，Default `:293`，组字段 `:918`）。
- DTO：TunModeItemDto 往返在位（`bridge_api/.../settings.rs:102`）+ FRB wire
 （`frb_generated.rs:9343/9382/10016/14160/16698`）+ Dart 侧 `api/settings.dart:1209`。
- 时机：`settings_timing.rs:74` Save。
- 组 patch 形状证据：`persistence/tests/edge_cases.rs:93`
 （`"TunModeItem":{"EnableTun":false,"AutoRoute":true,"Mtu":1500}`）。
- 缺口：CSV current_gap——组级五态保留/隔离正式验收未跑；真实 TUN 会话仅 mock
  （子字段卡 G-13）；整树 `unwrap_or_default` 移除与 CP-SET-01/02/03 为前置。

测试夹具和原版预期：合成 guiNConfig；正向两组不同有效 TunModeItem→patch→重开一致；
负向 missing/null/empty/bad-type/未知键按冻结语义。

本次必须通过的命令/真实场景（SD-01 方向，未运行）：`cargo test -p persistence --locked`
+ 正式入口→FRB→保存→独立重开→组往返与未知键保留断言。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-004.md`。

完成条件：组 patch 往返、未知键保留、它组不受影响三项闭环 + 重开；
子字段生效仍按 095–105 各卡验收，本容器不替代。

发现接口缺口时的处理：组级验收缺口已登记；归属 SD-01/SD-02，不私定模块。
