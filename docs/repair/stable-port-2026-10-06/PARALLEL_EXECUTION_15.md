# 10～15 子代理并行安排（2026-10-07）

用户允许10个甚至15个子代理。本文件调整执行安排，不启动生产修复，也不声称已经启动15个代理。调度目标为1个主控+10个主要子任务；资源足够时加5个补充子任务。当前Codex会话工具限制为共4个并发槽（主控占1、子代理最多3），不能用工具突破；在支持15并发的OpenCode等执行器按完整表运行，受限执行器按同一任务池分批运行。

## 1. 当前进度与调度原则

本次读取HEAD：`8da14529eecd7f8de54bb623c26a7f6eb984472b`，工作区还有正在进行的未提交实现。当前manifest标implemented的是SP-00～06、SP-11、SP-14、SP-23、SP-29；implemented不等于正式验收verified，SP-23登记通过不等于180字段全部有效。开始前再读最新manifest、证据和diff，保留已有成果，不派15个代理重做这些任务。

主控先做短合同协调：明确本批要用的函数、DTO、错误、版本和归属；固定可共享的接口后各代理独立写专属模块。完整卡仍按manifest前置验收；前置未完成时可做独立源码对照、夹具和模块准备，不伪造生产成功返回，也不提前宣称整卡implemented/verified。

## 2. 十个主要子任务

每个子代理一次领取一个小流程，不把表中整条职责一次标完成。

| 代理 | 职责与顺序 | 专属修改范围 | 当前立即可做/等待 |
|---|---|---|---|
| A01 事件与重连 | SP-07，控制事件lag、断连、snapshot对账 | net_host events.rs/server.rs；application net_host_client.rs | 复核SP-05/06合同后实现；IPC/DTO修改交主控 |
| A02 helper归属与清理 | SP-08 helper资源journal、失败保留、逐资源释放 | privileged_helper backend.rs/server.rs/windows.rs及专属journal模块 | 可立即做正确失败合同和helper实现；host接线由A03 |
| A03 host租约与TUN | SP-08 host端→SP-09保活→SP-10执行/探活 | net_host helper_client.rs/tun_lease.rs/session.rs/lifecycle.rs；application tun_plan.rs | 先固定A02接口再接清理；保活/提权按真实前置推进 |
| A04 保存与失败重试 | SP-12，native结果/newRevision/epoch/分阶段生效 | features/settings保存controller/actions/host；application settings.rs | SP-11已implemented，先复核再接；平台写入结果交A12或阶段重派 |
| A05 路由与DNS | SP-13及对应字段实例，读失败/增量提交/取消/重开 | features/routing；application routing.rs/dns.rs | 先核原版、建故障夹具；等A04共享receipt再完整接线 |
| A06 组订阅与节点键鼠 | SP-16→SP-18，当前组/编辑入口/右键与选择 | profiles_page/table/context_menu/command_context/actions；features/subs；application selection.rs/groups.rs | 原版键鼠对照可先做；组controller变更交A07单写，等A04版本合同 |
| A07 节点异步数据链 | SP-21→SP-22节点部分，后台分页/过滤/overlay | application store_repo.rs及专属查询模块；profiles_controller.dart | 查询合同/独立数据层可准备；接A06组ID合同后验完整流程 |
| A08 主窗与子窗呈现 | SP-17→SP-19，actual摘要/消息/底栏/主题/间距 | status_bar/ui shell/runtime_bridge/runtime_controller；option_setting_window_entry；共享样式 | 原版布局/样式可独立准备；actual接口等A01/A04；native窗口协议仍归主控 |
| A09 配置消费者 | SP-24和SP-23逐字段，Happy/Fragment/Mihomo/SRS等 | config_codegen；application codegen.rs/custom.rs/mixin.rs/templates.rs | 先原版参数化合同；最终UI保存/生效链等A04。domain类型改动交主控 |
| A10 应用HTTPS与更新 | SP-25→SP-27，信任源/当次flags/验签/安装恢复 | 低层HTTP模块；subscriptions网络消费者；updater/upgrade_runner；application update_service.rs/webdav.rs | 受控TLS和更新故障合同可先做；正式更新等真实发行源/key及A04/A12 |

A04只拥有设置表单/提交文件，A08只拥有独立入口与主题样式文件，不同时改option_setting_window.dart。A06不改profiles_controller.dart，组读写需求由A07接入。A02不改net_host session，A03不改helper backend。A09不改update/network模块，A10不改core codegen。文件范围不是无限授权重写，任务卡约束仍适用。

## 3. 扩展到十五个的补充任务

| 代理 | 职责 | 专属范围/阶段 |
|---|---|---|
| A11 连接与监控界面 | SP-20及SP-22连接/日志部分 | features/monitor与application monitor.rs；先原版列/右键/会话generation合同，再接A01/A08真实endpoint |
| A12 平台实际生效 | SP-15、SP-32及平台字段实例 | crates/platform与application platform_service.rs；先内容hash/desired-actual/故障合同，真实OS写入仅授权隔离机 |
| A13 普通用户验收 | SP-30按一个流程核对；独立找缺入口和误导反馈 | 读生产源码，写独立synthetic夹具/证据；不直接修改其它代理的实现。候选包就绪后占用GUI验收槽 |
| A14 性能与renderer | SP-26可行性研究，SP-31性能采样准备/验收 | benchmarks、专属测量工具和证据；先读runner/engine，不与A08抢改native。实际runner patch另分卡，主控落实owner |
| A15 核心/协议/字段覆盖核对 | SP-28逐核/协议；180字段与158实体的遗漏核对 | 只读codegen和实现，写专属fixtures/证据；不与A09抢改生成器；候选ready后使用独立数据/端口作真实会话 |

十并发模式下，这五类先入待领取队列，主控承担接口/用户流程抽查，主要代理交付后转岗；不是删除连接表、平台、性能或核心验收。十五并发模式可立即展开源码核对/夹具/研究，尚无候选时不反复跑占位测试。

## 4. 谁写共享文件

主控保留共享接口/注册入口写权：`crates/application/src/engine.rs`、application/domain的lib.rs导出、IPC/lib/helper/stable DTO、bridge_api公共API、BridgePort、FRB声明与生成物、workspace Cargo与依赖锁。需要修改时代理提交精确签名和最小补丁建议，主控及时接线，避免十余人同时改一份大文件。

`session.rs`租给A03、`settings_controller.dart`租给A04、`profiles_controller.dart`租给A07、`runtime_controller.dart`租给A08。主控整合这些文件时先交还写锁再合并。native C++协议host归主控；A08/A14提出独立UI/renderer需求，由主控指定一次一个实现者。A12独占platform文件，A03所需只读平台probe由它提供。

提供方先交编译可用接口和关键合同，不等整个领域做完才交一份巨型补丁。接口不足登记provider/caller/参数/error/revision/epoch/cancel/存储/生效点，主控解决。mock只作故障注入，不能拿占位成功打通生产流程。

## 5. 第一批应优先完成什么

先集中完成A01的SP-07和A04的SP-12；A06随后补SP-16，这三处能释放底栏、路由、组/右键、分页、连接和设置消费者的接线。

同时A02推进cleanup journal，A03准备host释放/续租接口，A07准备真实DB异步查询，A08做原版底栏/子窗呈现对照，A09/A10完成本领域纯正确合同。扩展代理先补GUI/字段差距核对、性能测量方法和HWA可行性，不空等前置。

一项交付就进入小批整合：主控核对共享接口→接调用方→相关编译/行为检查→当前真实流程。其它独立代理继续工作，不停全队等待一轮全量。发现实现缺陷返回原owner，审计代理不偷偷重写其它人的模块。

## 6. 十五个代理不等于十五套全量测试

遵循VALIDATION_POLICY：每人只跑自己的定向检查，整合者汇总跨域，发布候选完整门禁。全量290项、FRB二次生成、release构建不由15人分别各跑一次。

维护独立的构建/实测资源队列：先设置1个重型release构建槽，GUI和性能分别有独占测量槽；轻量测试可在资源允许时并行。根据CPU/内存/锁等待调整构建并发，不让多人写同一build/target目录、占用DLL或把编译负载混入性能数字。测试进程异常优先定向定位，不整队重跑。

所有真实会话使用每代理独立数据目录、pipe/session身份和预探测端口（≥11808），严禁10808及宿主代理/TUN/路由/Run-key副作用。只有持有本项目句柄/身份的进程能停止。A12/13/15不得同时操作同一个GUI数据目录或OS副作用环境。

执行器支持隔离worktree时，先由主控确认包含当前已做但未提交成果的共享基线，再分离；不能15个worktree都从旧HEAD起步丢掉当前修复。各代理不回写其它工作树、不提交秘密。若共享工作区则严格执行文件写锁，协调测试资源。两种方式都由主控统一生成桥和打候选包。

## 7. 调度输出与验收边界

代理每次汇报：已交付的唯一流程、改动文件、实际定向命令/结果、接口需求、待确认事实、下一依赖。卡状态按证据，implemented不等于verified。已有测试覆盖可复用，不每代理每卡新建重复测试。

轻重任务动态转岗，不按固定小时时间硬切；优先运行/数据安全阻断及能释放其它消费者的接口。当前任务池、独占文件及等待provider在parallel-assignments.json；该文件是调度方案，不是已经启动的进程列表，不取代实时manifest。

先把Windows x64普通候选真正修到可用，再安排对应长期与平台验收；六平台完整目标仍保留。任何总体完成宣称仍需实际用户链路和适用库存证据，子代理人数不作为完成度。
