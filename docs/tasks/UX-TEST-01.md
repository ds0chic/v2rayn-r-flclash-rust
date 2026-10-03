# UX-TEST-01 — 测速“不生效”专项诊断与可见反馈修复

- 状态：implemented（Rust 侧 TLS/端口冲突仍未 verified）
- 基线：`461ab53`（工作树仅 `dist/*` 未提交改动）
- 执行模型：deepseek-v4.1-flash
- 证据：`docs/evidence/UX-TEST-01/`

## 目标

用**真实窗口**复现“测速不生效”，定位根因并修复；保证任何成功/失败/空集合/启动失败都有可见反馈，禁止静默无效果。

## 已定位根因

1. 完成/失败后 `speedTestMessage=null`，无完成/失败文案。
2. 结果 `delay=-1`（测试失败）与“从未测试”同显示 `-`，无法区分。
3. `startSpeedTest` 硬编码 HTTPS URL，忽略 `SpeedTestItem` 的用户配置；受限探针只支持明文 `http://`。
4. 空选择静默扩为全量；空库静默无任务。
5. `result.ok=false` 只写 event log，无 `setMessage`。

## 改动

- `apps/desktop/lib/features/profiles/profiles_controller.dart`：`SpeedTestConfig` + `_resolveSpeedTestConfig`（执行用户配置）、完成汇总 `_summarizeSpeedTest`、空集合/启动失败/进行中/取消文案。
- `apps/desktop/lib/bridge/bridge_port.dart`：失败哨兵 `profileDelayTestFailed=-2` 叠加。
- `apps/desktop/lib/features/profiles/profiles_models.dart`：延迟列 `-2→失败`。
- `apps/desktop/lib/features/profiles/profiles_page.dart`：工具栏可见 `speedtest-message`。
- 新增测试：`test/ux_test01_speedtest_feedback_test.dart`、`test/ux_test01_speedtest_message_ui_test.dart`、`integration_test/ux_speedtest_diag_test.dart`、`integration_test/ux_speedtest_realnodes_test.dart`。
- 未改菜单结构；未改 Rust。

## 验收

- 真窗口集成（合成节点）：场景 A/B/C 通过；完成/失败/空集均有文案；失败列显示“失败”。
- 真实节点对照（脱敏）：39 节点（vless 26 / hysteria2 13）。TCPing 成功（141 ms）；真延迟/下载/混合/快速因默认 HTTPS + 明文探针限制失败，均显示失败原因。
- 门禁：`dart format`(0 changed)、`flutter analyze`(No issues)、`flutter test -PerFile`(all green)、`flutter build windows --release`(exit 0)；未改 Rust。

## 未决

- 远端 HTTPS 测速未实现（沿用 T15b §5）。
- 临时内核会话偶发 `error.port_conflict`（Rust/net-host 侧，超出本回合范围）。

## UX-TEST-02 收口（2026-10-03，追加）

两个遗留缺口已在本回合修复（详见 `docs/evidence/UX-TEST-02/README.md`）：

- **远端 HTTPS 测速**：探针经临时会话 SOCKS 叠加 rustls（`ring`），产线走 OS
  根校验、测试注入合成 CA，无 insecure 模式；结构化失败分类。真实节点默认
  `https://cachefly.cachefly.net/50mb.test` 下载测得 **29.6 MB/s**、HTTPS 真延迟
  129 ms；本地合成 TLS 目标集成用例通过。
- **`error.port_conflict`**：定位为“探测不占用 + 释放后 TIME_WAIT”竞态；改为
  进程内预留块 + 轮转游标 + net-host 释放确认。回归：40 次 start/stop 不立即
  复用、两 job 并发端口互斥，确定性用例 `ux_test02_https_probe`（15）全绿。
- 门禁（Rust + Flutter）全 exit 0，见 UX-TEST-02 证据 §6。

状态：Rust 侧 TLS/端口缺口由 `implemented` → 已实测（真实节点 + 确定性用例）。
