# TUN 与全部设置生效审计（2026-10-05）

结论：**TUN 有已复现的发布阻断缺陷，设置没有全部真实有效。** 当前应先修复运行生命周期、保存合同和缺失消费者，再验收真实平台效果。现有单测通过和设置能保存，不足以证明用户操作后的功能有效。

审计范围为 HEAD `672e666677a9a1e055e38aa99bd0293f90183d9e` **加审计开始时已经存在的未提交修改**，对照冻结 v2rayN 7.25.4 / `7d6a967c18c697f28dc6917122ed3a4993fcf336`。这些产品改动没有覆盖或回退，本轮没有修改生产源码或发行包。[baseline.json](baseline.json) 记录被审查工作文件的 SHA256。本次不是正在运行的某个安装包的真机验收，不能据此确认用户那一次故障的唯一原因。

本轮由三个并行审计子代理分别检查 TUN、网络/内核设置、桌面设置；主代理交叉核对关键代码、补故障复现并验证矩阵分母。所有执行只用合成数据、内存平台依赖、纯配置生成或专用 scratch 文件。未启动真实应用/内核，未监听端口，未修改宿主系统代理、TUN、路由、Run-key，也未读取用户订阅或节点凭据。

## 最先处理的确认问题

| 优先级 | 问题及用户可见后果 | 当前证据 | 详细定位 |
|---|---|---|---|
| P1 发布阻断 | helper 连接保持打开，空闲约 5 秒仍清理活跃租约；新增提权核心也被停止。可能出现开启后失效而主核仍显示 Running | 原生产 server + 内存 duplex，5010 ms 清理记录；真实网络后果未实测 | [TUN-A01](tun-audit.md)；`privileged_helper/server.rs:482,527` |
| P1 发布阻断 | LegacyProtect 生成主 Xray 与前置 sing-box 两个 TUN provider；严格路由的主核绑定处理也未对齐原版 | 原生产应用生成计划，两个 provider 名称均为 v2rayn-tun | TUN-A02；`application/engine.rs:2917,3025` |
| P1 发布阻断 | 关闭 TUN 申请失败、旧会话仍 Running，toggle 却返回 applied=true；标签根据期望开关显示未启用/已关闭 | 真实 RuntimeController 的结构化失败注入；FRB 丢失 TUN 实际事实 | TUN-A03；`tun_toggle.dart:37,46`、`status_bar_view.dart:421` |
| P1 发布阻断 | 提权仅接 sidecar，目标主核的 TUN 没接同等授权路径；提升 sidecar 返回进程句柄即成功，缺就绪/退出检查 | 生产路径静态确认；普通用户真实设备创建未验证 | TUN-A04/A05；`net_host/session.rs:954,1330,1595,1700` |
| P1 发布阻断 | 生产配置上下文没提供全局 IPv6 探测和内核进程路径保护，相关生成分支始终使用 false/空列表 | 全仓消费者核对 + 纯生成结果；未宣称已发生 IPv6 泄漏或代理递归 | TUN-A06；`application/codegen.rs`、`xray/inbound.rs:89`、两种 routing |
| P1 | 存储不可用时，分组保存绕过失败守卫，仍返回成功并写出默认配置文件 | scratch 中 `load_settings=Err`、`save_settings_group=Ok`，配置文件存在 | 引擎分册；`application/engine.rs:1679` |
| P1 | 开机自启首次写入失败后再次按确定，会跳过重试并提示成功 | 内存平台持续拒绝：attempts=1、第二次 ok=true | AUD-DESK-01；`settings_controller.dart:204,217` |
| P1 | 旧设置草稿使用控制器最新 revision 提交，覆盖编辑期间的新状态 | 先把语言改为 en，再保存旧稿；保存成功且恢复为 zh-Hans | AUD-DESK-02；`settings_window_host.dart:64`、`settings_controller.dart:140` |
| P1 | 系统代理应用失败没有进入设置总结果，设置窗口可报操作成功 | 平台注入 E_PLATFORM_BACKEND、内核应用成功，最终 outcome.ok=true | AUD-DESK-03 / AUD-ROOT-05；`settings_controller.dart:225` |
| P1 | Happy Eyeballs 开关未被最终生成器消费，关闭后仍输出对应配置 | true/false 两态的 Xray 配置完全相同，false 仍有 happyEyeballs | 引擎分册；`config_codegen/xray/dns.rs:225,262,371` |
| P1/P2 | 多个设置缺少生产消费者：HWA、应用日志、证书来源、消息过滤/刷新持久化、Mihomo IPv6/mixin | 逐项查 UI、保存、生成/客户端/平台消费者；不是单纯待授权测试 | AUD-DESK-04；详见字段矩阵 |
| P1 | 用户取消全部更新勾选，空数组被桥接解释成所有更新目标 | 原版空数组跳过全部，null 才选全部；当前桥接空数组扩为 BUILTIN_TARGETS | AUD-DESK-12；`bridge_api/api/t16.rs:869` |
| P2 | 完整保存后没有同步本地 groupRevisions，紧接着保存单个分组即 stale | 真实 SettingsController + 与 Rust 相同计数规则的内存桥接，正确合同失败 | AUD-ROOT-01；`settings_controller.dart:147`、`application/engine.rs:1653` |
| P1/P2 | 原版合法 Fragment MaxSplit=1-3 被拒绝（P1）；排除地址末尾逗号产生空 CIDR 导致失败（P2） | 原生产 validate/codegen/TunSpec 的纯检查 | 引擎分册；`application/settings.rs`、`option_setting_window.dart:1244` |
| P2 | 原版列、窗口和分隔比例设置与 ui_state/INI 两套状态分离；部分界面开关赋值后无人读取 | canonical 字段消费者缺口；当前 INI/临时 UI 状态仍可能有自己的恢复能力 | AUD-DESK-05/07/08 |

表中严重级针对当前可用性和移植合同，不代表已在宿主系统实际造成断网、泄漏或文件破坏。TUN 重复地址创建、停止旧设备后复用旧索引、env adapter override 不一致属于另外的条件问题，见 TUN-A07/A08/A09，未混入上述实际复现结果。

其它明确缺口见分册：外部路由模板来源填写后直接返回不支持；旧 RoutingIndexId 没有一次迁移；更新/监控开关的分组保存失败被忽略；自定义 PAC 缺失路径会被写入默认内容；独立设置窗口未继承主题/字体/语言；macOS Dock 与代理脚本缺生产实现。资源下载也不能自动等同实际配置使用：`local_srs_files` 在生产上下文没有赋值，本地 SRS 规则集选择分支仅测试覆盖（`config_codegen/input.rs:469`、`singbox/ruleset.rs:82`），仍需补组装和场景验收。

## 设置逐条结果

[settings-effectiveness-180.csv](settings-effectiveness-180.csv) 收录台账 **180/180 唯一 ID**，无缺失和重复；包括 23 个设置容器、2 个内部根状态、155 个细项。每条标注 UI 控件、持久化/消费者位置、生效时机、问题、证据和实际效果是否验证。容器不是独立用户开关，不作为额外功能数。

155 个细项的本轮审计状态：`implemented` 102、`identified` 34、`blocked` 19。`implemented` 表示找到生产消费路径，**不等于真实端到端验收通过**；`blocked` 分别标明平台待验或缺实现。两个根内部键仅按 `preserved_only` 登记。统计见 [matrix-summary.json](matrix-summary.json)，这些数值不能换算为产品完成百分比。共同保存故障还可能影响多个有消费者的字段，不能把其余 102 项理解成全部正常。

读分册可获得全部定位、原版语义和验收要求：

- [TUN 审计](tun-audit.md)：9 组发现，含 11 个 TUN 字段消费者；生命周期、权限、拓扑、实际状态与条件故障。
- [网络/内核设置审计](settings-engine-audit.md)：89 个细项；配置生成、DNS、Mux、测速、路由、Fragment、TUN 等。
- [桌面设置审计](settings-desktop-audit.md)：66 个细项、12 组发现；自启、证书、UI、热键、系统代理、更新、WebDAV 等。

## 主代理补充复现与修复合同

**AUD-ROOT-01：整树保存后的组版本不同步。** Rust `save_settings` 推进所有 SETTINGS_GROUPS 计数；Dart `saveDocument` 仅更新整树 revision，保留旧 groupRevisions。下一次 saveGroup 即用旧版本提交。修复须返回完整计数或成功后重新读取权威设置，并保持监听副作用有界。验收顺序是“完整保存→主题/托盘/更新开关等分组保存→重开”，不能把中间手动 load 当用户必需步骤。

**AUD-ROOT-02：TUN void 完成被当作成功。** 即 TUN-A03；一个真实 controller 的拒绝结果只写 state.error，Future 正常完成。结果类型必须承载本次 operation 的成功/失败/未知结果，避免读后来其它操作的 error 作为判断。开启和关闭均需核对实际租约及 applied revision。

**AUD-ROOT-03：加载异常生成可编辑默认配置。** `SettingsController.load:111-117` 的 catch 在生产也会执行；默认文档被标为 loaded=true。独立窗口的“空快照阻止提交”无法挡住一份非空伪默认文档。修复为加载失败态禁止提交；纯测试默认值改由显式注入提供。加载失败后不能以旧或默认快照继续保存。

**AUD-ROOT-04：不可用存储仍允许组保存。** `AppEngine::save_settings_group` 缺 guard_storage。pure probe 在专用 scratch 中证实它在 load 拒绝时仍成功写 guiNConfig。修复须让所有变更入口走同一存储守卫；失败应不落盘、不推进设置/期望修订、不声称成功。

**AUD-ROOT-05：平台失败漏出总结果。** SettingsController 只汇总内核和自启，系统代理/PAC listener 结果无对应返回值。修复须按操作采集“保存/内核/平台/重启”分阶段结果；若代理应用失败，保持“已保存、平台未应用”的失败与重试。sysproxy 去重还应涵盖实际配置内容，不只 mode/session/port。

## 建议执行顺序

1. **修保存与结果合同**：捕获草稿版本、同步分组版本、存储失败守卫、加载失败态、自启幂等重试和平台失败回传。先将本次 6 个正确预期 Flutter 断言全部变绿，再核对真实 Rust/FRB/SQLite 结果；不要改成接受错误行为的断言。
2. **修 TUN 可运行合同**：长期租约与心跳、唯一 provider、完整权限覆盖、提升进程 ready/退出检测、只读实际事实贯穿 FRB、取消和失败保旧。然后明确地址/路由所有权和 adapter generation，统一名称与 IPv6/进程保护输入。
3. **补实际设置消费者**：Happy Eyeballs 开关、合法 Fragment 取值、TLS 根来源、HWA/日志、Mihomo merge、外部路由模板、更新空选择语义及其它矩阵缺口。先比较冻结原版生成结果，再从正式 UI 经真实桥接到实际效果。
4. **统一 UI 状态和多窗口行为**：canonical 设置与 ui_state/INI 的迁移优先级；列宽、列顺序、分隔比例、窗口尺寸、主题/字号/语言。恢复/导入和彻底重开均须验证，失败不能无声退回另一状态源。
5. **完成受控平台验收**：正式 UI 开启 TUN→至少持续 60 秒→改参数/切节点→关闭→故障/睡眠恢复，核对 DNS、IPv6、排除规则和清理；另验系统代理/PAC、自启、热键、DPI 与托盘。仓库禁止本机路由/系统代理写入，真实路由接管仍在具备授权的隔离环境执行。代码缺口可先修，未进行的 OS 验收保留 blocked。

冻结上游内核作为外部程序使用；这些修复落在客户端计划生成、进程管理、平台协调和 Flutter 操作层。

## 本次执行证据

| 命令/检查 | 实际结果 | 含义 |
|---|---|---|
| Flutter `root_contract_repro_test.dart`，`--no-pub --reporter expanded` | 4 项正确预期合同全部 FAIL，退出 1 | 复现组版本、TUN 假应用、加载伪成功、系统代理假成功；不是编译失败 |
| Flutter `desktop_repro_test.dart` | 2 项正确预期合同全部 FAIL，退出 1 | 复现自启重试和旧草稿覆盖 |
| Flutter `tun-ui-contract_test.dart` | 1 项通过，退出 0 | 断言当前错误行为以记录复现，不是产品验收通过 |
| Cargo `tun-contract-repro`，`--offline --locked` | 退出 0，记录约五秒清理及双 TUN provider | 原生产代码的纯合同/配置复现 |
| Cargo `engine-probe`，`--offline --locked` | 退出 0，记录开关忽略、校验差异、上下文遗漏及存储守卫绕过 | 纯生成/专用 scratch 检查，无真实核心/OS 副作用 |
| Python `assemble_matrix.py` | 180 unique IDs，无缺失/重复，状态范围检查通过 | 证明审计台账覆盖；不证明功能全有效 |

日志和可复现源码全部保留在本目录。实际命令的完整参数见各分册；未运行全量 cargo/Flutter 发布门禁、原生 UI、真实 TLS/代理流量和系统写入，未重打发行包。Oracle skill 在当前技能目录未找到，未启动有费用的 API 审查；第二意见由上述并行分册与主代理交叉复核完成。
