# T14 运行时联调决策记录（helper 接入、租约归属、失败语义与 dry-run）

- 关联方案：`outputs/V2RAYN_FLUTTER_RUST_PLAN.md` §5（net-host 唯一所有权、helper 有限操作枚举）、
  §13（启动/切换/退出状态机、恢复日志、PID+创建时间防复用）、§14（快照读模型）、
  §15（运行时编排）。
- 关联台账：`compat/features.yaml` F-TUN-001..005（本轮置 `implemented`，严禁 `verified`）。
- 前置决策：`docs/decisions/T14-helper.md`（白名单/DACL/租约三档策略/路径校验）。
- 相关代码：
  - `services/net_host/src/{helper_client,tun_lease,session,main}.rs`
  - `crates/runtime/src/{tun,wire}.rs`
  - `crates/application/src/{tun_plan,runtime_client,net_host_client,snapshot}.rs`
  - `apps/desktop/lib/app/shell/status_bar_view.dart`（仅 TUN 指示）

## 1) net-host 是唯一 helper 会话持有者

`PipeHelperLink` 一连接对应一 TUN 会话，连接存活期即 helper 租约存活期（helper 以
`LeasePolicy::CleanOwned` 在断连时回收）。因此 `Inner.tun_link` 必须与 `tun_lease`
同生共死、保持到会话结束；提前 drop 会触发 helper 端回收，导致“刚加好的路由被自己删掉”。
`stop_managed`/失败路径按**反向顺序**释放：停内核树 → 释放 helper 租约 → journal Finalized。
理由：内核先停可避免无路由窗口期的流量黑洞颠倒；journal 最后写保证崩溃重放总能看到未完成态。

## 2) 失败即结构化失败，绝不静默降级

helper 不可用（未运行/禁拉起/无 token/超时/断连）或拒绝（鉴权/白名单/越界），一律以
`E_TUN_HELPER_UNAVAILABLE`（拒绝类）或携带该码的可重试超时向上传播；`tun_spec` 非法以
`E_INVALID_PLAN`/`E_INVALID_ARGUMENT` 失败。`apply_plan` 在 helper 失败时**不启动内核**、
直接 Finalized+删 staged 产物。不提供“直连 TUN”“跳过 helper 继续跑”等回退路径。
理由：TUN 失败后继续跑内核等于把用户流量置于无隧道保护的默认路由下，比直接失败更危险；
UI 侧据此显示`失败已回滚`而非假成功。

## 3) 恢复日志只记“我们拥有的”，清理幂等

`tun_lease.json`（`TunJournalEntry`）与核心 journal 分离存放、同目录：记 helper 会话 ID、
适配器名、接口索引、路由计数与**顺序无关摘要**、dry-run 标记，外加完整 `TunSpec` 以便
重建清理动作。清理只删记录内的路由/适配器；无日志=已清理；摘要失配仍清已记录资源并换新日志
（防泄漏优先于严格一致）；重启对账时 helper 不可用则**保留日志计 pending**，留待下次启动。
理由：§13“资源已经恢复时重复恢复应安全”；pending 保留避免把未清理的特权资源静默遗忘。

## 4) 进程身份沿用 PID+创建时间，TUN 侧以摘要核对

内核进程继续用 `(pid, created_at_ms)` 认领（`JournalEntry.identity()` + `matches_identity`），
`terminate_identity` 拒绝复用 PID。本轮新增用例把该不变量钉在 net-host 侧。
TUN 资源无 PID，其身份即“helper 会话 ID + 路由摘要 + 适配器事实”：`verify_lease` 与
`verify_journal_integrity`（重算摘要防篡改）承担对等职责。
理由：两种资源用各自可靠的身份原语，不把 PID 语义硬套到 helper 句柄上
（helper 句柄 `u64` 不绑定 PID 的缺口仍在，见证据 §5）。

## 5) dry-run 是传输级开关，审计可看

`HelperConfig.dry_run`（`--dry-run-tun` 或 `V2RAYN_R_DRY_RUN_TUN` 非空/非 0/非 false）使
`build_helper_link` 直接返回 `DryRunHelperLink`：只追加内存审计（`dry_run:` 前缀），
不建管道、不起进程、不碰 OS。`TunLease.dry_run` 随租约与日志一路标记，`RuntimeTunDetail`
与 `TunStatus` 同样携带，UI/日志可据此区分“演习”与“实操”。
理由：提权/路由操作不可在开发机上试错；dry-run 让全链路（plan→net-host→审计）可跑通而不产生副作用。

## 6) UI 只陈述快照能证明的

bridge_api 冻结，本轮不新增 FRB 字段。状态栏 TUN 指示仅用既有快照字段推导四态：
开关关或运行时非 Running→`未启用`；快照错误码为 `E_TUN_HELPER_UNAVAILABLE`→`失败已回滚`；
运行中且开关开→`已请求(未验证)`。`None` 永远渲染为未启用。
理由：计划 §14“快照是 UI 同步的唯一读模型”；在 per-lease 事实能到达 Dart 之前，
任何“已生效”文案都是伪造。

## 7) 设置→描述符映射的保守选择

`tun_spec_from_settings` 对缺失做显式决策而非猜测：IPv4 缺省上游首选 `172.18.0.1/30`；
裸 IP（无前缀）拒绝；`Mtu<=0` 回退 `1280`（上游首选）；`interface_index==0` 拒绝；
栈/auto-route/strict-route/ICMP/legacy-protect 留在 core config，不进 helper 描述符
（helper 只做地址+显式路由）。
理由：与 `compat/codegen-map.{xray,singbox}.yaml` 已有映射保持一致，避免两处 TUN 语义分叉。
