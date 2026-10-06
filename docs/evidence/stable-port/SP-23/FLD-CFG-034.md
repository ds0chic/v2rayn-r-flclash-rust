# SP-23.FLD-CFG-034 — CoreBasicItem.EnableCacheFile4Sbox

状态：implemented（实例登记完成；正式入口/真实 cache file 落盘验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-034（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改 CoreBasic EnableCacheFile4Sbox 开/关→保存→
同 revision 生成 runtime plan→sing-box `experimental.cache_file` 段生成；
开→受控 data 路径建 cache（含 fakeip 存储）；关→不建段。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-034；leaf；platform_scope=all；original_type=bool；original_default=true。
关联：FLD-CFG-030/031/032/033、FLD-CFG-026..029、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:22 :: CoreBasicItem.EnableCacheFile4Sbox`
（原版默认 `= true`）；只读核对 sing-box cache_file 语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `2ea50bd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
旧缓存存在≠本次启用事实（CSV required_verification）。
持久化 save；生效 restart_core（`settings_timing.rs:92`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:380`（投影）、
sing-box `stat.rs:16-18` cache_file 段装配。
本卡未改生产代码。

禁止改变的已有行为：原版默认 true（`domain/src/settings.rs:257`）；
关不断言删旧缓存文件；未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:240`（默认 true，`:257`）。
- DTO：`bridge_api/src/api/settings.rs:62,77,94` + FRB wire；
  Dart 侧 `api/settings.dart:241` + wire。
- 正式入口：`option_setting_window.dart:709-710`（core tab `:499`）。
- 投影：`codegen.rs:380 base.core_basic.enable_cache_file4_sbox`。
- 单测：`codegen.rs:995 core_basic_item_reaches_generated_config`
 （`:1004 enable_cache_file4_sbox=true`、`:1011`、`:1031` 断言生成段）；
  `config_codegen/tests/singbox_global.rs:157 singbox_dns_and_experimental`
 （`:167 开`、`:179 enabled`、`:183 path`、`:187 store_fakeip`）。
- 缺口：正式入口→FRB→持久化→重开→真实 cache file 创建/重开行为验收未跑（CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 开→plan 含 cache_file 段→core 校验→真实建文件；
负向 关→无段；旧缓存存在不冒充启用证据。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`core_basic_item_reaches_generated_config`）+ `cargo test -p config_codegen --locked`
（`singbox_dns_and_experimental`）；
正式设置窗→FRB→保存→同 revision plan→真实 cache file→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-034.md`。

完成条件：plan→真实 cache file 闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：真实落盘验收缺口已登记；归属 SP-24。
