# 完整移植验收矩阵与证据格式

状态identified；以下是待执行验收，不能引用为已通过。当前失败与历史已通过仅见审计目录。本方案不把静态implemented作为平台效果verified。

## 1. 每条证据的共同结构

每卡在 `docs/evidence/stable-port/<任务ID>/<实例ID>/` 追加 README、observations.json、原始命令日志、脱敏事件/截图、配置diff、独立进程重开结果及归属资源清理结果。每份 observations 至少记录：

```json
{
  "status": "identified",
  "application_commit": "执行时固定的完整SHA",
  "upstream_commit": "7d6a967c18c697f28dc6917122ed3a4993fcf336",
  "task_id": "SP-xx", "inventory_ids": [],
  "package_sha256": "真实包身份；开发合同测试明确写not_applicable",
  "armed": false, "protocol_versions": {}, "toolchain": {},
  "os_arch": "实际运行环境", "dpi": "实际数值",
  "fixture_id": "合成夹具与生成参数", "core_version": "适用时真实值",
  "steps": [{"input":"具体操作", "expected":"原版预期", "actual":"观测结果", "evidence":"相对路径"}],
  "commands": [{"command":"真实命令", "exit_code":0, "log":"日志路径"}],
  "reopen": {"process_is_independent": false, "result":"未验证"},
  "cleanup": {"owned_resources":[], "result":"未验证"},
  "unverified": []
}
```

JSON示例是格式，不是结果；不把示例exit0和armed=false当实际证据。对秘密只记录字段ID和结构，不记录真实地址/凭据。配置diff使用synthetic，日志按项目规则脱敏。

## 2. 正常与异常用户流程

| 流程/归属 | 普通入口与正常结果 | 故障、取消、并发和重开 | 最终事实 |
|---|---|---|---|
| 首次使用 SP-14/28/30 | 干净目录→导入A/B→安装/指定合法核心→选择B→启动 | 缺核修好重试、非法候选、取消安装、路径空格/中文 | 正式GUI→FRB→host→冻结B配置→真实SOCKS/TLS；端口≥11808 |
| 连续命令 SP-04/05 | A运行→B切换、F5默认入口 | A在途→stop→B；回包逆序；ACK丢失、超时、重复ID | 最后接受意图与实际descriptor一致，未完成结果可查询 |
| 核退出 SP-06 | main/sidecar ready与停止 | 杀本项目自身持句柄核/自然退出、提权核退出 | snapshot/端点/UI及时更新，历史applied与当前存活分开 |
| 事件压力 SP-07 | 状态/结果持续订阅 | 高速日志+2049控制burst、lag、断管、重连 | 不静默终止；epoch/seq缺口可对账；最终结果不丢 |
| 配置恢复 SP-01/02 | 缺字段兼容初始化、原版迁移 | 空/截断/null/已知坏类型/未知键；各写盘阶段崩溃 | 损坏不覆盖、不启动10808；epoch恢复到一份一致配置 |
| 备份 SP-03 | 默认B/组G（临时选C不改默认）→备份→改动→恢复 | DB/file失败、格式差异、WebDAV失败、取消 | canonicalIndexId/SubIndexId/UiItem正确，独立重开和上游可读 |
| 设置窗 SP-11/12/23 | 更改单ID→确定→按原版生效 | SaveSucceeded/applyFailed重试、丢回复、迟到、stale、关窗 | newRevision正确；保存与core/platform结果明确；无重复写 |
| 路由DNS SP-13/23 | 原版即时动作、子方案确定、关闭Modified reload | 读失败、合法空库、部分删除、后段存储失败、并发外部改动 | 不清空/复活规则；已保存事实不被apply失败隐藏 |
| 批导入 SP-14 | All/普通组/订阅组，Custom与分享格式 | 第二条失败、取消预览、rename失败、提交崩溃、重复确认 | 无半批数据库/未提交文件；来源与去重语义一致 |
| 平台设置 SP-15/32 | 保存后按实际内容应用 | 同模式端口改bypass/PAC、写失败、重开重试、外部改动 | appliedHash仅真实成功推进；desired不能推导OS已成功 |
| 组/订阅 SP-16/30 | 当前组恢复、新建直达、编辑直达 | All门控、空URL普通组、同名组、删除当前组、更新失败取消 | 当前操作目标正确；订阅事务与IsSub/活动映射保持原版 |
| 节点键鼠 SP-18 | 即时单击、多选、双击/Enter/F5、右键 | 空白、submenuEsc、滚动、失焦、删除target、排序refresh | action作用稳定ID和原版选择集合，菜单出现/消失实际对照 |
| 底栏/子窗 SP-17/19 | 左右同屏、实际节点、测速入口、主题字体语言 | 800px/DPI200%、长文本、旧错误/新结果、切换失败保旧A | 可读/可操作/反馈当前有效；不靠横向滚动找关键控件 |
| 连接/代理/日志 SP-20/22 | 真实核API→虚拟表、排序、右键close、列保存 | 10k、断线、关闭失败、换session迟到、日志压力 | 真实endpoint/generation绑定；列重开；不全量重绘/阻塞 |
| 大节点库 SP-21/31 | 10k/100k实DB切组筛选排序、多选/批动作 | 换筛选取消、游标失效、overlay并发、全选跨页 | timer/交互不中断；稳定ID/游标/版本；无截断或只操作首屏 |
| TUN SP-08/09/10/32 | 授权隔离机main/sidecar提权ready、IPv4/6/DNS | helper失联/心跳、退出/崩溃/残留/清理失败、恢复重开 | 按资源归属回收；pending失败可查可重试；defaultRoute恢复 |
| 更新 SP-27 | 本项目发行端→当次flags→签名→stage→安装→重开 | 错公钥/签名/来源、路径逃逸、文件锁、取消、断电、回滚 | 新包身份及信任明确；旧版本可恢复；不安装C#上游包 |
| 退出 SP-35 | 保存→stop→退出→立即重开 | apply/下载/清理在途退出、进程崩溃、权限失效 | 无被遗忘资源/写一半/重复session；owned记录可恢复 |

mock只用于可靠故障注入；每行正常流用真实FRB/持久化/最终消费者。主窗口截图不能替代核心和OS观测，纯harness也不能替代正式按钮。

## 3. 全库存与实例化

- 110 feature/185 action：沿既有coverage.csv逐ID追踪入口、可用条件、快捷键、确认/取消、后台调用、实际效果。SP-30为总审查入口，按既有R4卡分配缺口，不一次把总卡verified覆盖所有ID。
- 180设置：逐行CSV执行。容器检查完整结构/空缺/未知键/嵌套迁移；内部字段检查identity/revision/reference；155叶子检查最终消费者。无控件字段不造控件。field instance完成不等于相关action全部完成。
- 158实体：每字段类型/默认/nullable/枚举/引用/clone/import/export/migrate；重点group/subscription/active/template/route/DNS/file引用。直接SQLite与canonical配置往返并重开。
- 36 layout/3 main layouts/53 windows：结构、菜单、列、分组、窗口类型/尺寸/最小宽/焦点/关闭/父子关系、100–200%DPI。WPF与Avalonia各读自己的固定source，不用Windows截图带过Linux。
- 14 core：每核建实例（沿既有R4-34和协议R4-18卡），formalGUI选择→配置校验→ready→真实合成流量→停止/退出；适用协议/format/domain字段正负向矩阵。自动更新仅上游对应目标；原版没有的适用性必须有source证据才能not_applicable。
- 六平台：Windows x64/ARM64，macOS x64/ARM64，Linux x64/ARM64。每平台 build/package/runtime/托盘/热键/文件与权限/DPI/更新/清理独立证据；平台特有代理/TUN/backend单列。其它原版架构保留可行性差距与规格决策，不悄悄删除台账。

## 4. 门禁与性能

以下完整命令是发布候选的汇总检查，不是每卡都执行。日常修复、整合与高成本验收按[检查频率规则](VALIDATION_POLICY.md)执行；完整覆盖保持，避免重复跑整个清单。

Rust workspace：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --locked`。

`apps/desktop`：`dart format --output=none --set-exit-if-changed lib test`、`flutter analyze`、`flutter test`、`flutter build windows --release`。使用AGENTS锁定工具链；任何缺失、忽略、异常退出或未完成都列明，不改skip/list装绿。FRB二次生成no-diff，版本三处一致。

性能fixture实DB1k/10k/100k，连接1k/10k，有日志/测速overlay并发。至少20冷/30热启动样本；排序筛选和普通交互多个重复样本，预先固定采样数和percentile算法。记录p50/p95/p99和原始数据；cold/warm分开。

原规格门槛：反馈p95≤100ms；1k冷启动可交互p95≤2s；热≤1s；托盘≤150ms；10k搜索排序p95≤200ms；10k UI/raster各p95≤16.7ms且超预算≤1%。同时记录总帧时间避免各段通过但总帧超预算。内存/线程/队列有基线及无持续增长证据，不虚构规格未定义的绝对内存限额。

24h持续真实合成负载、至少500次切换含失败和cleanup；控制事件/速率/监控/日志并发；退出重开journal收敛。不能把一次测试进程没崩当24h通过。

## 5. 正式包验收和判定

SP-34包检查固定clean HEAD、工具链/锁文件、armed=false、FRB/协议/core版本、所有payload hash。先检查main runner+data/app.so+DLL+host+helper+updater+资源，再解压干净路径普通UI流程验收。包内不捆核时必须真有合法安装/选择入口，不能开发环境自动供核。

auto smoke只是辅助；普通UI验收不设置自动hook、不预置active，不读取用户data。OS副作用走授权隔离环境，10808绝对避让。真实流量使用受控loopback/TLS夹具，不需要真实用户节点凭据。

通过条件：本次所有正确失败合同转绿，相关历史好行为仍通过；库存适用项有链路证据，P0/P1零且完整移植P2关闭；门禁稳定完整；性能和24h/500切换通过；正式包与证据同一身份。外部输入不足时诚实blocked，不填写verified。

Windows x64阶段通过可称“Windows x64已验收”，但六平台完整目标仍需其它五单元通过。不给混合分母的完成百分比。
