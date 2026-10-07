# SP-23.FLD-CFG-083 — UiItem.HideColumnIpInfo

状态：implemented（实例登记完成；真实列隐藏联动验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-083（主 owner SP-23；消费者归属 SP-28）。

本次唯一用户流程：设置隐藏 IP 列开关→保存→重开→profiles 真实表隐藏
IP/geo 列（数据不删，仅列显隐）；开/关往返恢复。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI原版布局/窗口消费者；单次legacy迁移`；
另涉 SD-13（profiles 表）。

对应 ID：FLD-CFG-083；leaf；platform_scope=all；original_type=bool；
original_default=false。
关联：SD-06/SD-13。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: UIItem.HideColumnIpInfo`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/ConfigItems.cs:106`
（`bool HideColumnIpInfo`）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `176fd39`
（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false。
UI 草稿→`applySettingsDocument`（`ui_shell_controller.dart:241`
`hideIpInfo: ui['HideColumnIpInfo'] == true`）。持久化 save；生效 immediate
（`domain/src/settings_timing.rs:304`）。隐藏只影响列显隐，不删数据。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-28）：`apps/desktop/lib/app/shell/ui_shell_controller.dart`
（`:66/:97/:130/:155` 状态管、` :241` 读取）、`crates/bridge_api/src/api/settings.rs`
（UiItem DTO 段）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 false；隐藏≠删除数据；其它 UiItem 不动。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain/src/settings.rs:491`（默认 `false:515`）。
- DTO：`bridge_api` UiItem 段 + FRB wire。
- 调用链：文档→`applySettingsDocument`→`UiShellState.hideIpInfo`；
  全树 grep 该状态无任何表列读取点（CSV current_gap：
  `hideIpInfo state assigned but never read`——HEAD 仍成立）。
- 缺口：开/关→保存→重开→真实列显隐端到端验收未跑。

测试夹具和原版预期：合成开关；正向 开→IP 列隐藏且数据在、关→恢复；
负向 坏类型→拒绝；重开→状态一致。

本次必须通过的命令/真实场景（SP-28，未运行）：`flutter test`
（profiles/表列合同）；补正式开关→保存→重开→真实列观察。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-083.md`。

完成条件：保存、重开和真实列显隐三者一致；仅状态赋值不算。

发现接口缺口时的处理：列显隐联动缺口已登记；归属 SP-28。
