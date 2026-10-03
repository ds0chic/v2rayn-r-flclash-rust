# PARITY-AUDIT-01 — 多代理核对全部适用功能与原版行为

状态：`verified`（仅审查交付）。本任务是审查与修复规划，不修改应用实现，不把台账数量当产品完成度。

任务 ID：PARITY-AUDIT-01

本次唯一用户流程：用户用冻结原版完成一条正常操作，再用当前重构版走同一入口、对象、提交、取消、返回、重开和实际效果链路，拿到逐项差异及可执行的修复顺序；此审查流程逐项覆盖全部台账。

前置任务及已验证证据：冻结v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用HEAD `1251cbc6821276e35d082f7739b8b9c15b6f93dd`，dist/SHA256SUMS和build-info.json已有未提交改动。本轮不得改动这些文件。旧审查为线索，必须重新核对当前代码。

对应 feature / field / action / layout ID：`docs/evidence/parity-review-2026-10-03/inventory.json` 的全部行；每行有唯一key和负责人。功能110、字段338、动作185外，还检查协议/格式、内核矩阵、后台调度与布局/窗口。新发现的台账遗漏另列追加建议，不删旧项。

必读上游文件、符号和固定 commit：AGENTS.md、方案及§19、T01、相关UX任务卡、四份台账、upstream-lock、platform-matrix、分配文件中的source/evidence；实际源码在work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967。Windows以WPF为1:1依据，其他平台以Avalonia；不得互套。

输入、输出、错误、取消、权限、持久化及生效语义：输入为公开冻结源码/合成夹具与当前源码；输出逐项JSON、领域报告、根汇总和修复队列。对每项追UI→bridge→Rust→持久化→重开→配置/平台效果。缺入口、占位、仅存储未生效、对象/提交/取消语义差异分开登记。未运行写未运行，未实测写未验证；source一致仅说明代码一致。禁止读取个人配置/凭据，禁止10808与宿主代理/自启/TUN写入；不停止外部进程。原生UI无法操作和原生崩溃须保留证据，不冒充功能通过。

允许修改的模块：本任务卡、`tools/audit_parity_inventory.py`、`tools/audit_parity_root.py`、`tools/audit_parity_consolidate.py`、`docs/evidence/parity-review-2026-10-03/`；必要的独立合成测试须先与根代理协调。本轮根代理协调新增 `apps/desktop/integration_test/parity_review_smoke_test.dart` 与纯 Rust `crates/subscriptions/tests/parity_original_inner.rs`，不改生产实现。3个子代理只写自己的report/items文件，不改应用、compat、work、outputs、dist，不commit。Flutter/Windows集成构建由根代理串行执行，避免共同生成产物互相污染。

禁止改变的已有行为：原版适用功能、字段/动作/菜单/布局分母和平台合同；不得依台账的implemented/verified标签直接判定对齐；不得用删除入口或限制协议当修复建议。

测试夹具和原版预期：公开冻结模板与合成地址/UUID；有实际效果的测试需明确端口≥11808且先探测，禁止真实用户流量。原版源依据、当前源证据、测试/真实窗口证据分列。

本次必须通过的命令/真实场景：库存脚本无丢项/重复；逐行报告与分配key集合相等；领域高优先差异由根代理复核实际文件/符号；安全可运行的针对性测试记录结果。UI真实场景由根协调，不能把所有并行source审查称为人工逐功能验收。

证据文件位置：`docs/evidence/parity-review-2026-10-03/`。每代理 `<owner>-items.json` 与 `<owner>-report.md`；JSON每行至少key、id、kind、status（六种允许值之一）、upstream_expected、current_behavior、difference、upstream_refs、current_refs、evidence_level、tests_run、limitations、next_action；具体缺陷注明优先级与用户触发步骤。入口见同目录 `README.md`，总表见 `all-items.csv/json`，覆盖校验见 `coverage.json`，修复次序见 `repair-queue.md`。

完成条件：所有分配行有可核对结论，未验证明确列出；三个领域报告及跨领域一致性汇总完成，发现项不只重复旧报告；给出按用户流程排序的修复队列。审查完成不等于迁移完成，不给虚构整体百分比。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

本轮实际结果：258+373+107+62=800行严格与分配集合相等，缺漏0、重复0、引用警告0。应用 HEAD 仍为 `1251cbc`。第四轮 Windows 真窗口测试在合成节点归属有效组且主窗口选中该组的前提下完整运行，六个原版合同检查失败，`ui-run-04/observations.json` 的 `recordingComplete=true`。原版 InnerFmt 2条回归显式 `-- --ignored` 运行失败；默认 subscriptions 测试97通过、2 ignored；菜单模型7条与领域后端合成161条通过（测试有重叠，不相加）。第三轮 Flutter Windows 引擎发生原生崩溃，部分输出不计为功能结论。没有原版实机双窗口、macOS/Linux/ARM64 或受控平台写入验证；审查完成不等于迁移完成。
