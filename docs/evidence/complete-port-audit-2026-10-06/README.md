# 完整稳定移植复审与验收方案（2026-10-06）

状态：identified。当前版本 **不能验收为完整、稳定的移植版本**。本轮已做全范围库存复核、三个领域并行审查、当前源码构建检查、正确预期故障测试、真实 Xray 会话和真实 FRB/SQLite 检查。仍有运行状态错误、命令时序错误、数据往返错误、失败后不能重试、设置无消费者等实质缺陷，不是只剩 TUN 真机测试。

审计后的详细解决方案已单独交付：[完整实施方案](../../repair/stable-port-2026-10-06/IMPLEMENTATION_PLAN.md)、[执行卡](../../repair/stable-port-2026-10-06/tasks/README.md)、[180字段消费者映射](../../repair/stable-port-2026-10-06/SETTINGS_IMPLEMENTATION_180.csv)。包含拟接口、状态迁移、代码边界、失败恢复、依赖与验收；此追加仅文档，不改变下面的审计结果，也不代表生产修复完成。

## 1. 固定基线与证据口径

- 应用源码：`a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。开始时工作树干净；本轮未修改生产源码、冻结源码、规格原件、依赖锁和发行包，新增审计工具/测试/文档并追加相关任务记录。源码目录结束时 diff 为空。
- 原版：v2rayN 7.25.4，`7d6a967c18c697f28dc6917122ed3a4993fcf336`；Windows 以冻结 WPF 为界面基准，macOS/Linux 以 Avalonia 为基准。不是声称跟随审计日的新上游版本。
- 全库存：110 feature、185 action、180 设置字段（155 叶子、23 容器、2 内部）、158 实体字段、36 布局条目、3 主布局、53 窗口清单、14 核心锁及六 OS×架构交付单元。不同维度不可相加计算完成度。
- 完整库存身份、ZIP 哈希与元数据见 [baseline.json](baseline.json)。设置完整 180 ID 集合见 [当前矩阵](settings/settings-current-180.csv)。矩阵的静态 implemented 不是实际有效，台账当前没有条目级 verified。
- [UI 分册](ui/README.md)、[设置与数据分册](settings/README.md)、[运行与 TUN 分册](runtime/README.md) 提供定位、原版证据、输入时序、当前结果、修复要求和范围限制。旧 2026-10-05 报告只作历史，不直接当当前结论。
- 没有运行默认数据的桌面应用，没有读取真实节点/订阅；没有修改宿主代理、路由、TUN、自启或证书，没有触碰 10808。实际核会话只用已预探测 11977/11978、本次 session 身份和持有的进程句柄。

## 2. 需要先修的阻断项

下面按根因分组，不用问题条数代替完成度。P1 是稳定交付阻断；P2 对“完整原版移植”的验收仍是必修差距。

| 编号 / 优先级 | 用户实际流程与当前结果 | 当前证据 | 修复归属 |
|---|---|---|---|
| CP-01 / P1 | Xray 就绪后退出，实际端口失效，net_host 3 秒后仍报告 Running、旧 PID/端点、无错误 | 真实本地核故障，`runtime/real-loopback.log` | R4-01/23：运行管理持续观察主核与 sidecar 退出，更新事实并发事件；保留历史 applied revision 不等于保留活跃端点 |
| CP-02 / P1 | 启动 A 在途→停止→启动 B，队列执行 A/B/stop，最新启动意图被旧停止覆盖；apply 超时但后端已 Running 时 UI 不对账 | `runtime/runtime-contracts.log`，1 pass / 5 fail 中两项 | R4-04：命令统一有序队列/意图序号；超时=结果未知，查 operation/session 后决定，不能直接重放写入 |
| CP-03 / P1 | A 计划在途时把默认改 B，A 应用后 applied_target 却读成 B；成功 apply 的 job 在运行和停止后仍 Running | 实际 AppEngine 并发门控正确期望失败；Rust 观察用例 | R4-01/04/23：提交时冻结显式 target，跟实际 descriptor 保存/重连；operation ID/job ID 分开，终态正确结束 |
| CP-04 / P1 | TUN 清理错误被吞；回收记录删除、owned 资源清空，仍报成功，下一次不重试 | `cleanup-journal-contract.log`、`cleanup-helper-contract.log` 正确预期均失败，纯文件/FakeBackend | R4-25/05：保留 pending cleanup 和归属记录，逐步确认清理；错误要跨 helper→net_host→UI 传播。**没有声称本轮实际残留宿主路由** |
| CP-05 / P1 | 自启写失败后重开设置不再重试；独立设置窗第一次保存成功/应用失败后，再按确定只拿旧版本而 stale | 设置 3 项故障合同的前两项明确失败 | R4-13/12：desired 和 OS 确认分离；本窗口成功保存的 newRevision 更新回窗，retry-apply 与外部冲突分开 |
| CP-06 / P1 | 已存在空配置仍加载成功；某已知字段类型坏了，整份配置被默认化，原语言/去重/端口同时丢失 | 真实 Rust、合成磁盘/SQLite，`settings/pure-probe/observations.json` | R4-27：缺字段默认与坏类型/截断错误分开，结构化失败并禁止覆盖；未知键保留，恢复入口可见 |
| CP-07 / P1 | 当前活动 B，保存后旧 canonical IndexId 仍 A；实际本地备份→兼容 ZIP→恢复返回 Imported，活动变成 A | 真实库/ZIP/lifecycle restore，不是 mock | R4-28/27：canonical ID 与新状态统一，导出/导入双向迁移一致；选中、当前组、默认、实际运行四种身份明确分离 |
| CP-08 / P1 | 路由规则读失败/畸形快照仍当可写空规则；多选删除 a 成功、b 失败后再确定，旧全稿复活 a | MethodChannel 畸形快照和实际编辑 widget 故障合同失败 | R4-14/27：读取失败不能编辑/提交；每次增量提交后对账，主窗关闭不能重新写旧全集 |
| CP-09 / P1 | 参数/路由窗保存传输 ACK 成功但结果回复丢失，31 秒后仍永久等待 | `audit-ui-native_reply_loss-confirmed.log`：2项丢回复失败；另1项畸形快照失败属CP-08 | R4-12/13/14：request/window generation、有限等待、结果查询，主回调异常返回结构化结果，关闭后旧回复不串窗 |
| CP-10 / P1 | All/无组导入第二条写失败，第一条已保存；预览旧 API 可材料化 Custom 文件，新 pure preview/transaction API 还没接正式入口 | 合成仓储故障复现 + 当前真实 Rust/SQLite调用链确认；未注入真实SQLite失败 | R4-16：先纯解析，确认一次事务提交；取消无文件/DB效果，批写失败不半写。不把有组批事务的好结果代替 All 流程 |
| CP-11 / P1/P2 | Happy Eyeballs 关闭仍生成；合法 MaxSplit 1-3 被拒；HWA/应用日志/根信任/日志过滤/Mihomo merge/外部路由模板/本地 SRS 等消费者缺口 | 实际 codegen 前两项已复现；完整 180 字段矩阵及来源追踪 | R4-13/19/34：按字段 ID 接 UI→application→最终消费者，保留范围语义和原版生效时机，逐核校验配置差异 |
| CP-12 / P1/P2 | TUN 24h 无 helper 心跳仍清活跃租约；实际租约被 desired/旧错误遮盖、dryRun 也称已启用；主核提权/提升核探活退出/IPv6保护仍缺 | 虚拟时间 + 标签正确合同 + 当前计划/权限源码 | R4-25：持续会话保活、实际事实优先、主/sidecar 生命周期统一、补能力和保护路径；真实 OS 路由失败恢复尚未验 |
| CP-13 / P1/P2 | 更新/代理选项保存失败不显示或回滚；平台内容去重忽略 bypass/PAC 等变化；更新使用旧 flags；自有发行源/公钥接线不完整 | 故障合同 + 当前生产调用链 | R4-13/24/29：分别报告保存/core/platform 结果，内容 hash 去重，当次 flags 显式传给后端，接本项目源和信任根 |
| CP-14 / P2 | 当前组重开回 All；新增/编辑订阅仍打开总列表；运行节点身份和状态栏测试入口缺；子窗主题/字体/语言不继承；连接表列/右键未齐，旧平台消息盖新结果 | UI 分册源码/原版证据，组恢复合同失败 | R4-06/11/12/17/23/26/30：逐入口和状态身份对齐；canonical 布局持久化，消息按当前操作和严重性呈现 |
| CP-15 / P1/P2 | UI 调用同步全量数据读取；真实 10k 为102–123ms，100k 同样分页循环约2982ms，尚未加入过滤/排序/overlay | 当前 release Rust DLL + 实际 SQLite/FRB；详见 §4 | R4-09/10/31：后台分页/筛选、稳定排序游标、版本化取消与增量，小 payload；不能只虚拟化绘制 |
| CP-16 / 发布阻断 | 新源码能构建，现有 ZIP 却是旧提交+dirty=true；原测试门禁仍未全绿，完整普通入口/平台/性能未验 | build/test日志、ZIP内部元数据、二进制差异 | R4-32/33：整改后从固定提交重新完整打包并走未武装普通入口验收，不能换成开发配置/hook证明正式包 |
| CP-17 / P1（有条件） | 控制事件订阅落后时，生产forwarder把可恢复Lagged当EOF结束，连接却不通知客户端恢复 | 实际EventBus 2049事件burst出现Lagged + 生产循环确认；跨pipe压力未实测 | R4-04/23：明确lag/resync或可靠重连，控制事件与高频日志隔离，epoch/seq和权威快照重新校对 |

状态栏 800px 测试看到初始控件/速率在视口外，但滚动后全部可到达；这是原版左右同屏布局和体验差异，**不是功能丢失**。优先修数据/运行合同，再修细节；不删除 P2 以降低完整移植要求。

已修的顶部启动目标、即时选择、右键目标冻结、有组导入、DNS草稿保护、6项保存/结果合同，以及helper旧5秒回收、Legacy双TUN provider，仍保留对应进步。本报告追查的是剩余和新增合同，不把旧缺陷重新算未修。

## 3. 实际检查结果

| 检查 | 本轮结果 / 限制 | 原始证据 |
|---|---|---|
| Rust format / clippy -D warnings | 两者 exit 0 | `checks/cargo-fmt.log`、`cargo-clippy.log` |
| Rust 全 workspace tests | 1410 passed / 1 failed / 5 ignored；`--no-fail-fast`完整跑完，不忽略失败装绿 | `checks/cargo-test-no-fail-fast.log` |
| Rust 失败解释 | helper loopback 的旧测试只设置 request_timeout，idle_timeout 已是24h，2秒内不返回；是测试合同/配置未同步，不等于正常连接2秒坏。需要明确 timeout/保活合同并正确测试 | `runtime/helper-idle-regression.log`；本轮没有放宽或删除预期 |
| Dart format | exit1，3文件需格式：settings_actions.dart、settings_controller.dart、audit_tun_settings_contract_test.dart；`--output=none`未写源码 | `checks/dart-format.log` |
| Flutter analyze | exit0，无问题 | `checks/flutter-analyze.log` |
| Flutter 全量 tests | 284文件库存；批跑 exit1（898 passed、2 skipped、加载异常/多项未完成）；29受影响文件逐个补跑，最终26成功、3仍异常未完成 | `flutter-batch.log`、`followup-results.json`、`final-results.json` |
| 3个仍未完成文件 | r3_visual_dpi_windows、r4_07_contract、t11_menu；日志为测试进程退出/did not complete，无完整 Dart 断言或原生栈，根因未定。不能算作业务正确断言失败，也不能称门禁通过 | `checks/batch-recheck-*-retry.log` |
| 已修保存/结果6合同 | 本轮明确复跑6/6通过 | `checks/audit-fixed-six.log` |
| 新 UI 故障合同 | 7项正确期望均失败；首轮 native 加载异常和组夹具空 URL 错误保留，更正后才计产品断言 | `checks/audit-ui-*-confirmed.log`、`audit-ui-status_viewport.log`、`audit-ui-routing_partial_commit.log` |
| 新设置故障合同 | 3项全失败 | `settings/settings_retry_contract.log` |
| 新 runtime Flutter 合同 | 1通过/5失败（bool失败结果已修，其余实际标签/顺序/未知结果未修） | `runtime/runtime-contracts.log` |
| 真实 Xray | 正常启动/真实SOCKS握手/坏候选保旧/好候选切换/stop/shutdown通过；自有核退出后 ghost Running 确认 | `runtime/real-loopback.log`；生产源harness，不是正式GUI入口 |
| 真实 FRB/SQLite 10k | 有组导入10k、保存语言、独立进程重开读回均通过 | `checks/audit-frb-seed.log`、`audit-frb-reopen.log`、两份 observations |
| Rust/Flutter Windows release | 两者构建 exit0；仅编译，不证明普通GUI或OS功能通过 | `checks/cargo-release.log`、`flutter-release.log` |
| 14核心资产 | 14资产SHA256匹配lock、14exe存在，13exe有期望哈希且匹配；6核有上游checksum、8仅本地hash | `settings/core-inventory.json`；此次没有重跑全14核 |

本轮新增 Rust 正确合同（applied目标并发、清理错误保留journal、helper清理失败保留归属）明确失败。另有“记录当前错误”的观察用例返回 pass，不能作为正确实现通过；详见运行分册。

## 4. 性能和包身份：可证明什么

实际 FRB probe 通过新 release DLL打开独立 SQLite，没有调用会把仓库替换为 Memory 的 seed helper。10k 有组导入耗时约613ms，`FrbBridgePort.queryAllProfiles`为101.533/123.148ms，调用期间 Dart 定时器没有调度。这个结果只是两次同步读取观测，不是 p95/p99、release GUI帧率或原版对照。

100k 最初一次导入只得到10k，是 probe 忽略默认 MAX_IMPORT_ITEMS 的测试设计问题；后改为10次合法10k批次，实际 SQLite存有100k。Flutter tester在大读取时异常退出，不能据此断言发行应用必崩。追加独立 Dart VM直接加载同DLL，同500行分页循环完整读回100k，**2981.659ms**。结果见 [frb-native-100000-observations.json](frb-native-100000-observations.json)、`checks/frb-native-query.log`；重开数据库和语言已实际确认，原生桌面完整流未运行。按生产UI isolate同步调用关系推断，调用线程在循环完成前不能处理交互；它不是发行GUI直接计时，也不能称完整启动耗时。

现有官方ZIP的SHA256复算与SHA256SUMS一致，但内部build-info为 `672e666` / dirty=true / armed=false。新构建的 bridge_api、net_host、privileged_helper、upgrade runner、Dart AOT data/app.so均与ZIP不同；Windows runner exe一致并不意味着Dart界面代码一致。见 [release-identity.json](release-identity.json)。旧包不能用于关闭最新修复；本轮没有覆盖它，也没有把新build目录当正式新包。

## 5. 给实施模型的修复顺序

沿用 `docs/repair/`76张卡和原版库存，不重新生成卡覆盖历史，不减少功能/字段/平台分母。下面是当前基线增量入口，避免再用旧“主要完成”前提。

1. **先锁事实与数据安全。** CP-01/02/03/04/06/07/08/10。修核心退出观察、单一时序命令队列、冻结target、未知结果对账、helper失败清理保留、错误配置拒绝、canonical ID往返、路由提交、All导入事务。先让本轮正确合同转绿；数据损坏/错误实际状态存在时不开展正式发布验收。
2. **修真实窗口与失败重试。** CP-05/09/13。多窗口返回保存后的版本和分阶段结果；断联/异常/延迟回复有界、可查询，重试不重复已提交内容；自启与代理按实际效果对账。使用原生独立窗口，不能只把控制器单测变绿。
3. **逐字段补真实消费者。** CP-11/12/14。以180字段矩阵为主，每次一个原字段或分组，检查最终配置/OS效果，不把存储序列化当移植完成。RootCertProvider是应用HTTPS信任来源，不是安装系统证书；自动更新核心目标也按原版，不要求原版没有的全部核自动更新。TUN当前single provider等修复保留，不重复重写。
4. **完成用户交互和视觉一致。** 节点右键目标/子菜单Esc与失焦，选择键鼠时序，原版最小宽状态栏左右分区，连接虚拟表/列宽列序/排序/右键，子窗主题字体语言，当前组恢复，正确消息顺序，全部原版入口。不要凭自己的习惯改变默认、确认取消或原版提交时机。
5. **消除同步长路径并测量。** CP-15。查询/大I/O走worker，返回可视页/增量；UI保留焦点/选择/滚动，排序稳定、游标版本明确、取消和迟到响应正确。10k/100k真实DB、10k连接、测速/日志并发下测p50/p95/p99、帧、内存与队列等待。原方案普通反馈p95≤100ms、10k搜索排序≤200ms、10k UI/raster各p95≤16.7ms且超预算≤1%，同一核心/配置对照原版。
6. **最后验真实平台和发行。** TUN提权/主与sidecar ready和exit、IPv4/IPv6/DNS/protect进程/默认路由/失败退出恢复，在授权隔离机完成有副作用测试；非系统效果不等VM继续独立修复。接本项目真实发布源/公钥，当前 flags从UI显式传，验签、stage、安装/替换/回滚、重开实际跑。重新固定干净提交打包，核算所有包内文件身份并验未武装普通入口。其它OS/架构逐实例独立构建/实测，不把Windows通过带过六平台。

## 6. 完整稳定版退出条件

每个适用 feature/field/action/layout必须有“原版预期→正式入口→Rust→持久化→彻底重开→实际配置/内核/平台效果”的当前证据；测试替身只负责故障注入。每项结论标 `identified / implemented / verified / preserved_only / blocked / not_applicable`，保留未验证范围，不用“文件写完”“单测很多”“核版本能运行”计算完成率。

Windows普通未武装发布包至少走完：空目录首次导入/安装核/选B启动→缺核修好重试→切组/右键/快速键鼠→运行中切节点和改设置→TUN/PAC/代理实际状态→订阅失败/取消/活动映射→路由DNS保存失败与恢复→备份/改变/恢复/重开→更新/回滚→退出立即重开。全部异常反馈可读、可恢复；P0/P1为零，完整移植P2差距也需关闭。

构建门禁全绿且测试进程异常根因明确；24h真实负载、至少500次切换和相应故障清理证据完成；当前包固定HEAD/hash/未武装/依赖/核心版本/系统/DPI，原生截图与观察对应同一构建。无法本机完成的OS/架构实测单列未验证，不能写成已做。当前这些退出条件没有全部满足。

本轮交付是可复核审计、故障测试和修复/验收方案，**不是已修好的新版本**。Oracle技能在可用技能目录中未找到，未调用收费API；已用三个独立领域代理与根复核补交叉审查，不能冒称Oracle第二模型审计。
