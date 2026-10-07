# SP-23.FLD-CFG-147 — CheckUpdateItem.CheckPreReleaseUpdate

状态：implemented（实例登记完成；真实检查→签名→安装链路验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-147（主 owner SP-23；消费者归属 SP-27/SD-16 更新链路）。

本次唯一用户流程：更新窗勾选预发布通道→保存→检查更新时 prerelease 标志
显式传递→metadata/asset 按通道选择；失败可见、可回滚（以正式 release
repo/pubkey 与安装路径为前置）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-04/10 签名/安装/runner/备份链路；正式 release repo/资产/签名公钥`。

对应 ID：FLD-CFG-147；leaf；platform_scope=all；original_type=bool；
original_default=false。
关联：FLD-CFG-148（via_proxy）、149（SelectedCoreTypes），SD-17。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: CheckUpdateItem.CheckPreReleaseUpdate`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/ConfigItems.cs:245`
（`bool CheckPreReleaseUpdate`）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `176fd39`
（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false。
Dart 先改本地 option 再 `saveGroup`（`update_controller.dart:173` 注释
`OnCheckPreReleaseUpdateChanged -> SaveConfig`、` :214-220` 组装）；
持久化 save（`settings_timing.rs:18` 同组）。检查失败可见、
持久化失败不静默（CSV 记 `local option changed before saveGroup;
unsuccessful persistence not shown/rolled back` 为待修语义缺口）。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-27）：`apps/desktop/lib/features/update/update_controller.dart`
（`:134-150` 装载、` :173/:214-220` 保存）、
`apps/desktop/lib/features/update/check_update_view.dart`（`:48-54` 复选框）、
`apps/desktop/lib/bridge/bridge_port.dart`（`:363-384` 显式 flags 端口、
`:377-380` with_flags 注释）、`crates/bridge_api/src/api/t16.rs`
（`:824-827` 按核检查、`:1209-1225` `with_flags` 自更新 staging）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 false；`null SelectedCoreTypes`=>全量（`:134` 注释）；
仅 v2rayN/Xray 跟随 prerelease 通道（`updater/src/channel.rs:169-170`）；
其它 CheckUpdateItem 不动。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain/src/settings.rs:747`（默认 `false:759`）。
- DTO：`bridge_api/src/api/settings.rs:800-803/:807-823` + FRB wire。
- 调用链：复选框→controller→`saveGroup`→t16 `check_updates(cores, prerelease,
  viaProxy)`→`metadata.rs:219-248` 通道选择（含 `:300-304` 预发布单测）。
- 缺口：真实 repo/pubkey/签名/安装/runner/备份回滚端到端验收未跑；
  `app_repo=None` 时签名源不可信（CSV current_gap）；t16 单测未运行。

测试夹具和原版预期：合成通道标志；正向 开→取最新（含预发布）、关→仅稳定版；
负向 无稳定版仅预发布→结构化错误（`metadata.rs:316-320`）；保存失败→回滚显示。

本次必须通过的命令/真实场景（SP-27，未运行）：`cargo test -p updater --locked`
（通道选择）+ `flutter test`（更新窗合同）；正式窗→保存→真实检查→重开。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-147.md`。

完成条件：保存、真实通道选择和重开三者一致；仅标志落盘不算。

发现接口缺口时的处理：签名/安装前置与保存回滚缺口已登记；归属 SP-27。
