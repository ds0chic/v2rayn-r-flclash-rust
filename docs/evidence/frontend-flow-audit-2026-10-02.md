# 前端全流程契约补充与静态审阅证据

状态：`identified`。文件名沿用本轮交付批次，源研究在其各自报告标记实际基线。本轮交付完成的是规格与静态审阅，业务实现、真实UI、平台、性能均未验证。

用户要求：固定每个前端元素的操作、反馈和使用逻辑；按照真实用户顺序审阅整体链路，覆盖切页、返回、取消、失败与重开，避免仅看单按钮。延续先前“只给详细方案”的范围，本轮未改应用实现。

## 产品与基线

- 主规格是最近的SteamBox四模块：网络加速、账号切换、库存游戏、挂卡。Authenticator、ASF、GameTools排除；必要共享身份/会话/native/网络支撑保留。
- SteamBox本地源码`C:/Users/Colby/.gemini/antigravity/scratch/SteamTools`，HEAD `a7a5ce7f0830b9b759090a75b59d82679084b478`，读取工作树。正式冻结仍需按S00处理dirty文件与子模块；本轮不将HEAD假装完整快照。
- v2rayN当前checkout `d7fbe80`，冻结上游7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；另作独立流程补充，不混产品台账与设置语义。
- 读根AGENTS、对应只读方案与索引、实际任务卡和相关源定位。本轮写入仅新增文档及枚举工具，不改`outputs/work/compat`既有条目。

## 交付清单

| 文件 | 目的 |
|---|---|
| `docs/decisions/FRONTEND_AUDIT_HANDOFF.md` | 执行入口、阅读顺序、可复制模型提示和整条路径实施顺序 |
| `docs/decisions/FRONTEND_INTERACTION_CONTRACT_2026-10-02.md` | P00–P25、权威状态、反馈模板、request去重、draft revision、焦点/选择/返回/取消、性能目标 |
| `docs/decisions/STEAMBOX_FRONTEND_FLOW_SPEC.md` | 四模块与公共支撑动作、冲突矩阵、SB-J01–17完整路径 |
| `docs/decisions/STEAMBOX_FRONTEND_FIELD_POLICIES.md` | 101顶层种子逐字段提交/校验/效果/恢复；非UI/无消费者路线单列 |
| `docs/decisions/STEAMBOX_FRONTEND_ACTION_POLICIES.md` | 确切命令/事件、动态/代码注册入口、稳定动作ID、有限deadline/取消/效果合同 |
| `docs/decisions/steambox-frontend-elements-2026-10-02.csv` | AXAML声明节点稳定ID、来源/行/selector、命令/事件/绑定/条件、规则/字段/上下文/反馈/流程定位 |
| `tools/audit_steambox_frontend_elements.ps1` | 只读源码XML枚举，可重复输出；不启动app，不读用户配置 |
| `docs/decisions/V2RAYN_FRONTEND_FLOW_SPEC.md` | 当前产品独立元素规则和VN-J01–16，结合原compat动作/字段/布局 |
| `docs/evidence/steambox-workflow-research-2026-10-02.md` | 46家族、15静态状态断点、14研究路径，源码与建议分开 |
| `docs/evidence/v2rayn-workflow-crosscheck-2026-10-02.md` | 十项新增跨流程断点，与已有UX-001–009串联 |
| `docs/evidence/interaction-contract-second-review-2026-10-02.md` | 独立复核规则、门禁与两产品边界 |
| `docs/tasks/FE-SPEC-01.md` | 本次规格审阅任务卡，不改变既有业务分母 |

既有`docs/evidence/frontend-behavior-review-2026-10-02.md`及已存在的测试/截图/时间线未由本轮修改，也不借用其局部结果宣称新流程已通过。

## 主要静态断点与修正合同

| 连贯流程 | 静态发现 | 固定的重构目标 |
|---|---|---|
| 双击切账号→转库→挂卡 | 双击XAML与codebehind均有入口，切换成功缺真实SID核对，Web/native SID不符仍继续 | 单request/operation；配置应用/客户端启动/身份确认分别反馈；未同身份拒绝挂卡 |
| 库编辑→选图→取消→返回→再开 | 编辑共享App引用，cancel未撤文本/启动项，部分reset直接写Steam图 | 深拷贝draft/overlay；暂存图片；cancel零写；保存overlay与应用Steam两阶段 |
| 成就修改→保存失败→重试 | StoreStats失败转ResetAllStats路径；先改显示状态 | draft保留，失败/未知核对；只有用户明确重置才入重置确认 |
| 选下载→完成→倒计时→取消 | 通知30秒后执行但读到的方法直接电源调用，非下载可能被当完成 | 完整真实完成判据，30秒真实可取消计划，到期重新核实；断网/半写数据不触发 |
| A挂卡/Cloud→切B→旧callback返回 | AppId-only进程字典跨功能共享；旧账号结果可能写新投影 | worker owner/role/account epoch；仅own句柄清理；旧写journal结算但不污染B |
| 社区启动→接入→监听失败 | System/PAC写入在listener启动之前 | listener ready再接入；恢复结果持久可查，不toast后伪停止 |
| SDK测速→换game/area→旧回调 | 回调写CurrentAcceleratorGame，缺目标核对 | vendor epoch+operation+game/area/server拒绝过时投影 |
| G2导入/当前组更新→刷新→重开 | 添加/导入归属与当前组不一致，更新从另一窗口选中取目标 | 一组权威ID驱动所有入口，追加/替换分清，主投影与数据库同步 |
| 节点/设置保存→立即应用 | desired已增长而UI仍用旧snapshot revision | 保存回执交接新revision；旧回执不清新draft；saved/applied分层 |
| 设置测速→过滤→测试→停止→返回 | 常量覆盖设置，空ID扩大全库，job覆盖/取消未确认，返回过滤隐藏 | 精确参数和目标集合，一个可定位job，真实取消确认，输入/过滤恢复一致 |

以上来自源符号审阅，实际症状频率未测；每条对应研究报告的详细行号与合成复现。纠正缺陷不意味着任意改变原布局/菜单或恢复注释入口。模式选择保留原运行中禁用，脚本编辑保留外部编辑器+原确认入口，成就全选保留原未保护source范围并显示数量。

## 本轮检查与实际结果

1. 实际运行`./tools/audit_steambox_frontend_elements.ps1`并重复生成。XML reader禁DTD、resolver为null；输出绝对路径限制在本repo `docs/`。第一次原型遇root无Parent已修正，之后枚举成功；未运行原GUI。
2. 枚举2684声明节点：network907、accounts215、library612、idle257、shell_settings693。包含布局、属性、模板，不能叫2684个生产按钮。
3. 分类2295保留源码视图、143公共候选需route核实、246非当前/调试来源。全部runtime_verification为“未运行”；状态identified/not_applicable，候选不自动恢复入口。
4. 结构检查：2684唯一稳定ID；P00–25合法；属性声明不误分成可交互控件；未包含三删除插件源路径；无业务verified；相同输入重复输出SHA相同。
5. 源码视图带command/handler的168节点用于动作政策映射；代码注册/动态/native入口单独补，不把XML范围充当所有运行时可达性。
6. 文档检查：SteamBox17条和v2rayN16条流程ID唯一连续；补充文档代码围栏成对；逐字段/动作关联结果由最终检查追加在本节末。
7. 独立复核提出六处歧义，已修：提交≠接受；后端request去重/回执重查；保存中继续编辑revision；旧账号已提交操作继续结算；运行模式禁用/不加布局切换；每action有限deadline/取消提交点/补偿/unknown核对。另修P09按源on_change/on_submit，不强迫原提交式搜索即时查询。
8. `git diff --name-only -- outputs work compat`无输出。这里只记录tracked差异检查及检查时hash，不虚构本轮开始前不存在的hash对比。没有本轮app/Rust/service源码修改。

最终关联检查（实际执行PowerShell枚举与断言）：

| 检查项 | 实际结果 |
|---|---|
| 原种子ID+字段名 vs逐字段政策 | 101/101一致，差异0、重复0 |
| 字段入口来源分类 | E67 / N21 / Q13；分类不是实际运行通过 |
| 稳定动作合同 | SB-A-001–111，共111；前105含当前/补充合同，后6为候选/非适用边界 |
| 保留视图有命令/事件的源码节点 | 168；129含命令、40含事件、1同时含两者 |
| CSV到动作政策 | 168/168已关联，未解析符号0；通用About Command由运行时item kind分发007/008/009 |
| CSV到顶层字段政策 | 直接静态绑定50个种子字段；其余由VM/内部/生成入口定位，不谎称缺少迁移项 |
| 计算绑定 | ProxySettings.ProxyModes是候选列表，不误计为第102个持久字段 |
| 完整流程 | SB-J17条、VN-J16条，ID唯一连续；都是未来验收轨迹 |
| 最终CSV SHA-256 | `A851A8FD872CF65D107E19F08B4A1247477234225F5DAD1E0704B2FB7C3DEF44` |

逐字段源审阅另外定位六个活跃链缺口：最小化选项没有启动消费、选定Steam路径不进入启动服务、自动检查脚本开关不控制自动检查、Cloud筛选没有保存、语言culture Key写入与显示名读回不一致、自定义主题色RadioButton仅OneWay。字段表规定目标，不把源已有控件当效果已实现。

最后动作复核明确成就重置第二步的原Cancel枚举其实代表“仅统计”；新UI使用“统计与成就/仅统计”范围选项，Esc/X/真正取消均零重置，避免照搬别扭且危险的取消语义。未测试原弹窗。

动作政策作者在文档与静态符号检查已保存后遇到代理额度限制，后续终稿核对由主代理接手；没有将未完成的代理终稿声称独立复核通过。主代理核对映射、字段种子、状态/范围一致性并修正文案/取消边界；真实产品测试仍未运行。

Rust/Flutter format/analyze/build/test、真实窗口操作、代理/Steam/SDK/native/platform、Profile/Release性能测试：**本轮未运行**。脚本/表结构通过不代表用户流程verified，也不保证已无卡顿。

## 原件只读检查时SHA-256

| 原件 | SHA-256 |
|---|---|
| STEAMBOX_FLUTTER_RUST_PLAN.md | `1D253C1F62D9DFDF06A2A72C310B41F99633C8C637B2075B2A840C5803CB56F8` |
| STEAMBOX_EXECUTOR_PROMPT.md | `3E9F04C5A218D26BF49F2BE64474458B657C7786997DF6A8967A26AD0DA73134` |
| STEAMBOX_SOURCE_INVENTORY.md | `8FE78EF13505D587E1DA9B131408E8CD935A810F13E8AD4436CCBA1B99561594` |
| V2RAYN_FLUTTER_RUST_PLAN.md | `C06C01C1FD6E4BA5F433403ED4D2053E6417C73EC51DA7DB52D26FD1E59A5178` |

## 剩余前置和实际边界

- SteamBox生产route、候选/动态/native元素、四模块嵌套复杂字段继续冻结追加；101只是当前顶层种子，不是最终全部设置分母。
- 每条路径在真实UI→Rust→存储→重开→效果链完成，并跑重复/故障/取消/交错变体。未做的平台项不降低为not_applicable。
- 验证冷/热启动、60/120Hz、10k数据并发日志/扫描/测速或挂卡下的输入至反馈、UI/raster慢帧和相关进程资源，目标见公共契约；平均FPS/截图不足。
- 系统接入、电源、客户端切换等只在相应批准隔离环境测试，本轮不触10808、不修改宿主代理、不停止非自有进程、不读取/发送秘密。
- Oracle skill未在当前可用技能中找到，本轮未调用Oracle或启动API付费运行；采用协作代理独立只读复核并对照源定位，建议已由主代理核对/修订。该复核不替代本地真实测试。

下一执行回合先读`FRONTEND_AUDIT_HANDOFF.md`，确认产品/冻结前置，一回合从一条完整路径推进并留证据；不能从逐按钮占位开始。
