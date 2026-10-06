# 接口、领域子包与整合卡映射

状态identified。本表防止分册、SP卡和调用方各自创造接口。签名仅是拟合同；现成API明确标现有。SP-00核验现存签名与可编译类型后，统一修改bridge/IPC并补协议文档。

## 1. 对外合同与错误语义

| 合同/API | 当前或拟新增 | 提供方→调用方 | 必须带的事实/错误/取消 | 归属 |
|---|---|---|---|---|
| submitRuntimeIntent | 拟扩展现有apply/stop入口 | application→bridge/runtime UI；host负责admission | Rust入口在发送前登记operationId/commandId，冻结target/hash/revision；receipt不是Running；Stop清理barrier | SP-00/04/05 |
| queryOperation/getRuntimeSnapshot | 拟扩展现有operation_status/snapshot | host→application→bridge/UI | 不被正在apply的写队列长锁阻塞；actual/candidate/cleanup/watermark原子读；unknown不等于failed | SP-04/05/06 |
| RuntimeActualDescriptor/ActualEndpoint | 拟扩展 | host事实→application/monitor/UI | target/core/hash/appliedRevision、端点scheme/owner/core/API类型/auth-required；凭据仅Rust私有context，不能入DTO/log | SP-05/06/17/20 |
| 控制事件ResyncRequired | 拟新增 | host EventBus/pipe→application/UI | epoch/seq/generation，lag可恢复；私有snapshot不占广播seq；权威snapshot版本拒绝迟到 | SP-07 |
| helper V2 RenewLease/QueryOwnedProcess/ReleaseResources | 拟新增 | helper→host | 持有身份、鉴权、lease deadline、逐资源confirmed/pending；传输失败是Unknown；v1不承接新的TUN写入 | SP-06/08/09/10 |
| LoadSettingsDocument | 拟扩展读取入口 | persistence/application→bridge/settings UI | 不存在≠坏文件；raw/typed view、FieldState/default/normalize；错误安全JSON pointer，失败阻止写 | SP-01 |
| 可恢复commit coordinator | 拟新增/扩展 | persistence/application全部mutation入口 | epoch/journal、幂等mutationId、expectedRevision、recover-required，禁止旧直写旁路 | SP-02/03/14 |
| SettingsSaveReceipt/saveSettings/queryMutation/retryApply | 拟扩展 | application/platform→bridge/native/main UI | newRevision+saved_document_token/epoch，save/core/platform分阶段，OS实际fact与applied hash；save成功apply失败不统一false | SP-12/15 |
| WindowRequestEnvelope/WindowSaveOutcome | 拟扩展MethodChannel | main engine→native子窗 | requestId/windowGeneration/mutationId，ACK≠result；exception/timeout/reconcile/close；结果可查 | SP-11/12/13 |
| RoutingSnapshot/MutationReceipt | 拟扩展 | application/main→routing子窗 | schema/read status、domainRevision、persisted/apply、partial committed IDs；主确定不回写旧全集 | SP-13 |
| previewImportText/commitImportText | **已存在generated与Rust API，UI未完整消费** | Rust subs→BridgePort→subs UI | 先核验纯预览、All单事务与文件journal真实合同；取消no-diff、preview/hash或token一致、失败无半批 | SP-14 |
| QueryProfilesPageAsync | 拟异步扩展现有摘要/分页 | persistence/application→FRB→profiles UI | filter/sort/cursor/datasetRevision/requestGeneration/ID tiebreak、bounded page、cancel/late；不会同步读全库 | SP-21 |
| HTTP policy/factory | 拟新增/统一 | 底层共享网络模块→订阅/更新/资源/WebDAV | 无反向application依赖；policyHash/信任来源/受控根bundle，坏CA/hostname拒绝、取消与limits保留 | SP-25及SD-10 |
| t16_apply_app_update_spec_with_flags | Rust现有flags入口，Bridge/UI接线待核 | updater→bridge/update UI | 本次flags+本产品源/key；保存失败可见，验签不可跳过；安装/reopen不是spawn成功 | SP-27 |
| UpgradeReceipt | 拟扩展 | upgrade_runner/updater→application/UI | stage/install/healthy/rollback、匹配程序与数据schema、取消checkpoint、健康握手真实 | SP-27 |

settings/route/profile等domain revisions在DTO明示类型/域；不要让window expectedRevision变成统一desiredRevision。保存文档token使用受控opaque ID和hash，不泄露配置内容或任意路径。真实endpoint credentials只由Rust运行/HTTP context持有。

## 2. 设置子包→SP卡/字段实例

| 设置工作包 | SP归属与依赖 |
|---|---|
| SD-01严格读 | SP-01；容器/类型/default实例来自SP-23 |
| SD-02可恢复提交 | SP-02；所有写入口含SP-03/12/13/14/27均不得绕开 |
| SD-03 canonical IDs | SP-03/16；runtime actual另SP-05 |
| SD-04保存收据 | SP-11/12，共享桥/设置controller单writer |
| SD-05平台事实 | SP-15/32；纯代码不等隔离环境，有副作用验证才需要环境 |
| SD-06 UI canonical | SP-16/19/20和字段实例；不是只INI私有持久化 |
| SD-07通用参数 | SP-23每ID实例；代码生成专属SP-24，测速/调度各自消费者 |
| SD-08 Happy | SP-24的第一唯一流程 |
| SD-09 Fragment/MaxSplit | SP-23字段实例关联SP-24；与Happy分卡，不当一项boolean测试覆盖 |
| SD-10 HTTP | SP-25；其调用者全部逐客户端验证 |
| SD-11 renderer | SP-26可行性及后续runner/engine实例，最终发行检查真实效果 |
| SD-12应用日志 | SP-23叶子实例；日志并发性能SP-22，journal不被禁日志关闭 |
| SD-13消息/统计 | SP-17/22与SP-23字段实例 |
| SD-14 Mihomo | SP-23具体字段/原生custom实例，SP-28对应核真实会话 |
| SD-15路由资源 | SP-23/13，外部模板/本地SRS独立消费者实例 |
| SD-16更新 | SP-27；核心自动目标与SP-28手工库存不同 |
| SD-17备份/WebDAV | SP-03独立路径实例，HTTPS依SP-25，取消/commit依SP-02 |
| SD-18平台专属 | SP-32/33的唯一OS×架构×流程实例，原版不适用须source证据 |

180行CSV的SD依赖用本表解析，不能因为它没有写SP编号就绕开共享合同。相关工作包列不是允许同时修改所有模块的授权。

## 3. 运行子包→SP卡

| 运行工作包 | SP归属 |
|---|---|
| RT-00合同/V2 | SP-00 |
| RT-01快速受理 | SP-04/05 |
| RT-02队列/safe-point | SP-04 |
| RT-03丢回复对账 | SP-04/11 |
| RT-04实际target | SP-05/17 |
| RT-05退出 | SP-06 |
| RT-06主核提权 | SP-10及SP-06统一owned process |
| RT-07提权sidecar探活 | SP-06/10，依helperSP-09 |
| RT-08 adapter identity | SP-10 |
| RT-09长租约 | SP-09 |
| RT-10部分写失败 | SP-08/10 |
| RT-11失败清理重开 | SP-08及SP-35 |
| RT-12 IPv6/protect | SP-10 |
| RT-13 UI实际标签 | SP-10/17，CAS保存依SP-12 |
| RT-14事件恢复 | SP-07 |

RT标签用于进一步拆小实现，不替代SP的文件锁/依赖。避免环形等待：先纯共享合同→host/helper能力→UI，UI暂缺能力登记接口缺口，不能返回假的ready结果。

## 4. Windows阶段与全部完成

SP-30先构建干净未武装候选包用于普通入口验收，可在完整六平台前运行；候选不是最终发布。SP-31/32/35对同源候选实际量测并验OS/持续效果，任何变化重新绑定包身份。SP-34是完整目标发布总门，必须所有适用字段/动作/核心/平台实例通过。Windows x64单独阶段可写当前通过范围与其它未验证，不把总门标complete。

隔离环境/发行源未有时，SP-30按可执行段先收集真实证据，相关段blocked，不等待造成所有独立流停工；它的整卡完整退出条件仍不能伪造已满足。SP-33每单元同理。主manifest前置描述完整门禁，不禁止独立的前置子场景先做。
