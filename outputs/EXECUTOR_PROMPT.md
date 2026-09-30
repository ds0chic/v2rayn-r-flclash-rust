**给执行模型的总提示词与首个任务**

使用方法：把本文、`V2RAYN_FLUTTER_RUST_PLAN.md` 和 `UPSTREAM_INVENTORY.md` 一起交给执行模型。先要求完成 T00 并审阅其台账，再按任务依赖逐项推进。每个执行回合范围要小，项目最终完成标准不变。本文是未来实施指令，本次编写方案没有实现应用。

**可直接复制的总提示词**

```text
你要执行一个完整的桌面客户端重构项目。

目标：用 Flutter 做前端、Rust 做应用后端，完整迁移冻结版本 v2rayN 的全部适用功能、设置、数据和操作行为。界面布局、菜单、表格、设置分组和操作习惯保持原版，在此基础上改善视觉质量与可测量性能。

技术选型纠正已经确定：Flutter，不是 FlClash。不要 fork FlClash，不要换成 Tauri/Electron/WebView，不要把所有配置转成 Mihomo YAML。

必读附件：
1. V2RAYN_FLUTTER_RUST_PLAN.md：完整规格和任务依赖。
2. UPSTREAM_INVENTORY.md：固定版本源码定位、设置与命令索引；只是盘点种子，不是已验证功能表。

功能基线：
- 官方仓库 https://github.com/2dust/v2rayN
- tag 7.25.4，预发布版。
- commit 7d6a967c18c697f28dc6917122ed3a4993fcf336。
- 稳定版7.24.9作为迁移回归样本。
- Windows以WPF版界面为默认视觉/交互基准，保留Horizontal/Vertical/Tab三布局。
- Windows先完成，其他承诺平台分别验收；Flutter不覆盖的上游CPU架构必须明确记录，不能假装支持。

核心规则：
1. 先建立 features/fields/actions/layouts 四份可追溯台账，再按台账做实现。
2. Flutter只拥有界面与草稿；Rust拥有领域和业务持久化；net-host拥有真实运行状态；helper只执行必要权限操作。
3. desired_revision与applied_runtime_revision分离。保存成功不等于内核应用成功。
4. 保留Xray、sing-box及基线支持的外部内核，不从零重写代理协议。代理流量不能走界面桥。
5. Xray、sing-box独立生成配置；自定义内核按上游真实支持深度接入。
6. 不漏protocol/transport extra、订阅Headers/MoreUrl/前后置节点、完整模板、Mixin、策略组和代理链。
7. 普通数据与未知字段/原始配置分离保存。只保留数据但不能运行，状态必须是preserved_only。
8. 不更改原快捷键含义、不删高级设置、不把表格改成卡片、不用统一JSON框代替所有原版表单。
9. 运行操作串行收敛，有真实探活、取消、失败补偿与系统代理所有权恢复。
10. 性能数字是目标，必须在相同core/config下测量，不能以“Rust更快”替代证据。

执行方式：
- 从T00开始，按依赖顺序处理。
- 每回合先读当前任务、相关台账、上游实现及已有代码，不凭记忆补功能。
- 一张小任务卡只处理一条用户流程和一至三个功能ID。
- 共享契约变化先更新契约和调用方计划，不能各写一套状态/数据库/进程管理。
- 每个功能走完UI→Rust→持久化→重开→配置/平台效果→测试证据。
- mock只用于隔离和故障注入；真实闭环必须用真实后端与内核。
- 用原版代码/配置/操作结果做差分验证。不能只跑编译就把功能标记verified。
- 修改权限和网络设置的系统测试在隔离的测试环境执行，并准备恢复。
- 不接触或发送用户秘密；测试采用合成或脱敏数据。
- 遵守项目AGENTS.md。Oracle建议需本地验证；未经明确同意不启动Oracle API付费模式。

禁止：
- 用占位页面、空回调、示例节点、伪进度、全mock后端冒充功能完成。
- 静默忽略字段、偷偷切换内核、删除未实现项、把困难功能改成不适用。
- 只实现常用功能就称完整移植。
- 把未运行的测试写成通过，把未实测的平台写成支持。
- 为使测试通过而降低冻结预算、更新golden掩盖回归或缩小台账分母。

每次结束必须输出：
1. 本次完成的任务/功能/字段/动作/布局ID。
2. 具体改动与原版兼容依据。
3. 实际运行的验证和证据路径。
4. 未完成、仅保留、未验证和阻塞项。
5. 下一张任务卡及其前置是否满足。

最终发布必须通过方案规定的全量门禁，不能仅凭口头总结宣布完成。
```

**首次执行：T00 任务卡**

```text
任务：T00 / 建立可执行基线与完整台账。
本次唯一目标：让任何后续模型都能明确知道每一个原版功能来自哪里、怎么实现才算完成。
本次不进入应用业务实现。

读取上游固定commit中的：
- v2rayN/v2rayN/Views及代码后置（Windows WPF）
- v2rayN/v2rayN.Desktop/Views及代码后置（Avalonia平台差异）
- v2rayN/ServiceLib/ViewModels
- v2rayN/ServiceLib/Models/Configs/Config.cs、ConfigItems.cs
- v2rayN/ServiceLib/Models/Entities
- v2rayN/ServiceLib/Enums
- v2rayN/ServiceLib/Handler和Manager
- v2rayN/ServiceLib/Services/CoreConfig
- 翻译、模板、平台集成和现有测试

需创建的项目资料：
1. compat/upstream-lock.json：仓库、tag、完整commit、发布日期、发行包/core来源/版本/哈希。
2. compat/features.yaml：所有适用功能与平台/内核范围。
3. compat/fields.yaml：设置、持久化及协议扩展字段。
4. compat/actions.yaml：菜单、快捷键、键鼠、托盘、定时任务和取消行为。
5. compat/layouts.yaml：窗口、三布局、菜单和表格结构、DPI截图。
6. compat/platform-matrix.md：WPF/Avalonia及Flutter平台差异。
7. fixtures/source/：不含用户秘密的原版样本与参考结果。
8. docs/tasks/T01.md：下一步技术可行性任务。
9. docs/evidence/T00.md：实际读取范围、采集环境、证据及未核实项。

实施要求：
- 先验证本地checkout对应完整commit，不自动改用master。
- 不启动上游仓库的未知自动化脚本；运行基准应用须使用隔离数据目录/测试环境。
- 源码扫描加运行行为核对，不只看README和可见设置页。
- 自动提取保留源符号；随后识别注释、废弃字段、动态绑定和平台条件。
- 字段有效默认值需要追踪加载/迁移/界面初始化，不能只抄C#属性initializer。
- 当前环境不能采集的原生界面或平台，准确登记unverified并设计可执行采集步骤。

完成条件：
- 每个识别项有稳定ID、原符号、适用条件、预期行为和验收方法。
- UI字段与业务字段能双向追踪；后台功能也在台账中。
- 15配置类型、多内核支持深度、三布局、全部ConfigItems类均有条目或有证据的内部/废弃分类。
- 明确guiNConfig.json + guiNDB.db + 外部文件及V2/V3/V4迁移路径。
- 明确上游与Flutter架构支持差异。
- 所有不能核实的事实显式登记；不得填“默认应该如此”。

本回合交付台账和证据，随后按总方案推进T01与T02。
```

**后续每回合的简短继续提示词**

```text
读取项目总方案、四份台账、上一回合证据及未完成任务。
选择下一张前置已满足的小任务卡，按原版源码实现并验证一条完整流程。
保留全部兼容约束，不扩大本次范围，不降低最终要求。
完成后更新台账与证据，并按固定五项格式汇报。
如果发现上游遗漏，追加条目；如果发现契约矛盾，先给出具体差异与最小修正，不能自行删功能。
```
