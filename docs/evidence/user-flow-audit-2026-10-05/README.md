# 用户实际流程专项检查，2026-10-05

本轮响应：用户报告两个启动入口都无效、操作延迟高、底部可选项与只读文字混杂，要求完整移植修复方案。审查HEAD为77c74ed（完整hash见各报告）；原版冻结7d6a967 / v2rayN 7.25.4。

本轮没有保留生产源码改动，没有发布/安装新版本，没有执行宿主网络/自启操作，没有读取真实节点或订阅。三个子代理的短暂节点补丁已反向撤回并核对内容hash；测试临时副本仅用于执行后删除。git status可能显示该文件stat/EOL标记，但git diff为空且blob与HEAD一致。

| 证据 | 方法 | 实际结论 | 不能推导的结论 |
|---|---|---|---|
| startup-contract-repro.dart/.log | RuntimeController真实实现+CountingRuntimeBridge错误注入 | 3项正常断言失败：重试/F5没重发、pending不busy | 没有真实FRB/核/发布包验证 |
| profiles-ctrl-timing-repro.dart/.log | 真实ProfilesTable+Synthetic，正常pointer/key输入 | 快速松Ctrl后A+B变仅B，断言失败 | 50/400ms是测试虚拟时间，非实机时延采样 |
| profiles-enter-timing-repro.dart/.log | 正常MainShell UI+Synthetic/fake runtime | clickB后50ms Enter仍操作旧A，断言失败 | 不证明内核真实启动或网络性能 |
| dns-baseline-collision-repro.dart/.log | 真实DNS widget+合成桥/平台/运行 | direct改为remote旧值后刷新丢草稿，断言失败 | 没有原生窗口/真实DNS请求验证 |
| baseline-and-ledgers.json | YAML主表清点 | 110功能、185动作、180设置、158实体、36布局；旧主表无verified | 不等于所有代码没实现，也不能算完工百分比 |
| profiles-audit.md | 源码与冻结WPF对照+上述选择测试 | 14项发现/21流程，原版边界逐项校正 | 不是全领域实机通过 |
| runtime-audit.md | 运行/IPC/发布包只读检查 | 12运行缺口、哈希/包组成正确但不保证流程 | 不把旧已修项目重新说未实现 |
| settings-audit.md | 设置/路由/DNS/窗口/托盘对照、现有测试与DNS复现 | 17问题与6待验边界；DNS现有7测通过仍漏草稿缺陷 | 旧fake绿测不证明原生窗口/OS效果 |

六个新增修复契约均正常执行并失败，证明当前对应行为不满足修复合同；失败不是flutter_tester崩溃。领域报告中另外记录了原有窗口测试的批跑未完成/单独复跑结果，不能把那部分混写成整批通过。

computer-use初始化报 kernel assets/path错误，本轮没有当前正式包原生UI操作。查看的主窗图像来自历史R3-WPF-COMPARE/R3-VISUAL-DPI-TRAY归档；仅辅助布局诊断，不是当前commit新截图。

根复现执行步骤：将startup-contract-repro.dart临时复制为apps/desktop/test/_user_flow_audit_repro_test.dart，应用目录执行flutter test test/_user_flow_audit_repro_test.dart -r expanded，exit1/3 failed；之后删除该精确副本。其余harness命令与结果在各领域报告/log中，均无真实网络/平台调用。

本轮成果： [完整修复方案](../../repair/V2RAYN_COMPLETE_PORT_REPAIR_PLAN_2026-10-05.md)、[实施模型开始说明](../../repair/START_HERE.md)、[执行卡索引](../../repair/tasks/README.md)、[库存所有者覆盖](../../repair/coverage.csv)。后续修复证据另存docs/evidence/repair/<任务ID>/，不要将本目录诊断标成修复成功。
