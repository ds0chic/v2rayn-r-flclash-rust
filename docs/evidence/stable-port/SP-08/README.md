# SP-08 证据（helper 侧范围）

状态：helper 侧 implemented；本卡整体仍 identified（host 侧由 A03 完成，见分工）。

范围声明：本目录只覆盖 SP-08 的 helper 侧——资源 journal、失败保留、
逐资源释放与幂等重试。host 侧（net_host 归属回传、跨层 UI 重试）由 A03
在 `services/net_host/**` 完成，不在本证据内。

基线：`92d46dd`（不 commit）。审计依据 CP-04；红合同复现
`docs/evidence/complete-port-audit-2026-10-06/runtime/cleanup-helper-contract.log`
（`owned_tun_count=0 closed=true` 丢归属并报成功）。

## 根因（helper 侧）

旧 `HelperServer::on_disconnect` 先 `closed=true` 再 `take()` 全部 owned
资源，失败后仍 `clear()` 丢弃且第二次调用返回空（假成功、不可重试）。

## 改动文件（仅写锁内）

- `services/privileged_helper/src/journal.rs`（新增）：`ResourceJournal`
  记录 owned 路由/地址/内核句柄，逐资源确认释放，失败保留
  `CleanupFailed` 并可重试；标签为有界标识，无路径/参数/凭据。
- `services/privileged_helper/src/server.rs`：`ConnectionLease` 接入
  journal；`dispatch` 成功路径登记 owned/确认释放；cleanup 改按
  cores→TUN→routes 依赖反向逐资源释放；失败项保留 owned+未 closed；
  零失败才 closed；新增 `retry_cleanup`、`pending_cleanup`、
  `pending_cleanup_count`、`journal_snapshot`、`owned_cores`、
  `cleanup_attempts` 供拥有平面（A03/host）查询与重试。
- `services/privileged_helper/src/backend.rs`：`FakeBackend` 增加
  `attempts` 计数（含失败注入）与 `clear_failure`，供重试断言；
  `FakeOp` 补 `Ord` 派生。仅测试钩子，无生产语义变化。
- `services/privileged_helper/src/windows.rs`：`reset_tun_address`
  失败时保留 tun 注册表记录（成功才删）；`shutdown` 逐句柄尝试并
  返回首错而非首错中断。编译未执行 OS 调用。
- `services/privileged_helper/tests/sp08_cleanup_journal.rs`（新增）：
  FakeBackend 故障注入回归；不动宿主真实 OS 资源。

## 先红后修

红：4/4 失败——失败清理后 `owned_*=0`、`is_closed()=true`，且
`server.rs` 旧单测把“失败后第二次返回空”当幂等（即 CP-04 假成功）。
修后：56 passed / 0 failed / 1 ignored（elevation 门控）。

## 定向检查（exit 均为 0）

- `cargo fmt -p privileged_helper -- --check` → 0
- `cargo clippy -p privileged_helper --all-targets --locked -- -D warnings` → 0
- `cargo test -p privileged_helper --locked` → 0（lib 16、dispatch 28、
  loopback 8、sp08 4；real_windows 1 ignored 需提权）

未运行：workspace 全量门禁（VALIDATION_POLICY：发布候选才跑）；
授权隔离机真实 TUN/路由收敛（无环境，未验证）。

## 接口需求/阻塞（登记，不私改共享 DTO）

1. helper 现有 `HelperError` 无 `AlreadyAbsent` 变体；清理中“已不存在”
   按保守策略视为失败保留（不假成功）。是否新增跨 helper→host 的
   已确认缺席语义，由 SP-00 整合者定，提供方 ipc_contract、调用方
   net_host/helper（host 侧 A03 跟进）。
2. host 侧待 A03：`pending_cleanup`/`journal_snapshot` 的 net_host
   消费、跨层错误传播与 UI 重试入口；`HelperOp` 新增清理查询如需
   新协议字段同样走 SP-00 整合。
3. 真实收敛（授权隔离机失败→重开→retry）未验证，保持 blocked。

## 对应 feature/field/action/layout ID

沿 `docs/repair/coverage.csv` owner_task=R4-05、R4-25 集合选取本次
唯一流程（停止 TUN 某资源清理失败→可见待清理→重试成功）；不将
owner 全集标 verified。本卡整体状态保持 identified。
