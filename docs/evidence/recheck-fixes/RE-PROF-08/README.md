# RE-PROF-08 证据 — 完整客户端配置两导出与 UDP 测速

状态：`implemented`。日期 2026-10-04；冻结上游 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。开始 HEAD `842520b`。

本回合只运行合成/回环测试与定向单 crate 命令；未启动真实内核、未监听用户端口、未改系统代理/注册表/路由/TUN、未读取或写入用户凭据。UDP 夹具绑定 `127.0.0.1` 上 ≥11808 的空闲端口；`127.0.0.1:10808` 全程未使用。

## 改动概览

- 上游对照：`ProfilesViewModel.cs:743-797`（`Export2ClientConfigAsync`/`Export2ClientConfigResult`，文件对话框与剪贴板两条完整客户端配置导出）与 `SpeedtestHandler` UDP 动作（`ACT-PROF-018`）。
- Rust：`crates/application/src/speedtest.rs` 新增 `udp_target_endpoint`/`udp_ping`；`crates/bridge_api/src/api/speedtest.rs` 接入 `NetHostTestSession::udp_ping`、`speedtest_supported().udp=true`、新增 `export_client_config`。
- Dart：两导出入口从 `shell.notImplemented` 改为真实动作（可注入保存选择器、剪贴板、取消、写失败分支）；UDP 条目按 `SpeedTestSupport.udp` 动态启用，`startSpeedTest` 增加支持门。

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `cargo fmt -p application -p bridge_api` | 仅格式化两 crate |
| `cargo test -p application --lib --locked re_prof_08` | 4 passed / 0 failed |
| `cargo test -p application --lib --locked client_config_export_text` | 1 passed / 0 failed |
| `cargo test -p bridge_api --lib --locked speedtest` | 6 passed / 0 failed |
| `cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings` | passed |
| `flutter analyze` | No issues found |
| `flutter test test/re_prof_08_export_udp_test.dart` | 5 passed / 0 failed |

未运行（明确）：全量 workspace 测试、`flutter build windows`、真实内核/真实节点路由 UDP、真实文件保存对话框、FIX-10/10B 的测速集合/排序回归复跑。

## 接口缺口（FRB 重生成）

`export_client_config` 已实现但未写入 `frb_generated.*`；`FrbBridgePort.exportClientConfigText` 在重生成前返回 `E_FRB_REGENERATION_REQUIRED`，不引用未生成符号。根代理重生成后按任务卡“接口缺口”一段接线。

## 结构化事实

见 [observations.json](observations.json)。
