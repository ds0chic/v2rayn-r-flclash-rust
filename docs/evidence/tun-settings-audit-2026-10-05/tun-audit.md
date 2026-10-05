# TUN 功能审计（当前工作区，2026-10-05）

审计对象：HEAD `672e666` 加开始审计前已经存在的工作区修改；特别是 `application/codegen.rs`、`net_host/{session,helper_client}.rs`、`privileged_helper/{main,windows}.rs`。这些修改未覆盖、未回退。本次只增加审计报告和合成复现文件，未修改生产代码。

冻结原版：v2rayN 7.25.4，commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`，源码根 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`。读取了 AGENTS.md、R4-25 任务卡、TUN 相关 feature/action/field 台账、当前上游 `CoreConfigContextBuilder.BuildAll` 与 `StatusBarViewModel.DoEnableTun`。

结论：TUN 不能按“只剩真实 auto-route 验证”的口径验收。当前有已复现的 helper 生命周期缺陷、主核/前置核配置拓扑缺陷和 UI 误报；尚有提升权限、就绪检查与地址所有权方面的代码缺口。以下 P1 为发布阻断，不代表本次实际造成宿主断网。本次没有启动真实内核、没有监听任何端口，没有调用 WindowsBackend，没有修改系统代理、适配器、路由、注册表，也没有读取用户数据。

## 本次实际复现

| 用例 | 方法 | 本次结果 | 能证明什么 |
|---|---|---|---|
| 空闲 helper 清理活跃 TUN | 原生产 `serve_connection` + `tokio::io::duplex` + `FakeBackend`，默认 5000 ms；客户端保持打开 | 5010 ms 后接口 `[7] → []`，提升核 `running=true → false`，`stopped_handles=[1000]` | 真正生产 server 的闲置会话被清理；不证明真实 OS 路由效果 |
| LegacyProtect 主核和前置核都带 TUN | 原生产 `AppEngine::in_memory/build_runtime_plan_with_hints`，合成 Xray 节点、TUN=true、LegacyProtect=true、端口11970 | Xray 和 pre-socks sing-box 共两个 TUN provider，均名为 `v2rayn-tun` | 配置拓扑与冻结原版不同；不证明实际设备冲突错误码 |
| 关闭失败仍声称应用/未启用 | 原生产 `RuntimeController` 与 `toggleTunDesired`，bridge 注入结构化拒绝并保持旧 Running 会话 | `error=E_CORE_NOT_FOUND`、旧 session 仍存在，但 `toggle.ok=true`、`runtimeApplied=true`、实际标签 `未启用` | 生产 controller 把失败完成为 void，toggle/标签因此误判；不证明真实 TUN 租约 |

Rust 复现源：`tun-contract-repro/Cargo.toml`、`tun-contract-repro/src/main.rs`；日志 `tun-contract-repro.log`。Dart 复现源：`tun-ui-contract_test.dart`；日志 `tun-ui-contract.log`。测试断言记录当前错误行为，是审计复现，不是“符合产品要求”的验收测试。

## 确认缺陷与修复要求

### TUN-A01 / P1：保持 helper 连接不等于保持租约，默认空闲五秒即被清理

证据：`crates/ipc_contract/src/lib.rs:32` 的 `IPC_REQUEST_TIMEOUT_MS=5000`；`services/privileged_helper/src/main.rs:68` 用于 server；`server.rs:482-492` 对下一帧 `read_frame` 计时，超时退出循环，`:527` 调用 `on_disconnect`；`:309-325` 的 `CleanOwned` 删除租约地址/路由并停止提升内核。`net_host/helper_client.rs:643-727` 只有请求时 I/O，没有后台心跳；`:732-743` 的 `available/Ping` 只在申请时调用。`session.rs:765-768` 保持 link 字段不能阻止 server 的闲置超时。

本次复现已经证明：客户端没有断连，server 仍在五秒空闲后清理资源。工作区新增提升 sidecar 使用同一租约模型，因而也会在超时清理中被停止。net-host 没有相应持续 helper 会话检查，主核运行事实可能保持 Running；不能据此宣称 UI 所示 TUN 有效。

修复：分离请求帧读取 deadline 和长期租约 liveness；明确心跳协议、节奏、丢失次数、睡眠恢复与断连回收。空闲本身不能清理仍有有效 owner 的租约；真实 owner 丢失仍须回收。至少添加“保持活跃租约空闲超过两个原超时仍有效”“心跳丢失后有界清理”“清理导致 net-host TUN 状态撤回”三类合同测试。

### TUN-A02 / P1：LegacyProtect 同时配置两个 TUN 提供者

原版 `CoreConfigContextBuilder.cs:159-167` 在存在 pre-socks 时将主核 `IsTunEnabled=false`；严格路由时还在 `:168-181` 清掉主核 BindInterface/SendThrough。

本项目 `application/engine.rs:2917-2923` 在计算 pre-socks 之前生成主核；`:3025-3078` 另生成前置核，未关闭主核生成上下文的 TUN。`application/codegen.rs:422-435` 把同一 TunModeItem 映射给主核和前置核，工作区修改把名称统一为 `v2rayn-tun`。本次合成计划输出明确为两个 TUN provider。

修复：先冻结并计算完整 main/pre-socks 拓扑，再按不同上下文生成；只有前置核提供 TUN 时，主核必须剔除 TUN 并按原版严格路由规则调整绑定。测试须逐项断言主核无 TUN、前置核恰好一个 TUN、前置核 SOCKS 出站指向主核、启动顺序、DNS/进程保护合并；不能只断言 sidecar 文本包含 `socks`。

### TUN-A03 / P1：实际态丢失，关闭失败仍报“未启用/已关闭”

`application/snapshot.rs:67,101` 已携带脱敏 `TunStatus`，但 `bridge_api/api/contract.rs:67-85` 的 SnapshotDto 无 TUN 字段，`api/engine.rs:434-466` 丢弃 `s.tun`；Flutter `runtime_bridge.dart:26-85,311-327` 也无租约状态。

`tun_toggle.dart:37-38` 只等待 `Future<void>` 后返回 `runtimeApplied=true`。真实 `runtime_controller.dart:324-384` 收到结构化失败仅更新 state.error，最终完成 void waiter，不抛异常。`status_bar_view.dart:421-429` 只在开启时看错误；关闭有错误也发送 `tunSavedDisabled`。`tun_toggle.dart:46` 又依据 desired=false 立即显示 `未启用`，即使实际仍是旧会话。本次跨真实 controller 的故障注入已复现。

开启的任何错误还被 `_onTunToggle` 统一翻译成 `tunDenied`，掩盖内核缺失、配置错误等真实原因；UAC 取消后持久化 desired 保持开启，与原版 `StatusBarViewModel.cs:439-454` 的取消回退不同。

修复：把只读、脱敏、含 dry-run 区别的 TUN 实际事实贯穿唯一 FRB 生成；apply 返回结构化操作结果/operation id，toggle 等待对应操作完成并 reconcile；两种方向都检查实际结果。保存期望、申请中、已应用、失败保旧、回收失败、未验证须分别表达。授权取消按冻结原版还原开关/期望态。显示实际关闭只能以实际租约清理及对应运行修订确认。

验收缺口：`apps/desktop/test/r4_25_contract_test.dart:101-110` 把“switch off 即使 Running 也显示未启用”当正确预期；其授权拒绝测试只注入抛异常，未覆盖真实 controller 返回 void 的失败路径。R4-25 README 的“绝不显示成功/已关闭”结论不受当前实现支持。

### TUN-A04 / P1：新增提权只覆盖 sidecar，目标主核仍普通权限启动

工作区 `session.rs:954` 只给 `PreparedSidecar` 计算 elevated；`:1700-1703` 只在启动 sidecar 时走 helper。主核在 `:1330-1370` 一律普通 `Command.spawn`。sing-box 是目标核且没有 pre-socks 时，没有 helper RunElevatedCore 创建其 TUN；Xray 独立 TUN 同样未提升。检测函数 `:367-380` 只识别 sing-box JSON `type=tun`，未识别 Xray `protocol=tun` 或 mihomo YAML。

这是权限接线的代码确认缺口；本次未执行普通用户创建真实 Wintun，也未宣称实测某错误码。原版 Windows 通过 RebootAsAdmin 保证创建核的进程权限；本项目采用最小 helper 可以改变实现机制，但必须覆盖所有实际 TUN provider。

修复：根据完整计划的唯一 TUN owner 决定提升目标主核或前置核；不要只根据 sidecar 身份判断。处理不同内核配置/能力，禁止不支持路径静默尝试。提升后的核心生命周期、PID 身份、配置检查、端点就绪、日志、退出监控必须与普通 managed core 具有等效合同。

### TUN-A05 / P1：新增提升 sidecar 未检查核心就绪或持续存活

`session.rs:1595-1680` 的 `start_elevated_sidecar` 在 helper 返回 `(handle,pid)` 后直接返回成功，且丢弃 pid；`:1700` 提前返回，跳过普通分支 `:1774-1799` 的 SOCKS readiness。提权后台无法通过现有 sidecar 子进程句柄监控，也没有挂载 core.log reader。这不仅是颜值或日志缺失：创建进程成功不证明核心启动成功/配置生效。

本次只确认代码合同缺失，未以真实内核失败宣称已经发生。deferred 路径的接口发现只能证明名称可见；旧适配器/早退核心仍不能证明本次进程已应用配置。

修复：helper 返回可查询的进程身份/状态，必要时传回脱敏 ready/error；提升路径也执行对应端点或 TUN 健康探测、日志归档、退出检测。任何一环失败进入统一回滚，而不能将 CreateProcess 成功当作 Applied。

### TUN-A06 / P1：生产生成上下文没有补入 IPv6 探测和内核进程保护

`config_codegen/input.rs:464-465` 定义 `protect_core_executables` 与 `has_global_ipv6_address`，但生产 `application/` 无任何赋值，`codegen.rs:42-58` 走 Default。全仓 `rg` 只在 generator tests/examples 找到 IPv6=true，进程保护仅有定义和消费者。

因此主应用生成 Xray TUN 时 `xray/inbound.rs:89-100` 恒选不含 `::/0` 的默认分支；`xray/routing.rs:166-174` 和 `singbox/routing.rs:93-101` 不生成内核进程保护规则。原版 Builder `:52-53` 读取真实全局 IPv6 状态并初始化保护核心集合，`:200-208` 合并实际主核；原版 RoutingService 解析实际核心路径。

这证明生产上下文漏接，不能据此声称本次已观察到 IPv6 泄漏或代理递归。实际网络后果应在隔离环境测量。修复应让只读平台探测/核心定位结果成为明确纯输入，覆盖有/无全局 IPv6、EnableIPv6Address 两态、主核+前置核路径；比较生成结果与冻结原版，随后验证真实流量路径。

## 条件缺陷与需真实验证项

### TUN-A07 / P1 条件：核心和 helper 重复创建同一适配器地址

当前 deferred 顺序为核心以配置中的 address/gateway 创建适配器，再发现索引，然后 helper `SetTunAdapterAddress`；`singbox/inbound.rs:47-62`、`xray/inbound.rs:76-88` 与 `tun_plan.rs:186-198` 提供同一地址。`WindowsBackend.set_tun_address` (`windows.rs:416-430`) 对每一地址调用 CreateUnicastIpAddressEntry，任何非零返回即失败，不读取已有状态，不区分谁拥有已有地址。

微软文档确认同接口同地址重复创建返回 ERROR_OBJECT_ALREADY_EXISTS：[CreateUnicastIpAddressEntry](https://learn.microsoft.com/en-us/windows/win32/api/netioapi/nf-netioapi-createunicastipaddressentry)。据此推断：当内核已经配置相同地址时，helper 将把正常已有地址当失败，引发 TUN 回滚。本次未调用真实 API，须登记为条件缺陷，不计真实场景 verified。

修复：明确内核/平台谁拥有地址与 MTU。核心创建并配置的地址应校验/采用事实；helper 仅配置它实际新增的资源，租约不能删除借用的已有地址。所有权明确后测试已有一致/不一致地址、IPv4成功IPv6失败的部分提交、反复开启关闭与回收失败。当前 backend 只在全部地址成功后登记 registry，半途中失败可能缺少已创建地址的回滚记录（`windows.rs:417-429`、`server.rs:236-239`）。

### TUN-A08 / P1 条件：重复 reload 使用停止前接口索引

`tun_plan.rs:44-65` 在生成计划时发现当前适配器索引；非零时 `engine.rs:3106-3109` 构造 resolved descriptor。`session.rs:1125-1127` 停止旧核心/旧 lease 后，`:1236-1308` 先对该旧索引申请 helper 地址，再启动新核心。

若核心停止时删除它创建的 TUN 适配器，该索引已经失效；新核心也可能创建另一个索引。这一失败条件符合 TUN 重开生命周期，但本次未执行 OS adapter 操作，不能标为实测。修复应将索引绑定具体 adapter generation/owner，并在本次核心创建后重新发现和核验；不要在切换前把当前索引当本次会话永久索引。

### TUN-A09 / P2：名称的 env override 只进入 helper 描述符

工作区 codegen 在 `codegen.rs:425` 固定 `DEFAULT_TUN_ADAPTER`，`tun_plan.rs:46-51` 仍允许 `V2RAYN_R_TUN_ADAPTER` 改 helper/discovery 名称。显式 override 会再次导致核心和发现名称不一致。普通无 env 场景不受这个分支影响。修复为冻结统一 adapter identity 并传给生成器与 helper；隔离 test override 必须覆盖完整计划，不能分别解析 env。

## TUN 设置字段是否有效

“映射到代码”只能算 implemented；上述生命周期尚有缺陷、真实效果未验证，所以当前不能将整个 TUN 设置组标 verified。

| Field ID | 字段 | 已找到消费者 | 当前结论 |
|---|---|---|---|
| FLD-CFG-095 | EnableTun | codegen + tun_plan + toggle | 有接线；TUN-A01/A02/A03/A04 阻断可用性 |
| FLD-CFG-096 | AutoRoute | sing-box tun inbound `auto_route` | 配置映射存在；生产路由效果未验证；原版 Xray 没有此同名开关语义 |
| FLD-CFG-097 | StrictRoute | sing-box tun inbound `strict_route` | 映射存在；原版 legacy+strict 清 BindInterface/SendThrough 的上下文调整未移植 |
| FLD-CFG-098 | Stack | sing-box tun inbound `stack` | 映射存在；不同 stack 真实 TUN 运行未验证，不应据 Xray config 宣称适用 |
| FLD-CFG-099 | Mtu | sing-box/Xray inbound；helper descriptor | 配置映射存在；helper WindowsBackend 未执行 MTU 设置，其描述符字段不能作为平台生效证据 |
| FLD-CFG-100 | EnableIPv6Address | 双核 inbound + helper descriptor | 映射存在；真实 IPv6 和全局 IPv6 探测上下文未验证/漏接 |
| FLD-CFG-101 | IcmpRouting | sing-box routing | 规则映射存在；真实 ICMP 路径未验证 |
| FLD-CFG-102 | EnableLegacyProtect | pre_socks_of/sidecar graph | 有消费者，但 TUN-A02 已复现拓扑错误 |
| FLD-CFG-103 | RouteExcludeAddress | 双核 inbound 与 TunSpec 校验 | 映射存在；真实地址排除/IPv6 排除未验证 |
| FLD-CFG-104 | IPv4Address | 双核 inbound 与 helper地址描述符 | 映射存在；TUN-A07 所有权/重复创建待解决 |
| FLD-CFG-105 | IPv6Address | 双核 inbound 与 helper地址描述符 | 映射存在；blank/default 两条路径应统一，真实 IPv6 未验证 |

helper WindowsBackend (`windows.rs:416-430`) 只执行地址操作并记录 config，没有读取 `config.mtu`；这不等于主内核 MTU 字段无效，但说明“helper MTU 平台已实现/验证”的表述不成立。

## 建议修复次序与验收门槛

1. A01 租约超时、A02 唯一 TUN provider、A04 覆盖实际 provider 提权，先恢复可运行合同。
2. A03 实际事实与两向操作结果贯穿 FRB；同时更正把缺陷当预期的 R4-25 测试/证据，保留历史而追加本轮更正。
3. A05 就绪/退出监测、A07 地址所有权与局部回滚、A08 adapter generation；任何清理失败保留可恢复记录，不凭“pipe 已掉”就删除证据。
4. A06/A09 生产上下文完整性；以冻结上游配置差分验收每个设置字段。
5. 在授权隔离环境从正式 UI 完整运行：普通用户开启、取消 UAC、重新开启、连续保持至少 60 秒、变更 TUN 参数后 reload、切换节点、关闭、核心崩溃、helper 崩溃、睡眠恢复；记录实际 adapter/进程/路由/DNS/IPv6/排除结果及清理。未完成不得提升 verified，不以 pure/mock 测试替代真实平台结果。

## 命令与边界

实际读取命令：Get-Content AGENTS.md/任务卡/台账/当前源/冻结上游；rg/rg --files；git diff --stat、git diff（仅相关源）；web 查询微软 CreateUnicastIpAddressEntry 原始文档。最初尝试 `docs/tasks/T14.md` 和 `compat/settings.yaml` 不存在，随后改读实际 R4-25 与 `compat/fields.settings.yaml`，没有据错误路径跳过审计。

实际执行复现命令（repository root）：

```powershell
& C:/Users/Colby/.cargo/bin/cargo.exe run --manifest-path docs/evidence/tun-settings-audit-2026-10-05/tun-contract-repro/Cargo.toml --offline --target-dir target/tun-audit-repro
& C:/Users/Colby/.cargo/bin/cargo.exe run --manifest-path docs/evidence/tun-settings-audit-2026-10-05/tun-contract-repro/Cargo.toml --offline --locked --target-dir target/tun-audit-repro
```

第二次包含 helper 和 legacy plan 两项复现，退出码 0。独立审计小项目使用自己的 Cargo.lock；没有修改根 workspace Cargo.lock，不能把这条命令称为根 workspace 发布门禁。

实际执行 Dart 命令（cwd=`apps/desktop`）：

```powershell
& C:/Users/Colby/toolchains/flutter/bin/flutter.bat test ../../docs/evidence/tun-settings-audit-2026-10-05/tun-ui-contract_test.dart
```

结果 1/1，退出码 0；证明缺陷可复现。未运行 cargo fmt/clippy/workspace 全测试、Flutter 全测试/发布构建，因为没有修改生产源；本次也没有重打包或改 dist。真实 Wintun、默认路由、真实代理流量/DNS、IPv6、真实 helper UAC、睡眠崩溃恢复均未运行/未验证。Oracle skill 未能发现，本次未运行 Oracle API 模式。
