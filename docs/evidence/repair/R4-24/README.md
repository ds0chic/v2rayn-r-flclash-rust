# R4-24 代理PAC与applied端点 — 证据

- 固定 commit：`06f61f697415e3dda108b2c390c7b188cb71ba6f`（应用基线 77c74ed 之上的修复轮）
- armed=false；无内核、无网络、无宿主系统代理写入；合成数据夹具；端口 11808/11809/11810（≥11808，从未 10808）
- 平台：Windows 11 25H2 (build 26220) x64；Flutter 3.47.5 / Dart 3.13.4

## 结论

状态：**implemented**（真实隔离机 OS 效果 **blocked**）。

修复了方案 D19 / RUN-07 的核心断点：手动 apply / F5 / 切换节点成功链缺少上游
`MainWindowViewModel.LoadCore -> SysProxyHandler.UpdateSysProxy` 的对账。

改动（唯一生产文件）：`apps/desktop/lib/features/settings/platform_controller.dart`

1. `PlatformController.build()` 通过 `ref.listen(runtimeControllerProvider, ...)`
   监听运行态；每当出现**新的已应用会话**（`sessionId:port` 变化，且
   `hasAppliedEndpoint`）即按持久化的 `SysProxyType` 重新对账系统代理 / PAC。
   同一会话的重复快照不再重写宿主（幂等去重 `_lastAppliedSessionKey`）。
2. 新增 `syncAppliedMode()`：读取持久化模式并复用 `applyModeFromConfig`；
   `Unchanged` 保持 no-op（与上游一致）；无会话时不伪造成功。
3. `_appliedProxyInbound` 改为**始终使用实际已应用端口**（不再回退到配置端口），
   仅当持久化入站端口与已应用端口一致时才采信其协议，否则按 HTTP 处理，避免把
   SOCKS-only 监听误标为 HTTP / 反向误标。

未回退：R3-01 PAC 指令串（`PROXY/SOCKS5 host:port;DIRECT;`）、R3-PROXY-UI 协议区分、
FIX-15C 退出恢复的逐字段所有权与外部更改保留、`startPacFromConfig` 的自定义
PAC 路径/内容与缺失 seed 行为。

## 复现（先失败后通过）

`test/repair/r4_24_repro_test.dart`：在修复前（临时 `git stash` 掉生产文件后）
运行，断言 `platform.appliedModes` 应为 `[forcedChange]` 而实际为 `[]`（见
`prefix_repro_failure.log`）。恢复修复后同一测试转绿。

## 命令与结果

| 命令 | 结果 |
|---|---|
| `flutter analyze` | No issues found（`flutter_analyze.log`） |
| `flutter test test/r4_24_contract_test.dart test/repair/r4_24_repro_test.dart` | All tests passed（11/11，`flutter_test_r4_24.log`） |
| 复现测试修复前运行 | 失败：`Expected: contains forcedChange / Actual: []`（`prefix_repro_failure.log`） |
| 回归：r3_01_07_10 / fix15_tray_pac / fix15c_pac_resolve / recheck_rr02_03 / t13_platform_models | 全部通过 |
| `dart format --output=none --set-exit-if-changed`（本次三文件） | 通过 |

Rust 未改动，故未运行 cargo 门禁；原生构建未改动 Dart 之外的原生层，未运行
`flutter build windows`（本卡无原生改动）。

## blocked（未实测，登记）

- 真实 WinINET 系统代理 / PAC 写读、授权隔离机四模式、逐应用即时生效、所有权恢复：
  **未验证 / blocked**。当前环境无授权隔离 VM，且硬约束禁止写宿主系统代理。
  测试仅以 `FakePlatformBridge` + 合成 runtime 验证接线与事实判定；`enableRealBackend('windows')`
  的生产写入路径未在隔离机实测。
- `_appliedProxyInbound` 的协议事实目前只能来自 Dart 快照的端口 + 持久化入站；
  Rust 已解析真实入站协议/secret（`engine.rs:1742-1759`），但桥接快照未携带
  applied 协议字段。新增该 DTO 字段需要接口整合者改 FRB 生成（本卡禁改
  `frb_generated` / `lib/bridge/api/**`），登记为接口缺口。

## 接口缺口（提供方/调用方/输入输出/错误/生效点）

- 提供方：`crates/bridge_api/src/api/engine.rs` 的 `SnapshotDto`（或新增只读端点 DTO）。
- 调用方：`FrbRuntimeBridge._toView` → `RuntimeView`；`PlatformController._appliedProxyInbound`。
- 输入输出：追加只读字段 `applied_inbound_protocol`（HTTP/SOCKS/Mixed）与可选 `applied_inbound_port`。
- 错误：无（只读事实）；缺失时按现有 HTTP 默认。
- 生效点：每次成功 apply 的 snapshot；不得含认证/secret，敏感信息不入日志/证据。
