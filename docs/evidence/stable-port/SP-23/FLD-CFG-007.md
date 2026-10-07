# SP-23.FLD-CFG-007 — RoutingBasicItem（container）

状态：implemented（实例登记完成；组级保留/隔离正式验收未跑，不写 verified；容器只计台账覆盖，不替子字段生效）。
任务 ID：SP-23.FLD-CFG-007（主 owner SP-23；叶子消费者归属 SP-24；路由窗口编辑关联 SP-13）。

本次唯一用户流程：读取/迁移 guiNConfig 整树 `RoutingBasicItem` 结构（3 属性，
路由窗口编辑）→组 patch 提交→未知键保留；FLD-CFG-116（RoutingIndexId）迁移缺失缺口
按其卡登记，本容器不替代。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`逐ID冻结schema/default/nullable; SD-02可恢复提交`。

对应 ID：FLD-CFG-007；container；platform_scope=all；original_type=object；original_default=null。
关联：FLD-CFG-116（RoutingIndexId，已登记；legacy→IsActive 迁移缺失 G-14）、SD-01/SD-02/04。

必读上游文件、符号和固定 commit：`Config.cs:19 :: Config.RoutingBasicItem`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/Config.cs:19`
（类定义 `ConfigItems.cs:170`）。`work/` 仅只读核对，未改动。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `176fd39`（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法对象/缺失（回默认，
`domain_strategy` 有缺省常量，`domain_strategy4_singbox`/`routing_index_id` 为 null）/
null/空对象/坏类型/未知键（`extra` 保留）。
持久化 save；生效 save（`settings_timing.rs:69`）。取消提交前不写；stale revision 拒绝。
无平台写入；10808 禁占。

允许修改的模块（SD-01 方向）：`crates/domain/src/settings.rs`
（`:377-386` 结构，`:388-397` Default，`:924` AppSettings 组字段）、
`crates/bridge_api/src/api/settings.rs`（`:231` RoutingBasicItemDto）。
本卡未改生产代码。

禁止改变的已有行为：原版 3 属性形态（含 `DomainStrategy4Singbox` 重命名）与缺省；
未知键保留；组 patch 不覆盖其它组；stale 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain` RoutingBasicItem（`settings.rs:377`，Default `:388`，组字段 `:924`）。
- DTO：RoutingBasicItemDto 往返在位（`bridge_api/.../settings.rs:231`）+ FRB wire +
  Dart 侧 `api/settings.dart:751`。
- 时机：`settings_timing.rs:69` Save。
- 缺口：CSV current_gap——组级五态保留/隔离正式验收未跑；116 legacy→IsActive 迁移缺失
  （G-14，归属 SP-13）；整树 `unwrap_or_default` 移除与 CP-SET-01/02/03 为前置。

测试夹具和原版预期：合成 guiNConfig；正向两组不同有效 RoutingBasicItem→patch→重开一致；
负向 missing/null/empty/bad-type/未知键按冻结语义。

本次必须通过的命令/真实场景（SD-01 方向，未运行）：`cargo test -p domain --locked`
（settings round-trip）+ 正式入口→FRB→保存→独立重开→组往返与未知键保留断言。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-007.md`。

完成条件：组 patch 往返、未知键保留、它组不受影响三项闭环 + 重开；
RoutingIndexId 迁移仍按 FLD-CFG-116 验收，本容器不替代。

发现接口缺口时的处理：组级验收与 116 迁移缺口已登记；归属 SD-01/SP-13，不私定模块。
