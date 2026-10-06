# SP-23.FLD-CFG-062 — GuiItem.EnableHWA

状态：implemented（实例登记完成；可行性/最终效果验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-062（主 owner SP-23；消费者归属 SP-26）。

本次唯一用户流程：设置窗勾选硬件加速→按原版重启语义重开→主/独立窗真实
renderer 发生可观测变化（GPU/帧耗时），而非仅落盘一个 bool。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`Flutter3.47.5 engine/embedder锁定研究; SD-01启动设置; 主方案多窗口`。
research_status：需研究（该层无 SoftwareOnly 直接开关；Impeller 禁用 ≠ 软件渲染）。

对应 ID：FLD-CFG-062；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：SD-11 / SD-06/18。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: GUIItem.EnableHWA`；
`MainWindow.xaml.cs:151-154`（WPF 侧渲染选择）；固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `7466bee`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
UI 草稿 `option_setting_window.dart:1018-1019`、`settings_defaults.dart:104`。
SP-02 提交语义。持久化 save；生效 restart_app（`settings_timing.rs:122`）。
失败回滚语义（renderer 切换失败恢复旧模式）待 SP-26 定义，不预设。

允许修改的模块（SP-26，须 ADR 先行）：`apps/desktop/windows/runner`（engine 创建前
renderer policy）、`docs/decisions`（可行性 ADR）。若须扩展 runner/engine，
独立实现任务。本卡未改生产代码。

禁止改变的已有行为：不得把“关闭 Impeller / 选择低功耗 GPU”伪映射为软件渲染；
不得把勾选框存盘当完成；其它 GuiItem 行为不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:415-416`（`enable_hwa`，默认 false `:434`）。
- DTO：`bridge_api/src/api/settings.rs:269,284,301`。timing RestartApp（`:122`）。
- 消费者缺口 G-03：`apps/desktop/windows` 内 `hwa|HWA|impeller|software render`
  全文零命中——无 runner/engine 消费者。现有仅保存。

测试夹具和原版预期：不适用合成功能输入；可行性证据=锁定 engine 源码引用 +
最小 release 实验（真实设备/GPU/renderer 观察 + 帧耗时），见 CSV required_verification。

本次必须通过的命令/真实场景（SP-26，未运行）：engine 源码证据 + 最小 release
实验 + 所有主/独立窗真实 renderer 观察；开关与重开；失败 rollback。
若证实不可实现则标 blocked 并给出扩展路径，不假 checkbox 有效。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-062.md`。

完成条件：可行性源码证据 + 真实 renderer/设备结果；或 blocked + 扩展路径。

发现接口缺口时的处理：G-03 已登记；SP-26 先做可行性卡，不直接做开关接线。
