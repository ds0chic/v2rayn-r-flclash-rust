# 可直接交给执行模型的开始指令

先读[检查频率规则](VALIDATION_POLICY.md)：单任务仅受影响格式/静态/行为检查，跨模块由整合者汇总联动检查，发布候选才运行完整门禁。禁止每完成一张卡就跑全部约290个文件、整个workspace和release构建；不带filter/skip的run_all不是每卡钩子。失败先定向修复，下一稳定候选再全量。已有行为测试优先复用，不强制每卡新建同名测试。

在本仓库按 `docs/repair/stable-port-2026-10-06/IMPLEMENTATION_PLAN.md` 完成完整稳定移植修复。先读AGENTS、主方案、三份实施分册、当前全范围审计、execution-manifest和本次唯一任务卡。应用审计基线a7aa0a5，原版固定v2rayN7.25.4/7d6a967；先核对当前HEAD和已有改动，不把本方案当生产修复已完成。

第一次先核对SP-00共享合同、所有权和编译可用接口；已有implemented成果按当前证据复核，不重写。按manifest和[10～15子代理安排](PARALLEL_EXECUTION_15.md)领取当前未完成流程，前置不足先做独立源码对照/模块准备。新增API/DTO在设计里是拟新增，必须先找现存签名和消费者，能复用的API优先复用。一个任务一次一个用户流程/字段实例/平台实例，不整批把180字段标完成。

先为当前真实失败建立正确预期的回归，再改最小范围：启动A→stop→B、核心退出仍Running、applied目标竞态、cleanup失败丢journal、坏配置默认化、canonical备份还原、路由旧草稿复活、All导入半写、保存成功应用失败重试、独立窗丢回复。不能把测试预期改成现有错误，也不能拿观察bug的pass用例作正确实现证据。

每个功能走原版预期→正式UI→真实FRB→Rust→持久化→独立重开→真实配置/应用/核心/平台效果。mock只用于故障注入。未验证环境或消费者不足先登记提供方、调用方、DTO、版本、错误、取消、存储和生效时机，保留范围；不删功能、不造“暂时不支持”的默认豁免。

运行命令/窗口回包超时是结果未知，先query operation/mutation/snapshot，不能重写产生重复效果。selected/currentGroup/default/actualRuntime身份分离；desiredRevision/newRevision/appliedRevision/actualGeneration分清。stop cleanup不可被后来的apply吞掉；save成功apply失败返回分阶段事实并允许重试已保存内容。

用户允许10～15个子代理，按PARALLEL_EXECUTION_15.md的10个主要任务+5个补充任务调度，实际并发服从执行器限制；当前会话主控+子代理共4槽，最多3子代理同时活跃，不冒称启动15个。根整合者负责shared DTO/engine/bridge/FRB/native host接线与候选全门禁。单文件同时只有一个写入者，按owner租用；构建/GUI/性能另排队，不每个代理全量跑290项。FRB生成一人执行、三处2.13.0一致，桥或生成配置变化才二次no-diff；协议和迁移有版本及兼容错误，不暗中接旧helper写OS资源。

保留旧已修行为：顶部selected目标/F5默认、即时选择/捕获modifier、右键target冻结与滚动失焦关闭、有组批导入及IsSub来源、DNS草稿baseline、热键pause gate、stale拒绝/保存失败反馈、Legacy单TUN provider。原版路由即时提交和关闭Modified reload，参数窗关闭时机依原版。未真机对照的submenu Esc/高DPI仍逐项验。

测试只使用合成地址、凭据、CA和节点，不读取真实用户配置。测试端口先探测且≥11808，禁止占用/修改10808；禁止宿主WinINET/WinHTTP/注册表代理、TUN/default route/DNS、自启写入。OS有副作用必须已授权隔离环境；只停止本项目持句柄/身份记录的进程，不按名批杀。work/outputs只读，compat只能追加和按证据提升状态，不能降低分母。

先完成能独立进行的应用修复，再验OS副作用；不因没有VM而停掉普通流程。RootCertProvider是应用HTTPS信任来源；HWA先证明renderer可行性，不把低功耗GPU/禁Impeller当软件渲染。外部内核继续受管理，不重写Xray。

每卡更新状态和 `docs/evidence/stable-port/<ID>/`：改动文件、真实命令/exit、正确合同、正式入口、重开、最终消费者、owned资源回收、未验证与下一前置。状态仅identified/implemented/verified/preserved_only/blocked/not_applicable。任务已做不得重跑旧卡生成器覆盖历史。

最后必须重新从干净固定提交打未武装包，核对Dart AOT和所有Rustpayload hash，普通GUI验收。旧ZIP不能验新源码。完整门禁、原版库存、性能、24h/500切换和平台实例通过后才能报告对应范围稳定完成；不能再按“代码很多/单测全绿/构建成功”给总体完成率。
