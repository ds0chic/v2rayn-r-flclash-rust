# SP-23.FLD-CFG-131 — ClashUIItem.ProxiesSorting

状态：implemented（实例登记完成；正式入口/排序生效验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-131（主 owner SP-23；消费者归属 SP-17）。

本次唯一用户流程：Clash 代理页改排序→保存→重开→代理页初始排序即该值；
保存失败→可见回滚旧排序（非静默停在未持久值）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-04; native monitor端点与真实session; timer lifecycle`。

对应 ID：FLD-CFG-131；leaf；platform_scope=all；original_type=int；original_default=0。
关联：FLD-CFG-132/134（同页刷新）、SD-13/SD-14。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: ClashUIItem.ProxiesSorting`；
只读核对原版代理排序/查询语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `393fafd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法排序枚举 int/
非法值（拒绝）；持久化 save（经 saveGroup）；生效 immediate
（`settings_timing.rs:48`）。异步保存失败→回滚展示并可见（当前缺口）。

允许修改的模块（SP-17）：`proxies_view.dart:33-34,58-59`
（`_sorting = config.proxiesSorting` 初始播种）、`clash_ui_config.dart:17,29,67`
（`ProxiesSorting` 解析）、`bridge_api` ClashUI DTO 段。
本卡未改生产代码。

禁止改变的已有行为：core 切换/失败可诊断语义不动；暂停/取消定时器生命周期不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:661,681`（`proxies_sorting: i32`，默认 0）。
- DTO：`bridge_api` ClashUI DTO（`proxiesSorting`；FRB `frb_generated.dart:6378,9476-9488`）+
  Dart 侧 `clash_ui_config.dart` 解析。
- UI：`proxies_view` 从 config 播种（存在）；codegen 仅 Mixin 投影。
- 缺口：本地选项在 saveGroup 前已改；持久化失败不展示/不回滚（CSV current_gap）。

测试夹具和原版预期：合成代理列表；正向 改排序→落盘→重开初始一致；
负向 保存失败→回滚可见、非法值拒绝。

本次必须通过的命令/真实场景（SP-17，未运行）：正式代理页改排序→FRB→保存→
独立重开→初始排序一致 + 真实 core 查询顺序。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-131.md`。

完成条件：canonical→UI 初始播种 + 失败回滚 + 重开；仅本地变化不算。

发现接口缺口时的处理：失败回滚缺失已登记；归属 SP-17。
