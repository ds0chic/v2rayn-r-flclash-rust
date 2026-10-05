# R4-31 真实负载性能稳定 — 证据

- 任务卡：`docs/repair/tasks/R4-31.md`
- 仓库 HEAD：`8651e19`（工作树含其它任务代理未提交的并发改动；见下“工作树状态”）
- 本回合改动范围：Dart-only（`apps/desktop/lib/features/profiles`、`apps/desktop/test/**`）。
  Rust 未改动，故未执行 `cargo fmt/clippy/test`。
- 环境：Windows 11 x64；Flutter 3.47.5 (`C:\Users\Colby\toolchains\flutter`)。

## 1. 结论

- 发现并修复一处确定性的轮询空转/泄漏前置问题：测速/统计轮询路径
  `ProfilesController._refreshLive()` 在每次 150 ms tick 都无条件重建节点表读模型
  （全量 filter+sort、分配新列表），且缓存基为空时每 tick 触发一次全表结构读取
  `reload()`。已做最小修复：空基不再结构读取；overlay 无变化时跳过重建。
- 前置能力不回退：R4-09（游标分页）、R4-10（异步）、R4-22（测速 job/代际）、R4-23
  （monitor 会话代际/在途合并）相关测试全部通过。
- 未武装 release + 真实 SQLite 10k/100k、p95/p99 帧/反馈/搜索/首帧/IPC 队列、
  24h soak 与 500 次切换：**未验证**（见第 5 节）。

## 2. 改动文件

- `apps/desktop/lib/features/profiles/profiles_controller.dart`
  - `_refreshLive()`：`_baseSummaries` 为空直接返回（结构读取交给事件驱动的 `reload()`）；
    新增 `_overlayUnchanged()`，overlay 值未变时不执行 `_recompute`，保持同一读模型。
- `apps/desktop/test/r4_31_contract_test.dart`（新增，5 用例）
- `apps/desktop/test/repair/r4_31_repro_test.dart`（新增，2 用例，修复前失败）
- `apps/desktop/test/support/fake_monitor_bridge.dart`（新增订阅计数，泄漏守门用）
- `apps/desktop/test/support/r4_22_bridge.dart`（新增 `returnEmptySnapshots` 测试缝）

## 3. 复现证据（修复前 vs 修复后）

`test/repair/r4_31_repro_test.dart` 在**未修复**的 `_refreshLive` 下：

```
an idle poll tick must not rebuild the node table read model
  Expected: true
    Actual: <false>

the poll must not run a structural full read on every empty-base tick
  Expected: <2>
    Actual: <4>
```

即：空转 tick 重建读模型；空基下 ~350 ms 内多出 2 次全表结构读取（基线 2 → 4）。

修复后 `test/repair/r4_31_repro_test.dart` 与 `test/r4_31_contract_test.dart` 共 7 用例全绿。

## 4. 契约覆盖（合成，控制器级）

`test/r4_31_contract_test.dart`：

1. 空转 poll tick 保持同一读模型（无 churn）。
2. 真实结果变化仍重建并生效（Delay/Speed 覆盖）——不回退。
3. 测速完成后 poller 收敛、释放已跟踪 job（后续 stop 无可取消 job）。
4. 50 次反复切页/切会话下 `subscribeTraffic`/`subscribeLogs` 各仅 1 次（无订阅/句柄增长）。
5. 25×2000 行日志洪水后 Dart 读模型被 `maxDisplayedLogs=2000` 限界，溢出计数可见。

## 5. 未验证 / 阻塞

- **release 构建阻塞**：`flutter build windows --release` 失败于
  `bridge_api` 的 `error[E0425]: cannot find type ProfileDto in this scope`。
  该错误来自工作树中**其它任务代理的未提交 Rust 改动**（`crates/bridge_api/src/api/*`、
  `crates/application/src/engine.rs` 等，>500 行），与本卡 Dart-only 改动无关：
  本卡未改任何 Rust 文件。构建前已确认无本项目 `v2rayn_desktop.exe` 实例锁。
- 未武装 release + 真实合成 SQLite 10k/100k 未见；p95/p99 帧/反馈/搜索/首帧/IPC 队列
  未分别测量；24h 持续负载与 500 次切换未运行；真实慢盘/慢 host 下的 monitor 锁等待
  未定量。T18 既有基线（10k scroll build_p95≈21.8ms、50k≈36.9ms，短时内存）仅为历史
  合成参考，不构成本卡通过证据。
- 本卡未替换 Synthetic Bridge 掩盖慢路径：生产 `FrbBridgePort` 路径未改，
  修复位于生产控制器，测试仅用于控制流断言。

## 6. 工作树状态（只读观察，未触碰他人改动）

`git status --short` 显示本卡之外还有多代理并发改动（`compat/platform-matrix.md`、
`crates/application/src/*`、`crates/bridge_api/src/api/*`、`services/net_host/src/*`、
`docs/repair/tasks/R4-25|R4-33*`、`dist/*` 及若干 untracked 证据目录）。
本卡仅修改第 2 节列出的文件；未 `git add/commit`，未回退他人改动。
