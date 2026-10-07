# SP-23.FLD-CFG-080 — UiItem.MacOSShowInDock

状态：identified（仅完成实例定位与存储/DTO 盘点；无任何平台 consumer，
Windows 宿主 not_applicable，macOS 效果未验证）。
任务 ID：SP-23.FLD-CFG-080（主 owner SP-23；消费者归属 SP-32/SP-33；SD-18 平台 macOS runner，关联 SD-06）。

本次唯一用户流程（macOS 专属，Windows 宿主不适用）：macOS 设置 Dock 显示→
保存→重启应用→runner/window-manager 按 activation policy 显示/隐藏 Dock；
Windows 下该字段无任何效果（不造 Windows 行为）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`macOS 实际 runner/window-manager；SD-06 policy`。

对应 ID：FLD-CFG-080；leaf；platform_scope=macos；original_type=bool；
original_default=false。
关联：SD-18（平台），SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: UIItem.MacOSShowInDock`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/ConfigItems.cs:103`
（`bool MacOSShowInDock`）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `176fd39`
（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false。
持久化 save；生效 restart_app（冻结台账；`domain/src/settings_timing.rs:305`）。
macOS release 实际 Dock 显示/隐藏 + 重启恢复；Windows 下无效果且不得有副作用。
平台写入只授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-32/SP-33，将来）：macOS runner/window-manager activation-policy
读取点（当前缺失，见缺口）。本卡未改生产代码。

禁止改变的已有行为：原版默认 false；Windows 行为零改变；其它 UiItem 不动。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain/src/settings.rs:485`（默认 `false:512`）。
- DTO：`bridge_api/src/api/settings.rs:426/:449/:479` + FRB wire
  （`frb_generated.dart:8916/12996/16286`，双向在位）。
- 调用链：无——全树 grep 无任何 activation-policy 读取/调用
  （CSV current_gap：`macOS activation-policy reader absent`）。
- 缺口：macOS consumer 完全缺失；Windows 宿主 not_applicable，macOS 实机未验证。

测试夹具和原版预期：macOS 隔离机合成开关；正向 开→Dock 显示、关→隐藏、
重启恢复；Windows 下字段值变化零可观察效果（待验）。

本次必须通过的命令/真实场景（SP-32/SP-33，未运行）：macOS release 构建→真实 Dock
显示/隐藏→重启恢复观察；Windows 下零副作用确认。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-080.md`。

完成条件：macOS 真实 Dock 效果 + 重启恢复；仅 DTO/落盘不算——故本次只记
identified。

发现接口缺口时的处理：activation-policy reader 缺失已登记为接口缺口；
归属 SP-32/SP-33，不私定模块。
