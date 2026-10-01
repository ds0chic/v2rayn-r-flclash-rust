# T14 运行时联调证据（net-host TUN 受控执行 + helper 租约 + dry-run）

- 任务：T14-runtime（privileged_helper 接入 net-host/运行时：TUN 模式受控执行与失败回滚）。
- 前置：`docs/evidence/T14-helper.md`（helper 协议/校验/租约，64 测试）。
- 范围：`services/net_host/**`（helper 客户端、TUN 租约编排、会话接线）、
  `crates/runtime`（`tun.rs` + `RuntimeTunDetail`）、
  `crates/application`（`tun_plan`、`TunStatus` 快照链）、
  `apps/desktop/lib/app/shell/status_bar_view.dart`（仅 TUN 实际状态指示）、
  本证据 + `docs/decisions/T14-runtime.md` + `compat/features.yaml`（F-TUN-001..005 状态）。
  未改 `bridge_api`、`config_codegen`、`platform`、`updater`、`subscriptions`、
  `persistence`、apps 其他文件；未执行 FRB 再生成；未 commit。
- 环境：Windows 11 x64，Rust 1.98.1 stable（MSVC），Flutter 3.47.5 / Dart 3.13.4。
- 硬约束遵守：全程未占用/修改 127.0.0.1:10808（未下发任何 RuntimePlan，未启动任何内核，
  未绑定任何 TCP 端口）；未改系统代理/注册表；未提权运行 helper（从未执行
  `privileged_helper --serve`）；只终止本轮自己启动的 net_host 冒烟进程（PID 见 §4）；
  无秘密写入日志/证据/夹具。
- 接手状态：本轮进场时仓库处于中断的半成品态——`services/net_host/src/session.rs` 已引用
  尚不存在的 `crate::helper_client`/`TunJournal`，`cargo check -p net_host` 编译失败。
  本轮补齐全部缺件并跑通门禁。

## 1) 架构与文件

执行顺序（`HostState::apply_plan`）：准备配置（校验/落盘/journal Prepared）→ TUN 描述符
校验（`tun_spec_from_plan`）→ 端口预检/内核定位 → **helper 添加路由/适配器**
（`tun_lease::apply_tun_lease`：validate → available → apply → 写 `tun_lease.json`）
→ 启动内核 → 就绪探测 → Applied。停止/失败时反向清理（先停内核树，再清 helper 租约，
再 Finalized）。

| 文件 | 职责 |
|---|---|
| `services/net_host/src/helper_client.rs` | helper 传输与租约：`HelperConfig::from_env`（`V2RAYN_R_HELPER_PIPE/TOKEN/BIN/RUN_ROOTS`、`V2RAYN_R_HELPER_NO_AUTOLAUNCH`、dry-run）、`TunLease`（路由摘要/适配器/dry-run 标记，`summary()` 脱敏）、`HelperLink`（`available/apply/cleanup/audit`）、`PipeHelperLink`（Windows 命名管道，一连接一会话，失败回滚本次已加路由）、`DryRunHelperLink`（只记录不触碰）、`UnavailableHelperLink`（非 Windows/测试，显式不可用）、`FakeHelperLink`（`#[cfg(test)]`，deny/timeout/disconnect/partial 注入）、`E_TUN_HELPER_UNAVAILABLE` 与 `map_helper_error`（拒绝/超时→该码，不静默降级） |
| `services/net_host/src/tun_lease.rs`（新） | TUN 恢复日志扩展：`tun_lease.json`（`TunJournalEntry{session_id, lease, updated_at}`，temp+rename 原子写）、`verify_lease`（会话/适配器/计数/摘要核对）、`verify_journal_integrity`（重算摘要防篡改）、`apply_tun_lease`（validate→available→apply→落盘；失败/落盘失败均清理并原样返回结构化错误）、`cleanup_tun_lease`（幂等：无日志即已清理；摘要失配仍清已记录资源；helper 清理失败仅日志）、`reconcile_stale_tun`（重启扫描残留日志逐个清理；helper 不可用则保留日志计 pending，绝不丢弃） |
| `services/net_host/src/session.rs` | 接线：`HostConfig.helper`、`Inner.tun_lease/tun_session_id/tun_link`（helper 连接保持到会话结束，早 drop 会触发 helper 端断连回收，故必须持有）、`HelperLinkFactory` 测试缝（`#[cfg(test)]`）、`release_tun_lease`（反向幂等释放，`spawn_blocking` 包裹阻塞管道 IO）、`apply_plan` TUN 阶段（helper 失败→Finalized+删 staged+结构化错误返回，内核不启动）、`abort_job_assign`/`rollback`/`stop_managed`（含 stop 早到窗口）全部释放、`HostState::new` 启动即对账、`detail.tun`（`RuntimeTunDetail`）随行、`tun_detail_from_lease`（脱敏） |
| `services/net_host/src/main.rs` | 注册 `helper_client`/`tun_lease` 模块 |
| `crates/runtime/src/tun.rs`（新） | 纯 TUN 描述符：`TunSpec` 校验（与 helper 合同同一套 validator）、`tun_spec_from_plan`（`tun_enabled` 与 `tun` 节点+`Tun` 特权一致性，错配即 `E_INVALID_PLAN`）、`route_digest`（顺序无关）、`route_summary`（仅计数）、`dry_run_from_env`（`--dry-run-tun`/`V2RAYN_R_DRY_RUN_TUN`） |
| `crates/runtime/src/wire.rs` | `RuntimeTunDetail`（适配器/接口/路由数/dry-run，无地址）随 `RuntimeDetail` 事件下发 |
| `crates/application/src/tun_plan.rs`（新） | `TunModeItem`→`TunSpec` 纯映射（`EnableTun=false`→`None`；IPv4 CIDR，缺省上游 `172.18.0.1/30`；裸 IP 拒绝；`Mtu<=0`→`1280`；IPv6 仅启用且有值时；`route_exclude` 透传校验；`interface_index==0` 拒绝）+ `attach_tun_to_plan`（置 `tun_enabled`/`Tun` 特权/`tun` 节点，幂等，可被 `tun_spec_from_plan` 回读） |
| `crates/application/src/runtime_client.rs` | `TunStatus` + `RuntimeSnapshot.tun`（`None`=未启用，禁止伪造） |
| `crates/application/src/net_host_client.rs` | `map_snapshot` 原样透传 `detail.tun`（`None` 保持 `None`） |
| `crates/application/src/snapshot.rs` | `Snapshot.tun`（§14 读模型，不经 bridge，不改 FRB） |
| `apps/desktop/lib/app/shell/status_bar_view.dart` | 仅 TUN 指示：新增 `tun-actual` 文本——开关关/`!isRunning`→`未启用`；`E_TUN_HELPER_UNAVAILABLE`→`失败已回滚`；运行中且开关开→`已请求(未验证)`。无后端事实绝不显示成功 |
| `compat/features.yaml` | F-TUN-001..005 → `implemented`（严禁 `verified`，见 §5） |

上游对照（只读）：`StatusBarViewModel.DoEnableTun`（开关→存 `TunModeItem.EnableTun`→Reload，
非管理员 Windows 回退 `RebootAsAdmin`；本轮只做指示，不做提权重启）、
`CoreManager`（启动前 `RemoveTunDevice`；本轮对应物为租约范围清理+helper 断连回收，
**未执行真实设备删除**）。

## 2) 门禁与测试（本机真实运行）

工作目录仓库根；`cargo` 用显式路径 `C:\Users\Colby\.cargo\bin\cargo.exe`。

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | 通过（exit 0） |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过（exit 0，0 warning） |
| `cargo test --workspace --locked` | 通过（**973 passed / 0 failed / 1 ignored**，83 个 target 行） |
| `dart format`（status_bar_view.dart） | 通过（已格式化，exit 0） |
| `dart analyze lib/app/shell/status_bar_view.dart` | No issues found |
| `flutter test test/t13_statusbar_test.dart` | All tests passed（2/2，既有，无破环；本轮未新增 flutter 测试） |

T14-runtime 范围用例（`#[test]` 计数，`--locked` 实测全过）：

| 位置 | 用例数 | 说明 |
|---|---|---|
| `crates/runtime/src/tun.rs` | 18 | plan 携带/缺失/特权/controlled-file/JSON/字节上限、kind/适配器/接口/地址/MTU/路由族/条数上限/exclude、helper 载荷转换、digest 稳定、summary 脱敏、dry-run 判定 |
| `services/net_host/src/helper_client.rs` | 14 | Fake 全流程：apply 记录路由+适配器、无路由跳过、deny→`E_TUN_HELPER_UNAVAILABLE`、timeout 可重试、disconnect、partial 回滚无残留、cleanup 幂等、断连清理幂等、dry-run 只记录、unavailable 结构化、summary 脱敏、lease JSON 回环、错误映射、地址族保持 |
| `services/net_host/src/tun_lease.rs` | 14（新） | apply 落盘+回读核对、deny/timeout/disconnect 无残留日志、非法描述符不触 helper、cleanup 清 helper 态+删日志、cleanup 幂等、无日志清理 Ok、失配仍清理、重启恢复（新 link 清理+二次空转）、不可用 pending 且保留日志、空目录零报告、dry-run 标记落盘、entry JSON 回环 |
| `services/net_host/src/session.rs` | 9（5 新） | 既有 reclaim/job_assign 4；新：detail 脱敏映射、dry-run 环境变量透传、`JournalEntry.identity()` 携带 PID+创建时间且 `matches_identity` 通过、伪造 identity 不匹配、factory 缝替换构造 |
| `crates/application/src/tun_plan.rs` | 11（新） | 关闭→None、显式 CIDR、缺省 `172.18.0.1/30`、裸 IP 带字段拒绝、零接口拒绝、MTU 回退 1280、非法 MTU 拒绝、IPv6 三态、坏 exclude 拒绝、attach 回读+幂等、非法 spec 不污染 plan |
| `crates/application/src/runtime_client.rs` | 3（2 新） | 默认无 TUN、TunStatus 回环 |
| `crates/application/src/net_host_client.rs` | 2（新） | tun 透传映射、无 tun 保持缺失（不断言伪造） |
| `crates/application/src/snapshot.rs` | 4（2 新） | tun 流入快照、无 tun 保持缺失 |
| **T14 合计** | **75（本轮新增 36）** | 均在 workspace 973 内，0 失败 |

## 3) dry-run 证据

- 单元层：`DryRunHelperLink` 全程只写内存 `ops`（`dry_run:add_routes`/`dry_run:set_tun_address`/
  `dry_run:remove_routes…reset_tun_address`），`tun_lease` 的 dry-run 用例断言 audit 前缀与
  `lease.dry_run=true` 落盘；`--dry-run-tun`/`V2RAYN_R_DRY_RUN_TUN` 判定 6 分支全覆盖。
- 冒烟层（真二进制，未下发 plan）：`target\debug\net_host.exe --dry-run-tun`（`V2RAYN_R_RUN_ROOT`
  指向临时目录、`V2RAYN_R_PIPE` 指向测试管道名），进程存活（PID 41896），stderr 仅启动行，
  会话目录零创建（除重定向日志外无文件），随后按 PID 终止（只杀自己）。全程无 helper 连接、
  无路由/适配器/DNS/代理动作、无 10808 绑定。

## 4) 明确声明：真实 TUN/提权未在本机执行

- **未提权**：从未以管理员身份运行任何进程；`privileged_helper --serve` 从未执行；
  `V2RAYN_R_HELPER_TOKEN` 未在真实特权会话下发。
- **未建真实管道**：全部传输测试走内存/dry-run；`PipeSecurity`/DACL、`open_pipe`、
  `launch_helper`、`CreateIpForwardEntry2`/`DeleteIpForwardEntry2`、
  `CreateUnicastIpAddressEntry`、`CreateProcessW`/Job 绑定均**编译但未执行**
  （`PipeHelperLink` 无任何单测/冒烟实例化）。
- **未改路由/适配器/DNS/系统代理/注册表**：`apply_plan` 的 TUN 分支只在 Fake/DryRun link
  下被单测调用；冒烟仅启动 idle net-host。
- **未启动内核/未绑定端口**：冒烟未下发 plan；`preflight_port` 未被触发；10808 未被探测或绑定。
- 结论：本轮为**编排/租约/恢复逻辑的离机验证**（implemented），平台真实行为未验证。

## 5) 未决项清单

1. **真实路由/适配器/DNS 未验**：`PipeHelperLink` 对真实 helper 的 AddRoutes/SetTunAdapterAddress
   往返、路由度量/优先级冲突、TUN 地址冲突、IPv6 前缀行为、失败回滚的真实码均未执行。
2. **提权首次验证未做**：helper 提权 daemon 的 `--serve` 生命周期、真实管道多实例/SID 拒绝、
   net-host 拉起 helper（`auto_launch`）的 10s 等待/杀进程语义未验证。
3. **真实内核联调未做**：TUN 计划 + 真实内核启动的端到端（准备→helper→内核→就绪→Applied）
   未执行；`RemoveTunDevice` 语义仅由租约清理近似。
4. **linux/macos 未验**：非 Windows 固定 `UnavailableHelperLink`（structured unavailable）；
   Unix socket 后端、提权模型缺失；`process_creation_time_ms` 非 Windows 返回 None。
5. **UI 事实最小化**：bridge 冻结导致快照 `tun` 字段到不了 Dart，状态栏只能显示四态文字
   （未启用/失败已回滚/已请求(未验证)）；per-lease 事实（适配器/路由数）待 bridge 解冻后透传。
6. **审计仅内存**：link `audit()` 只在失败日志打印一次，无落盘/轮转；helper 侧审计同 T14-helper。
7. **未 commit**：全部改动在工作区未提交状态。
