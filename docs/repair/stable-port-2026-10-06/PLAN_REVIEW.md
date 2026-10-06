# 本次方案交付核对记录

日期2026-10-06；产品状态identified。本记录仅验证方案一致性，不声称生产修复、测试或平台通过。

## 交付内容

- 主实施方案及运行/TUN、设置/数据/更新、界面/性能三分册。
- 36张整合任务卡，13个固定小卡字段齐全；字段实例模板及唯一字段/核心/动作/平台实例要求。
- 180设置ID逐项消费者映射，155叶子/23容器/2内部；SD-01..18和RT-00..14映射到SP卡，接口提供方/调用方和共享文件锁。
- 可机读依赖manifest、完整验收矩阵、执行模型开始指令；历史R4卡和全库存不削减。

## 交叉复核与已修正文档问题

运行代理提出并整合：发送前登记operation ID、hostInstance/global admission序列与幂等hash、typed实际端点且秘密留Rust、host/application快照权责、query不排长写队列、所有Stop清理barrier、helper V2同包迁移，以及SP-06/10新增process/执行/生成模块范围。

设置代理提出并整合：非二元保存状态CommitUnknown/RecoveryRequired、datasetEpoch与mutation/commit身份分离及跨窗对账、SP-25低层HTTP和Cargo修改所有权、SP-27实际application发行源模块，以及临时selected C不擅自变成原版备份恢复要求。

原版选择核对：冻结 `ServiceLib/ViewModels/ProfilesViewModel.cs:361` 的RefreshServersBiz，消费内存pendingSelectIndexId后选可见Config.IndexId，再否则首行。正常备份/重開验证默认B、组G；临时C不改default，不增设用户未要求的持久化选择语义。

UI方案代理因用量限制中止；根代理依据已完成UI审计、当前正式调用链和冻结源码补全UI分册。不得把中止委派算成独立完成审查。

修正过的文档源路径：Config位于Models/Configs/Config.cs；核心控制位于Manager/CoreManager.cs；当前核心锁为tools/cores/cores.lock.json。新增文档所有相对链接及旧R4引用实际存在。

## 实际运行与结果

`python docs/repair/stable-port-2026-10-06/validate_plan.py`：exit0；36唯一整合卡、17 CP覆盖、180唯一ID与台账相等、分组23/2/155、冻结source_file均存在、依赖DAG通过、49个本目录文档链接存在。结果写document-validation.json。此脚本不启动产品、不改台账/生产模块。

重新核对git HEAD为a7aa0a5b1ecd523bc60d201e0696ff6051cddf26；git diff在crates/services/apps/tools/compat/work/outputs范围无生产改动。当前变更为上一轮审计和本轮方案文档；未创建新发行包、未运行本轮Rust/Flutter产品门禁、未启动内核或写OS设置。历史真实测试结论仍在原审计，不能混进本轮“已通过”。

## 执行前置和边界

第一次执行先SP-00，逐个签名收敛和共享文件锁，再做数据/运行状态；字段、动作、核和平台每次唯一实例。设计里的API是拟新增，已有preview/commit API必须先核验复用。

本产品真实发行源/信任根、授权OS副作用测试环境、其它OS/架构设备仍是执行验收前置。HWA要锁定引擎研究，hotkey生效时机与TLS后端需核原版实际入口；不凭CSV静态文字忽略源码冲突，不因OS待验而放过代码消费者缺口。

完整稳定移植的生产目标尚未实现。文档通过只说明实施映射与依赖一致，不能用36卡/180行/17问题数量计算软件完成率。
