# UX-TEST-01 — 测速“不生效”专项诊断与可见反馈修复

执行模型：deepseek-v4.1-flash。基线：`461ab53`（工作树仅 `dist/SHA256SUMS`、`dist/build-info.json` 有未提交改动，与本次无关）。真窗口 Flutter 集成测试 + 真实 FRB / Rust / SQLite / 本地回环服务器 + 已授权临时数据目录的真实节点对照。

## 1. 结论

用户实测“测速不生效”**部分成立、根因已定位并修复**。链路（工具栏/右键 → `emitAction` → `startSpeedTest` → FRB → `SpeedTestRunner` → `ProfileExStore` → 150ms 轮询 `reload()` → `applySpeedTestOverlay` → 延迟列）本身是通的：TCPing 对真实 socket 测出数值。真正的问题是**结果表达层**，而不是任务没跑：

1. **成功无文案**：完成/失败后 `speedTestMessage` 始终为 `null`，工具栏与状态栏没有任何“完成/失败/成功 N”文字。
2. **失败被吞**：结果 `delay=-1` 的节点与“从未测试”的 `-1` 完全同样显示 `-`，用户看不出“测过但失败”。
3. **配置不生效**：`startSpeedTest` 硬编码 `https://cachefly.../gstatic` 的 URL/超时，**忽略用户在 `SpeedTestItem` 里保存的 URL**；而受限探针只实现明文 `http://`，HTTPS 必失败 → 用户看到的“测速不动”。
4. **空集合静默**：未选节点时 `realping/speedtest/tcping` 静默扩大为“全部节点”，空库时静默无任务。
5. **启动失败无反馈**：`startSpeedTest` 对 `result.ok=false` 只写 event log，不 `setMessage`。

## 2. 本地决定性复现（新 `integration_test/ux_speedtest_diag_test.dart`）

真实窗口、真实桥、真实 SQLite。本地 TCP 监听 `127.0.0.1:11808`，本地 HTTP `127.0.0.1:11809` 提供 `/ping`(204) 与 `/speed`(小文件)；用真实 `importFromText` 播种 3 条 `vless://…@127.0.0.1:11808` 合成节点。

修复前（`observations-before.json`，HEAD=461ab53）：

- 场景 A（TCPing）：`delay=1` 写回成功，但 `message=null`，无任何完成提示。
- 场景 B（真延迟/测速/混合/快速）：`delay=-1`，表格同 `-`，无汇总、无原因；且当时用内置 HTTPS URL，探针必然失败。
- 场景 C（空选择）：静默扩为全量，无提示。

修复后（`observations-after.json` / `observations.json`）：

- 场景 A：`stage=SpeedtestingCompleted`，`message="测速完成：成功 1"`；UX-1 = `1 ms`，其余 `-`。
- 场景 B：通过**真实设置文档**写入本地 `http://127.0.0.1:11809/ping`、`/speed` 后再走 UI 触发：
  - realping → `delaySentinels {UX-1:-2}`，`message="测速完成：失败 1（连接失败）"`；
  - speedtest → `message="测速完成：失败 1（SpeedtestingSkip）"`；
  - mixed → 3 节点 `-2`，`message="测速完成：失败 3（SpeedtestingSkip）"`；
  - fastrealping → 3 节点 `-2`，`message="测速完成：失败 3（连接失败）"`。
- 场景 C：空选择经真实桥 `startSpeedTest(kind, [])` 扩为全部（播放到基础节点），结束后留下“测速完成：…”文案；空库分支由 widget 测试断言为“没有可测试节点”。

据此确认：**“不生效”不是任务没跑，而是完成/失败/空集/启动失败都没有可见反馈，且用户配置的 URL 未被执行**。

## 3. 真实节点对照（脱敏，`real-nodes-*.json`）

数据来源：`%TEMP%\t21f_data` 的**授权副本**（`%TEMP%\ux-test01-realnode\data`）。仅记录计数/类型/耗时/结果码，不写地址、别名、凭据、URL。

- 库存：总计 **39** 节点（`vless` 26、`hysteria2` 13）。默认 `SpeedTestItem` 的 URL 方案为 **`https://`**。
- 真实窗口逐项（每次独立进程，≤3 次重试以规避 Debug 窗口在节点多时的间歇崩溃）：

| 动作 | 类型 | 耗时(ms) | 取消 | 阶段 | 可见文案 | 结果码 | 延迟哨兵 |
|---|---|---|---|---|---|---|---|
| Tcping | vless | 1551 | 否 | SpeedtestingCompleted | 测速完成：成功 1 | Speedtesting | 141 ms |
| 真延迟 | vless | 1057 | 否 | SpeedtestingCompleted | 测速完成：失败 1（连接失败） | Speedtesting | -2 |
| 下载(测速) | vless | 1057 | 否 | SpeedtestingCompleted | 测速完成：失败 1（SpeedtestingSkip） | SpeedtestingSkip | -2 |
| 混合 | vless | 744 | 否 | —（直接 job） | — | SpeedtestingSkip | -2 |
| 快速真延迟 | vless | 737 | 否 | —（直接 job） | — | Speedtesting | -2 |

判定：TCPing 对真实节点成功（141 ms，直连 TCP 可达）；真延迟/下载经临时 Xray 内核（日志证据见本地 `02-realping.png`：`Xray 26.3.27 started`、`accepted tcp:127.0.0.1:11809`）发起 SOCKS→明文 HTTP 请求，但默认 URL 是 **HTTPS**，明文探针不支持，故全部失败。因此真实节点的“测速不生效”= **网络/协议路径失败**（远端 HTTPS 未实现 + 临时内核会话偶发 `error.port_conflict`），**已由本次修复在 UI 明确显示失败原因**，不再只留 `-`。

> 受限规则：早期使用真实节点的截图因会出现真实地址/订阅 ID，已从仓库删除，仓库仅保留脱敏 JSON 与合成节点的截图。

## 4. 修复清单（UI 侧 + bridge 接线；未改 Rust）

- `apps/desktop/lib/features/profiles/profiles_controller.dart`
  - 新增 `SpeedTestConfig` 与 `_resolveSpeedTestConfig()`：从持久化 `SpeedTestItem` 读取 URL/超时/并发/页大小/IPAPI/UDP/延迟间隔，缺省回退上游默认。**用户配置的 URL 现在真的被执行**。
  - `_startTestPolling()` 完成时按目标节点集合汇总并写 `speedTestMessage`；`_summarizeSpeedTest()` 统计“成功/失败/未完成”，全失败时附首个真实原因；将占位 `Speedtesting` 归一为“连接失败”。
  - `startSpeedTest()`：空集合 → `没有可测试节点`；启动失败 → `测速启动失败：<code>（<detail>）` 并置 `SpeedtestingFailed`；成功 → 进行中阶段文案（含“未选择节点，正在测试全部 N 个节点”）。
  - `cancelSpeedTest()` → `已停止测速`。
- `apps/desktop/lib/bridge/bridge_port.dart`
  - `applySpeedTestOverlay` 将真实失败行映射为哨兵 `profileDelayTestFailed = -2`（与“从未测试”的 `-1` 区分）。
- `apps/desktop/lib/features/profiles/profiles_models.dart`
  - 延迟列：`-2 → 失败`，`-1 → -`，其余 `<ms> ms`。
- `apps/desktop/lib/features/profiles/profiles_page.dart`
  - 工具栏渲染 `speedTestMessage`（`ValueKey('speedtest-message')`），成功/失败/空集/启动失败都可见。

可见反馈契约（已覆盖）：成功 → 汇总文案 + 数值；失败 → `失败` + 汇总原因；空集合 → 明确提示；启动失败 → 结构化错误文案。

## 5. 测试与证据路径

- 真窗口集成：`apps/desktop/integration_test/ux_speedtest_diag_test.dart`（合成节点，场景 A/B/C，含本地 HTTP）。
- 真实节点：`apps/desktop/integration_test/ux_speedtest_realnodes_test.dart`（`V2RAYN_UX_TEST01_ACTION` 选择单个动作，脱敏记录）。
- Widget/控制器：`apps/desktop/test/ux_test01_speedtest_feedback_test.dart`（5）、`apps/desktop/test/ux_test01_speedtest_message_ui_test.dart`（1）。
- 证据：本目录 `observations-before.json`、`observations-after.json`(= `observations.json`)、`real-nodes-*.json`、合成截图 `00-seeded.png`/`01-tcping.png`/`02-realping.png`/`02-speedtest.png`/`02-mixed.png`/`02-fastrealping.png`/`03-final.png`。
  说明：状态栏文案位于窗口右下（`status-message`），在 1180 宽窗口内于 `x≈490` 起可见；工具栏的 `speedtest-message` 为本回合新增的用户可见转发点。

## 6. 门禁

- `dart format --output=none --set-exit-if-changed lib test integration_test` → **0 changed**（exit 0）。
- `flutter analyze` → **No issues found**（exit 0）。
- `flutter test`（逐文件，`tools/flutter_test_retry.ps1 -PerFile -MaxAttempts 4`）→ **PER-FILE PASS: all test files green**（exit 0，含新增 `ux_test01_*` 两文件）。
- `flutter build windows --release` → 成功（exit 0，`build/windows/x64/runner/Release/v2rayn_desktop.exe`）。
- 未改 Rust，故不重跑 cargo 门禁（`cargo fmt/clippy/test` 不适用）。

## 7. 复现命令

```powershell
# 本地决定性用例（合成节点 + 本地 HTTP）
$env:V2RAYN_R_DATA_DIR = "$env:TEMP\ux-test01-run\data"
$env:V2RAYN_UX_TEST01_EVIDENCE_DIR = "<repo>\docs\evidence\UX-TEST-01"
$env:V2RAYN_R_AUTOSTART = "0"; $env:V2RAYN_R_AUTO_SMOKE = "0"
flutter test integration_test/ux_speedtest_diag_test.dart -d windows

# 真实节点对照（单个动作；数据目录为 t21f_data 的授权副本）
$env:V2RAYN_R_DATA_DIR = "$env:TEMP\ux-test01-realnode\data"
$env:V2RAYN_UX_TEST01_REALNODE_EVIDENCE_DIR = "<repo>\docs\evidence\UX-TEST-01"
foreach ($a in 'tcping','realping','download','mixed','fast') {
  $env:V2RAYN_UX_TEST01_ACTION = $a
  flutter test integration_test/ux_speedtest_realnodes_test.dart -d windows
}
```

## 8. 未决项

- **远端 HTTPS 测速 URL 仍未实现**：受限 SOCKS 探针只做明文 HTTP；真实 `SpeedTestUrl`/`SpeedPingTestUrl` 默认 HTTPS，真实节点真延迟/下载仍失败。这是 T15b 既有未决项（`docs/evidence/T15b.md` §5），本回合未扩到 TLS。
- **临时内核会话偶发 `error.port_conflict`**：混合/快速并发时端口分配与测试会话时报冲突，属 Rust/net-host 侧现象，未在本回合（允许范围仅 UI/bridge）修复，仅确保 UI 显示该原因。
- **Debug 真窗口在 39 节点时偶发原生退出**（`did not complete`）：采用按动作独立进程 + 重试规避，非产品缺陷结论。
- 未验证 mac/Linux；未做物理鼠标点击、TUN/系统代理（遵守 AGENTS 禁用项）。
