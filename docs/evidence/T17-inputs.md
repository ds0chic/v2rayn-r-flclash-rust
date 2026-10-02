# T17 输入登记（联调前置 / 未决输入）

- 来源：`docs/evidence/audit/T09-T16.audit.muse.md`、`T09-T16.audit.gemini.md` 与各任务证据 §未决。
- 用途：T17–T20 联调前必须闭环的输入项；本文件只登记，不声称已验证。

## 1. 系统级真实写入首验（需隔离环境 + 用户明确授权）

以下路径在 T13/T14 中**仅编译存在、单测全走 Fake/Loopback**，未对宿主 OS 执行。首验必须在隔离环境
（专用测试机/快照/非生产用户会话）并在用户明确授权下进行，禁止在用户日常环境直接调用：

- **WinINET**：`WindowsSystemProxyBackend` 的 `InternetQueryOptionW`/`InternetSetOptionW`(per-conn 75)、
  `SETTINGS_CHANGED(39)`/`REFRESH(37)`；RAS/多连接（PPPoE）枚举（上游 `EnumerateRasEntries`）未复现。
- **HKCU 注册表**：`HKCU\...\Internet Settings` 的 `ProxyEnable/ProxyServer/ProxyOverride/AutoConfigURL/AutoDetect`；
  自启动 `HKCU\...\Run` 的 `RegSetValueExW`/`RegDeleteValueW`。当前自启仅 `FakeRegistry` 验证。
- **路由/TUN**：`CreateIpForwardEntry2`/`DeleteIpForwardEntry2`、`CreateUnicastIpAddressEntry`/
  `DeleteUnicastIpAddressEntry`；`reset_tun_address` 依赖本地 registry 记忆，跨进程重启后无法回滚未登记项。
- **提权管道**：假装 daemon `--serve` 生命周期、真实命名管道/DACL、SID 拒绝取值、Job Object
  （`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` 未设置、`AssignProcessToJobObject` 失败码被忽略）、
  `CreateProcessW` 提权启动内核、`TerminateProcess` 真实回收均未执行。
- **硬约束**：全程不得占用/修改 `127.0.0.1:10808`；不得改动用户真实系统代理与注册表；不得按名杀进程。

路径/句柄风险：**已整改**（见 docs/evidence/T09-T16-remediation.rust.md 与 T14-runtime.md）：
helper 路径经 `ipc_contract::validate_elevated_core_canonical` 规范化复验；Job 对象设 `KILL_ON_JOB_CLOSE` 且
Assign 失败即终止；租约清理失败外显并写审计；内核句柄绑定 PID+创建时间（`crates/runtime/src/identity.rs`）。
仍待补：完整 TOCTOU 竞态窗口收窄、租约恢复日志归属 net-host 的联调、真实提权上下文首验（隔离环境）。

## 2. 更新发布不可发布（未签名）

- `crates/updater` 默认 `UnsupportedSignatureVerifier`（`is_available()==false`），不验证任何签名。
- `.dgst` 资产解析未接入；`sha256` 仅取 GitHub `digest` 或 `tools/cores/cores.lock.json`（sing-box `sha256_verified:false`）。
- 外部 updater 自替换（对齐上游 `AmazTool`「等待退出后替换」）未实跑；跨卷 rename 未实现。
- 结论：**未选定签名方案 + 内置可信公钥 + 外部实跑通过前，禁止对外发布/自动更新。**

## 3. helper 与 net-host 租约联调待接线

- `services/net_host`/`bridge_api` 目前**没有** helper 客户端；会话令牌下发、`expected_sid` 与 token 握手、
  资源所有权交接、net-host 侧断连检测/恢复编排（§13 日志重读）均未接线。
- helper 租约尚未被任何上层驱动；恢复日志归属（net-host）未定。
- T17 需先定租约/恢复归属，再做端到端联调。

## 4. ACT/LAY 统计口径分裂

- 统计聚合**未按 `TagClass` 分桶**：`StatsAggregator` 按 tag 输出，proxy/direct 汇总需 UI 自行按
  `classify_tag` 聚合。
- 量纲不统一：Xray 显示单位按 **1024**（`LINK_BASE=1024`，复刻上游 `StatisticsXrayService.linkBase`），
  sing-box 上游按 **1000**（`core_adapters` 保留原始字节、由展示层换算），当前无统一换算口径。
- `generation` 语义不对称：仅计数回退/重置时递增，跨源（Xray 轮询 vs sing-box 增量累加）不对齐。
- ACT/LAY 计数口径（聚合分母、TagClass 分桶）未定；T17 状态栏/节点表若假设已验证会算错，需先定映射与除数。
