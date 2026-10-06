# 运行生命周期与 TUN 综合审计（2026-10-06）

基线：`a7aa0a5`。冻结原版：v2rayN 7.25.4 / `7d6a967c18c697f28dc6917122ed3a4993fcf336`。生产源码只读；本轮只新增此目录内的审计源码、合成夹具与日志。所有结论按本次源码和本次执行结果给出，不复用旧完成口径。

结论：正常 Xray 启动、预检拒绝保留旧会话、真实端口切换、正常停止可运行；故障后的实际状态、快速用户操作顺序、TUN 长会话和清理恢复仍有发布阻断缺陷。不能称为完整稳定移植，不能把剩余工作只归结为真实 TUN 授权验收。

## 本轮范围和证据边界

- 阅读当前 UI `runtime_controller/runtime_bridge/tun_toggle/status_bar_view` → bridge/app engine → net_host 的启动、就绪、切换、停止、journal、事件流、helper → helper lease/Windows backend。前置卡为 `docs/repair/tasks/R4-01.md`、`R4-04.md`、`R4-05.md`、`R4-25.md`，以及 `docs/tasks/RR-10.md`、`R3-04.md`。相关台账为 `F-CORE-005/006`、`F-TUN-001..005`、`ACT-STAT-003`、`FLD-CFG-095..105`。
- 真实运行的是当前生产 `services/net_host/src/main.rs` 编译出的独立审计 host，以及锁定工具目录的 Xray 26.3.27。审计包 version=0.0.0、debug 构建，不能代替打包 RC 的 Flutter→FRB 完整 GUI 验收。
- 真实流量只做 `127.0.0.1` SOCKS5 greeting，没有远端连接。端口 11977/11978 在启动前由 TcpListener 探测，均 ≥11808。唯一命名管道与 run_root 均位于审计范围。无 TUN 配置、无系统代理/默认路由写入、无真实用户数据。
- 故障注入只终止本轮会话记录的 Xray：核对 PID、进程创建时间、可执行路径后持有进程句柄，再停止该句柄；未按名称杀进程。最后停止自己持有的 audit host。所有运行目录保留合成证据。
- Flutter/Rust mock 仅用于故障分支和并发控制。它们不证明 OS 地址/路由清理真实成功、UAC 正常、IPv6 不泄漏或 TUN 流量已经被接管。
- 未进行生产 TUN、路由/DNS 接管、真实权限、睡眠恢复、Flutter GUI 连续操作、FRB 原生真实 TUN、macOS/Linux/ARM64 验收。未运行 Oracle（此环境未找到该 skill，不调用收费 API）。

## 已执行场景与结果

| 场景 | 本轮实际结果 | 证据 |
|---|---|---|
| 当前真实 Xray 启动 | Running；11977 SOCKS5 greeting 实际响应 `[05,00]` | `real-loopback.log`、`real-loopback.ps1` |
| 无效配置切换 | `E_INVALID_PLAN error.config_check_failed`；旧 PID 42756 / 11977 仍服务 | 同上 |
| 切换新合法配置 | 新 PID 23988 / 11978 服务；旧 11977 不再服务 | 同上 |
| 正常 stop/shutdown | PID、ports、session 被撤回；audit host 已停止 | 同上 |
| 就绪后核心退出 | 3 秒后仍发布 Running / 旧 PID / 11978；实际 SOCKS 不可达，确认 RUN-01 | 同上 |
| helper 活跃租约空闲 | 虚拟 6 秒仍保留；虚拟 24h 后删除活跃地址租约，确认 TUN-A01 残余问题 | `rust-tests.log`，真实 server + duplex + FakeBackend |
| apply job/operation | 运行后及停止后 apply job 仍 active；operation ID 带 `:job-…` | `rust-tests.log` |
| 事件订阅落后 | 实际 EventBus 2049 条合成 burst → Lagged；生产转发循环没有处理此分支 | `rust-tests.log`；没有跨命名管道压力实测 |
| TUN actual 三种状态 | desired=false 活跃 lease、候选错误活跃 lease、dry_run lease：正确期望断言均失败 | `runtime-contracts.log`，前 3 个失败 |
| 启動 A→停止→启动 B | 实际 controller 派发 `[apply:a, apply:b, stop]`，最终 Stopped；最新启动意图丢失 | `runtime-contracts.log` |
| apply 超时但 backend 已运行 | backend Running，UI Stopped，reconcile=false；正确期望失败 | 同上 |
| 新 bool apply 合同 | apply 失败后 toggle.ok=false / runtimeApplied=false，1 条通过 | 同上；证明此部分修复有效 |
| A 正在 apply、desired 移到 B | 实际 AppEngine 发布 A 的会话为 B；正确期望失败 | `applied-target-race-contract.log` |
| net_host 清理失败 | 注入 cleanup Err 后仍返回 Ok，journal 被删；正确期望失败 | `cleanup-journal-contract.log` |
| helper 清理失败 | ResetTunAddress Err；FakeBackend 仍有 if=7，但 ownership 被清空/closed=true；二次调用不重试 | `cleanup-helper-contract.log` |
| 现有 TUN 恢复合同 | 14/14 通过，均 FakeHelperLink/合成 journal；不覆盖失败后保留 ownership | `tun-recovery-tests.log` |
| 现有切换失败恢复 | 2/2 通过，持有项目 job 的 `.cmd` stub；恢复失败如实 Stopped、能恢复则 Running | `switch-rollback-tests.log` |
| 无 PID journal 重开清理 | 4/4 通过，合成目录/记录 | `reopen-journal-tests.log` |
| LegacyProtect 单 TUN provider | 现有正确预期测试 1/1 通过，主核无第二 TUN、前置 sing-box 有 TUN | `tun-single-provider-test.log` |
| 当前 helper 原有 idle 测试 | 单项复跑失败，2 秒超时；属于旧 fixture 没给新 idle_timeout 的门禁回归 | `helper-idle-regression.log` |

`rust-tests.log` 的 3 个通过测试是“记录现状”的观察测试：其中两项恰好确认缺陷仍在，不能算发布门禁通过。`runtime-contracts.log` 是正确行为断言，1 pass / 5 fail。三个新增 Rust 正确行为合同均 fail。独立审计包因只引用 EventBus 部分方法有一项 dead_code warning；未据此宣称独立包 clippy 零警告。

## 确认缺陷与具体修复

### RUN-01 / P1 / identified：主核就绪后退出，运行事实永久留在 Running

定位：`services/net_host/src/session.rs:1490-1540` 在就绪时存 `Session.child` 并提交 Running；`:589-610` 直接返回缓存 detail；`services/net_host/src/server.rs:226-252` watchdog 只管理 client lease/发 host heartbeat，不检查主核或 sidecar 生存。`session.rs` 的 `try_wait` 只出现在启动就绪检查和临时测速 session 查询，没有常规 Running session 的退出监视。

复现：见真实日志，记录自己的核心退出后等待 3 秒，`state=running,pid=23988,ports=[11978],error=null`，SOCKS greeting 失败。`host_alive=true` 是真实的 host 存活，不能拿它代替 core 存活。当前 AppEngine 的 applied-session reconcile 信任这个缓存，后续监控/代理端点会继续引用已不服务的入口。

修复：给每个 managed generation 建立主核/普通 sidecar/helper sidecar 的退出通知，按 session+process identity 防迟到事件。主核退出必须撤回实际 endpoint、写错误、结束该 generation，逆序释放属于它的资源并保留无法确认清理的记录。sidecar 死亡需按拓扑定义降级/停机，不继续报告完整 Ready。恢复只能重启明确的上次良好 plan，不能在 UI 伪造 Running。

验收：当前 loopback harness 从 OBSERVED 变为正确断言：注入自有主核退出后规定期限内非 Running、无已退出 PID/旧端点；helper/sidecar 退出同样有合同；旧 generation 的退出事件不得污染已启动的新会话。原版 `CoreManager.cs:334-363` 检查启动早退，`ProcessService.cs:132-142` 的 Exited 仅撤日志 handler；本项依据本项目“真实运行事实”合同，**不宣称原版已有完整健康监控**。

### RUN-02 / P1 / identified：有界队列改变用户命令顺序

定位：`apps/desktop/lib/features/runtime/runtime_controller.dart:275-305`；`:280` 总优先 `_pendingApply`，而不比较 stop/apply 时间或 intent generation。

复现：A apply 在途；用户 stop；用户再 apply B。当前实际调用 A→B→stop，最终 Stopped。用户最后明确点击的是启动 B。反向 stop-after-B 也必须真正停在 stop，不能用一个固定优先级同时满足两种操作。

修复：给命令分配单调 intent ID。定义合并/淘汰规则，保留“最后一次显式 start/stop”意图；stop→start 必须先 stop 后 start 或明确 supersede 旧 stop；start→stop 必须最终 stop。被合并的 waiter 返回明确 superseded/终态结果，不让旧用户动作借新动作的结果冒充成功。保持队列有界、不可取消的在途 apply 在安全点收尾，之后执行最新 intent。

验收：修复本轮失败用例；增加 A(start在途)→stop→B(start)、A→B→stop、双 stop、双 F5、B 被 C 替换、失败后 retry 的顺序/最终状态合同。原版 `MainWindowViewModel.cs:661-750` 用 semaphore + `_hasNextReloadJob` 串行并重读最新 default，R4-04 明确要求连续启停落到可信最终状态。

### RUN-03 / P1 / identified：apply 结果未知没有 reconcile

定位：`runtime_controller.dart:315-392`。stop 错误分支 `:329-334` 对 timeout/unavailable/bridge 重新 snapshot；apply 分支 `:367-373` 直接记 error 返回，异常分支也不核对。命名管道调用方超时并不证明服务端没有完成 apply。

复现：故障 bridge 接受并运行，但返回 E_TIMEOUT。实际 controller 仍 Stopped，reconcileNeeded=false。正常服务端 apply 的响应在启动就绪后发出（`server.rs:122-130`），超时/丢响应与执行成功同时发生是合法故障模型。

修复：未知 outcome 必须进入 reconcile，按 raw operation/session/plan revision 向服务端核对，保留原命令错误作为反馈，实际状态另存。首次核对本身也失败时显示未知/恢复中，禁止把未知当 definitive Stopped/成功。应用执行和 UI 等待都要有界，迟到结果只能更新同一 intent/session。

验收：本轮用例转绿；分别覆盖 apply已执行/未执行/服务端仍 Checking、response丢失、reconcile二次失败；不得为“重试成功”悄悄触发第二个 core。R4-01/R4-04 的结果未知合同目前未满足。

### RUN-04 / P1 / identified：accepted plan 的 applied 目标读成后来的 desired 目标

定位：`crates/application/src/engine.rs:1825-1829` 在阻塞 `self.runtime.apply(&plan)` 返回后才执行 `*apply_target=self.active_profile()`。计划本身没有 target profile ID 的实际会话字段。`reconcile_applied_session:1928-1955` 再把该变量作为真实活动节点发布。

复现：本轮实际 AppEngine + 受控 RuntimeClient gate，A plan/revision 已提交、尚未返回时 set_active(B)，放行 A accepted；snapshot 的 `applied_session.active_index_id=synthetic-b`，实际接受的 plan 是 A。正确断言失败。没有进程/IPC/OS。前端节点激活会在进入 apply 队列前持久化 default，因此快速操作可触发这种时序。B 后续失败并保留 A 时，错误归属可持续存在。

修复：immutable plan/command 必须带明确 target ID；target/core/endpoint/hash/revision/session 在 net_host 提交成功时一起成为 actual descriptor。至少在提交前绑定明确 target，不允许 accepted 后读取变化的 desired default；仅靠提前读 `active_profile()` 仍不能覆盖显式 applyTarget(B) 与 active(A) 的 API 合同，最好使用 plan 的真实 target 字段。

验收：本轮 A→desired B 的正确合同转绿；显式 target != desired、B失败保旧A、旧A回滚恢复、快速重开时 actual descriptor 均一致；监控流量不能记入 desired B。关联 R4-01/R4-05、F-CORE-005、desired/applied 分离约束。

### RUN-05 / P2 / identified：apply job 无终态，operation 相关 ID 不可往返

定位：`engine.rs:1839-1843` 只 `jobs.start("apply_runtime")`，返回 `"{operation_id}:{job_id}"`；`engine.rs:1918-1920` operation 查询原样透传；`net_host_client.rs:293-309` 原样发送 GetOperation；`net_host/session.rs:447-453` 查 raw ID hashmap；`:1512-1521` 实际已经把 raw operation 记为 Done。不存在对应 apply job 的 finish 接线。

复现：实际 AppEngine + NullRuntimeClient，Running后和stop后 snapshot 均有一个 active apply job；返回 `op-synthetic-accepted:job-00000001`。复合 ID 无 strip/map，无法用 apply 返回值查 raw runtime operation（这一往返后果由当前读写源码确认，未做跨管道 GetOperation 端到端）。

修复：DTO 分离 operation_id/job_id；绑定同一不可变 correlation，终态事件/查询可靠地结束应用 job。若 apply IPC 的 Accepted 实际已等到 Ready，应明确该同步合同并当场完成 job；若转成异步 Accepted，需要真实 terminal listener。取消必须反映已提交 safe point，不能只让应用 job 标 CancelRequested 却不通知实际运行面。

验收：成功/失败/取消/超时 reconciliation 后无悬空 job；apply 的原始 operation_id 可直接 get_operation→Done/Failed；重开后的查询结果真实，任务面板不增长无限 active apply 项。

### RUN-06 / P1（有条件）/ identified：事件落后时转发器悄悄退出

定位：`net_host/events.rs:14,26-30` broadcast capacity=1024；`net_host/server.rs:147-152` `while let Ok(event)=receiver.recv().await` 遇 Lagged 即结束；客户端 `net_host_client.rs:465-495` 只有 pipe read结束才重连，没有该 forwarder 结束通知。

本轮确认：实际 EventBus 在2049条合成事件未消费后返回 Lagged，随后仍可继续收事件；生产循环会把它当永久 EOF。跨管道堵塞是否在普通用户负载发生，**未实测**，不能声称所有用户一定断流。大量 core log/慢订阅者具备触发条件。

修复：显式处理 Lagged：发布 resync-required/取 authoritative snapshot，恢复 seq/epoch 后继续；或关闭该订阅连接使客户端可靠重连。控制事件与高频日志采用不同 backpressure/coalescing 策略，不让日志淹没 actual state。订阅 from_seq/epoch 若不支持 replay 要明确 reset，不能仅回显参数。

验收：安全 synthetic slow subscriber + 事件 burst 的真实 pipe 合同；跨 epoch、lag 后最新实际状态仍送达，UI 必须知道何时重新取 snapshot。

### RUN-07 / P1 / identified：TUN 清理失败被说成成功，并删除恢复所有权

定位：
- `net_host/tun_lease.rs:141-163` 吞 `link.cleanup` error 后删 journal 返回 Ok；`:201-217` 重开 cleanup 的非 E_TUN_HELPER_UNAVAILABLE 错误也被算 cleaned 并删记录。
- `net_host/session.rs:653-680` 在清理前先 `detail.tun=None`，take掉 lease/link，忽略 cleanup 结果；缺 link 时也直接删 journal。
- `helper_client.rs:769-780` 吞 RemoveRoutes 错误，关闭 pipe 后立即 Ok，没有 adapter reset/lease cleanup 的确认。
- `privileged_helper/server.rs:294-351` 把 lease closed=true，take资源后失败仅记 audit，仍清列表；二次 on_disconnect 不重试。
- WindowsBackend `windows.rs:433-455` reset 在删除各地址前 remove其 registry record，后续失败同样丢失重试所需的完整配置。

本轮正确行为测试均失败：
1. 调用实际 cleanup_tun_lease，故障 link.cleanup=Err：实际 result_ok=true，recovery_journal_retained=false（仅合成文件）。
2. 实际 HelperServer + FakeBackend reset故障：实际 fake接口仍[7]，但 owned_tun_count=0/closed=true，二次 cleanup 没有重试。

这证明故障合同错误，不证明本轮宿主存在残留地址/路由；本轮没有做任何 OS 写入。

修复：清理结果用 Cleaned/Pending/Failed 或结构化错误；保留每个未确认释放资源的持久化 ownership。端点可先撤回以免复用死入口，但实际网络资源状态应显示“清理中/失败待恢复”，不能等价于未启用。每项删除成功才移除记录；部分成功记录进度，允许幂等重试。pipe disconnect 的 helper cleanup 必须可靠确认或由 journal+OS 只读核对后收敛，不能假定“另一个层一定会清”。退出、失败回滚、重开清理共用这一结果合同。

验收：本轮两个失败合同转绿；地址/route每个删除位置注入错误、部分成功后重开、helper丢失/拒绝、cleanup超时、rollback根错误+cleanup次错误同时可见；pending journal保留，重复清理只作用于自有资源。真实 OS 清理由授权隔离环境另验。

## 旧 TUN-A01..A09 在当前基线的状态

| 条目 | 当前判断 | 当前定位/下一步 |
|---|---|---|
| TUN-A01：空闲5秒回收活跃lease | **旧5秒问题已修；持续24h长会话仍有代码缺陷**。真实 helper server 的虚拟时间合同已确认6秒保留、24h清理；并非真机等待24h | `privileged_helper/server.rs:493-502` 使用 idle_timeout；默认 `:67` / main `:69`为24h。net_host watchdog仅向UI发heartbeat；HelperLink.available的Ping只在 apply/预检查调用，无周期helper心跳。需长会话keepalive/lease renewal+失联reconcile，不仅延长超时。request_timeout还未用于分开限制半帧/请求执行 |
| TUN-A02：Legacy双provider | **本项单provider修复有效**，本轮当前正确测试1/1通过 | `engine.rs:2931-2947` 前置provider存在则禁主核TUN。但原版 Builder`:168-181` strict-route时还清主核 BindInterface/SendThrough；当前 codegen`:370-371`仍投射。该相邻迁移差距需新增 strict+legacy+bind/send 验收，不重新宣称双provider仍存在 |
| TUN-A03：toggle假applied、lease DTO、actual标签 | **bool结果和DTO贯通已修，标签仍错误**；本轮bool1pass、标签3fail | `bridge_api/contract.rs:75,84-88`、engine`:448-453`、runtime_bridge`:348-352`已含lease/dryRun；`tun_toggle.dart:54-64`先desired false、再error、再isRunning，忽略dryRun。改为实际lease/actual-state优先、候选错误另展示。关闭失败必须显示仍启用/待清理；dry-run显示模拟。UAC取消后持久化desired仍true的行为，与原版 StatusBarVM`:439-454`回false不同，应补真实cancel/重开合同 |
| TUN-A04：主核自身TUN权限 | **仍开放，confirmed source gap；真实权限结果未验证** | `session.rs:954`只计算 sidecar elevated；主核`:1348-1370`始终普通Command.spawn。不能因 Legacy sidecar成功而宣称sing-box自身或非legacy Xray主核TUN权限已完整。以plan权限+实际provider统一决定helper启动；非TUN核不能被无故提升 |
| TUN-A05：提升sidecar readiness/exit | **仍开放，confirmed source gap；真实设备/核心退出未验证** | `session.rs:1595-1677` helper返回(handle,_pid)后立即Ok；`:1699-1702`提前返回绕过普通sidecarready。只发现适配器不能证明新core进程、SOCKS或TUN流量Ready，旧适配器还能造成误判。要保留pid+creation identity、helper查询/退出事件、拓扑就绪条件。与RUN-01联动 |
| TUN-A06：IPv6/进程保护上下文 | **仍开放，confirmed codegen consumer gap** | `application/codegen.rs:45-59`默认CodegenSettings；整个生产assembly未赋 `has_global_ipv6_address/protect_core_executables`，保持false/空。`config_codegen/input.rs:464-465`、xray/inbound`:89-99`、xray/routing`:166`、singbox/routing`:93`需要这些输入。原版Builder`:52-53`有实际IPv6探测与Xray/sing-box保护列表。应补只读平台能力探测/已解析运行exe列表并传入main/sidecar。不能据此宣称本轮实际IPv6泄漏或回环已经发生 |
| TUN-A07：地址所有权/部分失败 | **仍开放，confirmed conditional fault gap；未做实际API故障** | `windows.rs:415-430`逐地址create，全部成功才写registry；中间失败时已成功部分不被记录，server`:243-250`也只在整个backend成功后记lease。reset`:433-455`先删registry。应按地址/route记录own/existing/created结果，部分失败可逆且不误删core或其他owner已有资源；与RUN-07统一 |
| TUN-A08：计划中已存在interface index | **条件风险仍开放，未真机复现** | `tun_plan.rs:41-59`计划前发现旧index，resolved spec路径在core创建新接口前可应用旧index；停旧core再创建设备后index可能变化。deferred路径已有按名发现，需在新provider Ready后重新绑定/核对ownership，不能把旧index当新generation实际事实。不同内核是否重建设备/索引变化需隔离实测 |
| TUN-A09：adapter env override与生成器不一致 | **默认v2rayn-tun已对齐；非默认override仍是P2条件缺口** | `tun_plan.rs:41-47`支持V2RAYN_R_TUN_ADAPTER；生产assembly在`application/codegen.rs:426`将tun.name固定为DEFAULT_TUN_ADAPTER，`config_codegen/singbox/inbound.rs:29-36,61`消费该name。生成器自身允许name输入，缺口在assembly没有传入同一hints。自定义名需同一plan context进入core和helper；或明确拒绝override并移除其支持口径。没有设置该override的普通用户不能计为默认故障 |

当前 `docs/evidence/tun-settings-audit-2026-10-05/fixes-phase-1-2.md` 自己列 A04/A05/A06 未完成，与本轮读取一致。旧 R4-25 的“无desired立即未启用”验收不能代表actual真值；本轮保留正确断言，不迁就错误实现。

## 门禁回归的准确归因

本轮独立重跑：

`cargo test -p privileged_helper --locked --offline --test loopback idle_connection_times_out -- --exact --nocapture`

0 passed / 1 failed，2.00秒：`services/privileged_helper/tests/loopback.rs:178` “timeout response should arrive: Elapsed(())”。`:36-44` fixture 只设request_timeout=150ms，未设新增idle_timeout，所以实际继承24h，却期待2秒内收到idle错误。这是**旧测试fixture与新idle合同不匹配**；不是5秒缺陷仍旧，也不是能忽略的发布门禁失败。修fixture时显式配置短idle来测idle expiry，再保留活跃lease/持续keepalive正确合同，不能把“24h活跃lease回收”当作预期放宽。

parent本轮完整workspace门禁日志位于上一级 `checks/cargo-test.log`；本报告只报告自己单项实际执行，不代称完整suite已绿。

## 重开、取消、并发、性能尚待完整验收

- 已安全运行14条TUN journal/retry合同、4条无PID stale journal恢复、2条stub切换回滚。它们证明正常mock分支；RUN-07证明失败后pending/ownership语义仍不完整。真实GUI退出→秒重开/磁盘锁/单实例/actual session重连均未验证。
- `engine.rs:1934-1946` 重连没有apply_target时选择不伪造desired，方向正确；但net_host/RuntimeDetail尚未给出actual target/core/full endpoints，R4-05的“重连返回actual目标/core/端点”接口仍不完整。应在RUN-04的actual descriptor下沉后补端到端，不能把“发布None而非造假”算完整功能。
- `net_host_client.rs:339-343` cancel明确NotCancellable；application job cancel与实际运行面缺correlation（RUN-05）。UAC取消/stop在启动准备中/超时执行已完成均应有独立最终状态合同。禁止用取消UI提示代替真实停止。
- 当前UI队列有界/请求worker有gate是有效基础。`runtime_controller.dart:289`只在执行前检查commandDeadline，真实执行超时来自IPC；没有本轮完整输入→首帧/FRB/配置预检/平台写入的延迟分段测量。不能把debug backend脚本耗时当GUI响应性能。
- 多个IPC连接上的apply/stop是否被host全局串行，以及进程退出事件与stop/apply之间race，当前未注入真实并发，不得宣称线程安全已完整验证。需要按同一generation/intent服务端合同测试；单个controller有队列不足以覆盖多个控制来源。

## 建议修复顺序及完成门槛

1. **先固定运行实际事实**：RUN-01退出监控；RUN-04 immutable target/core/hash/revision/session/endpoint descriptor；正常/失败/恢复/重连使用同一事实源。不要把UI状态更改当修复。
2. **修命令结果和顺序**：RUN-02有序intent；RUN-03 unknown outcome reconcile；RUN-05拆分operation/job与terminal接线；RUN-06 lag恢复。先让本轮正确失败合同转绿，再补真实pipe迟到/丢响应/多控制来源。
3. **固定TUN运行和释放所有权**：TUN-A04主/侧核统一权限，A05统一ready/exit，A01持续helper会话；RUN-07/A07按资源确认清理、pending持久化、重开幂等恢复。完成后才有资格真实auto-route接管。
4. **补TUN生成语义**：A06平台IPv6/可执行进程保护，legacy+strict-route bind/send清除，A08新generation的adapter identity，A09单一adapter名上下文；每项做到生成配置正确+指定核校验，不只字段保存。
5. **完整用户验收**：原版与移植版同一合成节点/设置，从GUI入口启动、切节点、无效节点/缺核修复重试、快速启停、TUN开关拒绝/取消/失败关闭、真退出重开；Flutter→FRB→SQLite→net_host→配置/平台效果全链记录。最后授权隔离机测试TUN auto-route/DNS/IPv4/IPv6/排除/崩溃睡眠恢复/清理重开。所有P1关闭并通过当前干净构建门禁后，才重新评估“稳定完整”。

## 可复跑命令与实际退出码

所有Rust命令从仓库根执行；均`--offline`，无额外网络下载。

```powershell
cargo build --manifest-path docs/evidence/complete-port-audit-2026-10-06/runtime/rust/Cargo.toml --bins --locked --offline --target-dir target/runtime-complete-port-audit
& ./docs/evidence/complete-port-audit-2026-10-06/runtime/real-loopback.ps1
```

build exit=0；script exit=0（记录已复现缺陷，非正确验收通过）。初次审计client缺合成session_token导致脚本setup失败（`initial-harness-attempt.log`）；只修审计client后得到上述正式证据，初次未spawn核心。这个setup错误不计产品缺陷。

```powershell
cargo test -p privileged_helper --locked --offline --test loopback idle_connection_times_out -- --exact --nocapture
cargo test -p net_host --bin net_host --locked --offline tun_lease::tests -- --nocapture
cargo test -p net_host --bin net_host --locked --offline journal::tests -- --nocapture
cargo test -p net_host --bin net_host --locked --offline rr10_failed_switch -- --nocapture
cargo test -p application --lib --locked --offline pre_socks_legacy_sidecar_uses_base_port_and_real_socks_config -- --nocapture
```

分别exit=1、0、0、0、0（PowerShell cargo输出管道包装取实际 `$LASTEXITCODE`；直接cargo测试失败通常返回101）。

独立观察测试首次运行命令为 `cargo test --manifest-path .../runtime/rust/Cargo.toml --lib --offline --target-dir target/runtime-complete-port-audit -- --nocapture`，当时库仅3个观察测试，3/3通过。当前独立库增加了只读引用的生产模块/新合同，复跑应精确选择本轮测试，避免把生产模块自带测试混计：

```powershell
$auditManifest = 'docs/evidence/complete-port-audit-2026-10-06/runtime/rust/Cargo.toml'
cargo test --manifest-path $auditManifest --lib --offline --target-dir target/runtime-complete-port-audit tests::active_helper_lease_is_still_cleaned_at_24h_without_a_heartbeat -- --exact --nocapture
cargo test --manifest-path $auditManifest --lib --offline --target-dir target/runtime-complete-port-audit tests::completed_apply_leaves_a_running_job_and_compound_operation_id -- --exact --nocapture
cargo test --manifest-path $auditManifest --lib --offline --target-dir target/runtime-complete-port-audit tests::slow_control_subscriber_hits_the_unhandled_lagged_branch -- --exact --nocapture
cargo test --manifest-path $auditManifest --lib --offline --target-dir target/runtime-complete-port-audit tests::cleanup_failure_should_keep_recovery_record -- --exact --nocapture
cargo test --manifest-path $auditManifest --lib --offline --target-dir target/runtime-complete-port-audit tests::helper_failed_cleanup_should_retain_owned_resources_for_retry -- --exact --nocapture
cargo test --manifest-path $auditManifest --lib --offline --target-dir target/runtime-complete-port-audit tests::applying_a_while_desired_moves_to_b_must_still_publish_a -- --exact --nocapture
```

后三条精确命令本轮已执行，各exit=101，正确期望分别“清理失败不能成功/丢journal”“失败资源保留供重试”“A实际plan不能标B”。前三条在初次3测试命令执行，本轮没有为文档补跑重复测试。

Flutter命令从`apps/desktop`执行：

```powershell
flutter test --no-pub ../../docs/evidence/complete-port-audit-2026-10-06/runtime/runtime-contracts_test.dart
```

exit=1，1 pass / 5 fail。测试只使用controller/purelabel+ProviderContainer故障桥，不启动RustLib、不读真实数据、不触碰平台效果。

本轮新增文件：本README、`runtime-contracts_test.dart`、`rust/{Cargo.toml,Cargo.lock,src/lib.rs,src/client.rs}`、`real-loopback.ps1`、上述日志与合成run目录。未修改生产源码、台账、任务卡，未提交commit。
