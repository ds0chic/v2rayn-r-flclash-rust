# SP-23.FLD-CFG-149 — CheckUpdateItem.SelectedCoreTypes

状态：implemented（实例登记完成；真实按核检查→安装链路验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-149（主 owner SP-23；消费者归属 SP-27/SD-16 更新链路）。

本次唯一用户流程：更新窗选目标核心集合（null=全部，空=不检查）→保存→
`t16_check_updates` 按集合检查；空选择→检查为空；失败可见、可回滚。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-04/10 签名/安装/runner/备份链路；正式 release repo/资产/签名公钥`。

对应 ID：FLD-CFG-149；leaf；platform_scope=all；original_type=list<string>；
original_default=null。
关联：FLD-CFG-147/148（同 flags 组），SD-17。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: CheckUpdateItem.SelectedCoreTypes`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/ConfigItems.cs:247`
（`List<string>? SelectedCoreTypes`）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `176fd39`
（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法 null（→全部，
`update_controller.dart:134` 注释）/非空集合/空集合（→检查为空，
`t16.rs:1347-1354`）；排序后存（`:220` `..sort()`）。持久化 save
（`settings_timing.rs:19` 同组）。保存失败静默忽略为已知语义缺口
（CSV：`option save failure still silently ignored`），不得宣称已修。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-27）：`update_controller.dart`（`:137` 装载、
`:220` 排序保存）、`bridge_port.dart`（`:363-373` cores 显式端口）、
`t16.rs`（`:824-827` 按核检查、`:1326-1354` 无源/空选择单测）。
本卡未改生产代码。

禁止改变的已有行为：null=全部、空=不检查的冻结语义（`backend null-vs-empty
fixed` 已在树内，见 CSV）；其它 CheckUpdateItem 不动。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain/src/settings.rs:751`（`Option<Vec<String>>`，默认 `None:761`）。
- DTO：`bridge_api/src/api/settings.rs:803/:812/:823` + FRB wire
  （`frb_generated.dart:6440/9608/13730/14768`）。
- 调用链：多选→controller 排序→`saveGroup`→t16 按集合检查/安装。
- 缺口：真实按核 metadata/asset→签名→安装→runner/备份回滚端到端未跑；
  保存失败回滚未修（CSV current_gap）；t16 单测未运行。

测试夹具和原版预期：合成集合；正向 null→全核、子集→仅子集、空→检查为空；
负向 未知核名→结构化错误；保存失败→旧集合保留（待修）。

本次必须通过的命令/真实场景（SP-27，未运行）：`cargo test -p bridge_api --locked`
（t16）+ 授权隔离机真实按核检查→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-149.md`。

完成条件：保存、真实按核检查和重开三者一致；仅集合落盘不算。

发现接口缺口时的处理：保存回滚与真实链路缺口已登记；归属 SP-27。
