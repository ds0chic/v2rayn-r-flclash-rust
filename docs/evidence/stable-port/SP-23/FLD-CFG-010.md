# SP-23.FLD-CFG-010 — UiItem（container）

状态：implemented（实例登记完成；组级保留/隔离正式验收未跑，不写 verified；容器只计台账覆盖，不替子字段生效）。
任务 ID：SP-23.FLD-CFG-010（主 owner SP-23；叶子消费者分属 SP-16、SD-06 等）。

本次唯一用户流程：读取/迁移 guiNConfig 整树 `UiItem` 结构（17 属性，上游类名 `UIItem`，
含列定义与窗口几何集合）→组 patch 提交→未知键保留；子字段效果按各叶卡验收
（117/118/119 列定义 canonical 无实际表格消费者缺口 G-15）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`逐ID冻结schema/default/nullable; SD-02可恢复提交`。

对应 ID：FLD-CFG-010；container；platform_scope=all；original_type=object；original_default=null。
关联：FLD-CFG-067、074/075/076/077/078/079、107/108/109/110/111/112/113、
117/118/119、121/122（已登记叶子；156/157/158 窗口几何组待本轮之外）、SD-01/SD-02/04。

必读上游文件、符号和固定 commit：`Config.cs:22 :: Config.UiItem`（类型 `UIItem`）；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/Config.cs:22`。
`work/` 仅只读核对，未改动。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `176fd39`（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法对象/缺失（回默认，
`main_column_item`/`window_size_item` 为空 Vec，`current_language` 有缺省常量）/
null/空对象/坏类型/未知键（`extra` 保留）。
持久化 save；生效 save（`settings_timing.rs:75`）。取消提交前不写；stale revision 拒绝。
无平台写入；10808 禁占。

允许修改的模块（SD-01 方向）：`crates/domain/src/settings.rs`
（`:457-494` 结构，`:496-519` Default，`:934` AppSettings 组字段）、
`crates/bridge_api/src/api/settings.rs`（`:411` UiItemDto）。
本卡未改生产代码。

禁止改变的已有行为：原版 17 属性形态（含 `MacOSShowInDock`、`MainColumnItem`、
`WindowSizeItem` 集合与 `skip_serializing_if` 零值形态）与缺省；未知键保留；
组 patch 不覆盖其它组；stale 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain` UiItem（`settings.rs:457`，Default `:496`，组字段 `:934`）。
- DTO：UiItemDto 往返在位（`bridge_api/.../settings.rs:411`）+ FRB wire +
  Dart 侧 `api/settings.dart:1273`。
- 时机：`settings_timing.rs:75` Save。
- 缺口：CSV current_gap——组级五态保留/隔离正式验收未跑；117/118/119 canonical 列定义
  无实际表格消费者、UI 用独立缓存（G-15，归属 SP-16）；整树 `unwrap_or_default`
  移除与 CP-SET-01/02/03 为前置。

测试夹具和原版预期：合成 guiNConfig；正向两组不同有效 UiItem→patch→重开一致；
负向 missing/null/empty/bad-type/未知键按冻结语义（含集合字段空/缺省）。

本次必须通过的命令/真实场景（SD-01 方向，未运行）：`cargo test -p domain --locked`
+ 正式入口→FRB→保存→独立重开→组往返与未知键保留断言。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-010.md`。

完成条件：组 patch 往返、未知键保留、它组不受影响三项闭环 + 重开；
子字段生效仍按各叶卡验收，本容器不替代。

发现接口缺口时的处理：组级验收与列定义消费缺口已登记；归属 SD-01/SP-16，不私定模块。
