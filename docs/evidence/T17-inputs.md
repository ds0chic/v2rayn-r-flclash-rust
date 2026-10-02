# T17 输入登记（联调前置 / 未决输入）

- 来源：`docs/evidence/audit/T09-T16.audit.muse.md`、`T09-T16.audit.gemini.md` 与各任务证据 §未决。
- 用途：T17–T20 联调前必须闭环的输入项；本文件只登记，不声称已验证。

## 1. 系统级真实写入首验（T21-D 已执行，范围见 T21-real-os.md）

**2026-10-02 更新：本机（Windows x64，当前用户，管理员会话）已执行真机首验**，证据 `docs/evidence/T21-real-os.md`：

- **系统代理**：注册表权威层写-读-复原 **verified**（`apply → enabled=true server=127.0.0.1:11808 → 复原`）；修复了 snapshot 与写入层不一致缺陷。WinINET per-connection 推送在本机失败（注册表持久化生效）；RAS/多连接枚举仍未复现；对运行中应用的即时生效未逐应用验证。
- **HKCU 自启动**：`HKCU\...\Run\v2rayNAutoRun_<md5>` 写→读→删 **verified**，无残留。
- **路由**：`AddRoutes/RemoveRoutes` 经 IP Helper API 真实增删 `198.51.100.0/24`（loopback）**verified**；默认路由未动。
- **TUN 适配器**：sing-box 1.14.2 `auto_route=false` 创建/销毁 `v2rayn-r-test-tun` **verified（安全范围）**；默认路由保持不变。
- **仍未执行**：生产 TUN 自动路由/全局接管（隔离环境首验）；提权 daemon `--serve` 全生命周期与真实命名管道/SID/Job 拒绝路径的真机执行；`CreateProcessW` 提权启动内核与 `TerminateProcess` 真机回收；`reset_tun_address` 跨进程回滚。
- **硬约束**：全程未占用/修改 `127.0.0.1:10808`；未按名杀进程。用户代理镜像已归一为与有效状态一致的直连（见 T21-real-os.md §1）。

路径/句柄风险：**已整改**（见 docs/evidence/T09-T16-remediation.rust.md 与 T14-runtime.md）：
helper 路径经 `ipc_contract::validate_elevated_core_canonical` 规范化复验；Job 对象设 `KILL_ON_JOB_CLOSE` 且
Assign 失败即终止；租约清理失败外显并写审计；内核句柄绑定 PID+创建时间（`crates/runtime/src/identity.rs`）。
仍待补：完整 TOCTOU 竞态窗口收窄、租约恢复日志归属 net-host 的联调、真实提权上下文首验（隔离环境）。

## 2. 更新发布（T21-B/C 已实做，仍缺代码签名）

- **已实现并真实验证**（`docs/evidence/T21-signature.md`）：内置 2dust 公钥（OpenPGP v5）验签真实通过（GnuPG CLI 后端，指纹 `76945E9F…3AE0`）；篡改拒绝；`.dgst` 正反例；`verify_app_release_asset` 接线点。
- **已实跑**（`docs/evidence/T21-install-update.md`）：Inno Setup 安装/静默卸载（无残留）；外部 `upgrade_runner` 自替换成功/失败回滚（loopback 合成 release）。
- **仍未完成**：安装器与主程序**无代码签名**；真实 GitHub 端点端到端自动更新未跑（仅 loopback）；跨卷替换；无 GnuPG 机器上 v5 验签 fail-closed（需自实现或声明依赖）。
- 结论：可作受控 RC 分发；**对外公开发布仍需代码签名 + 真实端点实跑**。

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
