# SP-23.FLD-CFG-059 — GuiItem.KeepOlderDedupl

状态：implemented（实例登记完成；正式订阅去重端到端验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-059（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗选择去重保留旧/新→保存→重开→profiles 去重服务按
KeepOlderDedupl 保留旧（true）或新（false）节点身份，active/sub 关联正确。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-059；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：SD-07/SD-03；订阅合并 `subscriptions/src/merge.rs:12`。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: GUIItem.KeepOlderDedupl`；
只读核对原版去重保留旧/新语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `0b80627`
（全 `0b80627661536f6bb76b4b05c897400646076048`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
UI 草稿 `option_setting_window.dart:844-845`、默认 `settings_defaults.dart:101 false`。
持久化 save；生效 save（`settings_timing.rs:125`）。
合成重复节点新旧顺序、标签/订阅/当前节点关联及重开；不能仅 list 去重数量。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:844-845`）、`apps/desktop/lib/features/profiles/profiles_controller.dart`
（`:58-74 readKeepOlderDedupl`、`:1828-1838` 去重调用）、
`crates/subscriptions/src/merge.rs`（`:12` 语义）、`crates/bridge_api/src/api/subs.rs`
（`:599` 事务注释）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 false（保留新）；active/sub 关联语义不动；
保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` GuiItem 段（默认 false）。
- DTO：`bridge_api` settings GuiItem 段 + FRB wire；Dart 侧 wire + 默认。
- 调用链：正式窗 `_set('GuiItem','KeepOlderDedupl')`→保存→重开→
  `readKeepOlderDedupl`→`deduplicate`→merge collapse。
- 单测：Dart `r4_13_s10_contract_test.dart:19/26`、`recheck_r3_prof_controller_test.dart:240-305`
 （false=newer wins）、`profile_dedup.dart:73` 上游对齐注释；Rust `fix10_speedtest_result.rs:10` 引用语义。
- 缺口：正式订阅导入重复节点新旧顺序 + active/sub 关联 + 重开端到端验收未跑（CSV current_gap）。

测试夹具和原版预期：合成重复节点（新旧顺序、标签/订阅/当前节点）；
正向 true→留旧、false→留新且关联正确；负向坏类型→拒绝。

本次必须通过的命令/真实场景（SP-24，未运行）：`flutter test`
（`r4_13_s10_contract`、`recheck_r3_prof_controller`）+ `cargo test -p subscriptions --locked`；
补正式窗→FRB→保存→合成订阅去重→重开→关联校验。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-059.md`。

完成条件：保存、重开和去重身份/关联三者一致；仅 list 数量不算。

发现接口缺口时的处理：端到端验收缺口已登记；归属 SP-24。
