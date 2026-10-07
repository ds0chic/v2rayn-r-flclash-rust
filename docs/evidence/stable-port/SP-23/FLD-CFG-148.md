# SP-23.FLD-CFG-148 — CheckUpdateItem.UpdateViaProxy

状态：implemented（实例登记完成；真实代理会话更新链路验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-148（主 owner SP-23；消费者归属 SP-27/SD-16 更新链路）。

本次唯一用户流程：更新窗切换经代理更新→保存→检查/安装时 via_proxy 显式传递→
真实 session/policy 按标志走代理或直连；无可用端点→结构化 ProxyUnavailable。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-04/10 签名/安装/runner/备份链路；正式 release repo/资产/签名公钥`。

对应 ID：FLD-CFG-148；leaf；platform_scope=all；original_type=bool；
original_default=true。
关联：FLD-CFG-147（prerelease）、149（SelectedCoreTypes），SD-17。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: CheckUpdateItem.UpdateViaProxy`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/ConfigItems.cs:246`
（`bool UpdateViaProxy = true`）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `176fd39`
（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
装载 `persisted['UpdateViaProxy'] != false`（`update_controller.dart:150`，
C# 默认 true 语义 `:134`）。持久化 save（`settings_timing.rs:20` 同组）。
无端点经代理→结构化错误（`t16.rs:1300-1316`、`subs.rs:1108` 同语义）。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-27）：`update_controller.dart`（`:150` 装载、`:219` 保存）、
`bridge_port.dart`（`:363-384` 显式 viaProxy 端口）、`t16.rs`
（`:1300-1386` 单测：无端点结构化、空选择检查为空、`with_flags` 两用例）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 true；其它 CheckUpdateItem 不动；
失败不覆盖旧配置。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain/src/settings.rs:749`（默认 `true:760`）+
  `:1437` 冻结默认断言。
- DTO：`bridge_api/src/api/settings.rs:802/:811/:822` + FRB wire。
- 调用链：开关→controller→`saveGroup`→t16 `check_updates/apply_*_with_flags`
  显式传参（不再读隐式默认）。
- 缺口：真实代理/直连双路径 metadata/asset 获取端到端验收未跑；
  t16 单测未运行。

测试夹具和原版预期：合成标志；正向 开→走代理、关→直连；
负向 开但无端点→ProxyUnavailable 结构化错误；保存失败→回滚显示。

本次必须通过的命令/真实场景（SP-27，未运行）：`cargo test -p bridge_api --locked`
（t16 更新单测）+ 授权隔离机双路径真实检查→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-148.md`。

完成条件：保存、真实双路径效果和重开三者一致；仅标志落盘不算。

发现接口缺口时的处理：真实链路验收缺口已登记；归属 SP-27。
