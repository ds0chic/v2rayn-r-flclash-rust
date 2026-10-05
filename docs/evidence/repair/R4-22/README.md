# R4-22 测速范围取消和代际 — 证据

状态：implemented（本卡范围的行为测试与发布构建通过；真实内核/隔离平台效果未实测，见未完成）。

- 固定 commit：a95897f（执行时工作树含并行代理未提交改动，见未完成；本卡只改自有文件）。
- armed=false；未启动内核、未占用 10808、未改宿主代理/路由/TUN/自启；测试只用合成数据、无真实端口/网络。
- 夹具：`test/support/r4_22_bridge.dart`（合成 BridgePort + 可控 job/结果闭包）、`crates/application/src/speedtest.rs` 单元夹具。无用户凭据。
- 平台/DPI/内核：仅 `flutter build windows --release` 通过；真实 TCP/真实延迟/下载/UDP 效果未实测。

## 缺陷与修复

D08/D33（测速期间全库同步重读、停止缺乏运行代际合同；UF-PROF-12）
- Rust `SpeedTestJob` 增加单调 `generation`（`SpeedTestJobs::start` 用序号），`SpeedTestHub` 增加按节点的 `result_generation`；新 helper `begin_job_results`（起跑清空目标旧结果并按 generation 认领）与 `apply_job_batch`（仅接受 `generation >= 已接受代际` 的批次）。旧的/被取消的 run 迟到批次不再覆盖新 run 的结果，未被新 run 认领的节点仍可被旧 run 填充（R4-04“迟到响应淘汰”在测速域的落地）。
- Dart `ProfilesController` 增加 `_speedTestGeneration` 与 `_activeSpeedTestJobs`；`startSpeedTest` 先递增代际并把真实 `jobId` 入册，起跑后立即 `_refreshLive()` 反映“本轮尚无结果”；轮询器捕获 `(generation, targets)`，代际不再当前即自停，既不 settle 也不汇总别的 run。
- `cancelSpeedTest` 现在绑定并取消**全部**在途真实 job（对齐原版 `SpeedtestService.ExitLoop` 取消所有 `_runCtsList`），停止后保留一个有界 settle 轮询，直到 `speedTestActiveJobs()==0` 再读一次 overlay：取消只撤销未完成部分，已测得结果不丢，且本轮绝不汇总为“完成/成功”。

## 命令与结果

- `flutter analyze`（apps/desktop 全项目）：EXIT=1，仅 `test/r4_16_matrix_test.dart:65` unused_element 与 `test/r4_23_contract_test.dart:11` unused_import（两文件为并行代理本轮新增，非本卡所有权，未修改）。本卡 4 个改动文件单独 `dart format --set-exit-if-changed`：PASS；analyze 对 `lib/features/profiles/profiles_controller.dart` 无 issue。
- `flutter test test/r4_22_contract_test.dart`：8 passed / 0 failed。
- `flutter test test/repair/r4_22_repro_test.dart`：2 passed（修复后）；修复前同一文件 2 failed（证据见 commands.log 与 observations.json）。
- 回归：`flutter test test/r4_09_contract_test.dart test/t15b_speedtest_test.dart test/reprof04_sort_readback_test.dart test/fix10b_profile_order_test.dart test/re_prof_08_export_udp_test.dart test/ux_test01_speedtest_feedback_test.dart`：35 passed / 0 failed。
- `cargo fmt --all -- --check`：EXIT=0。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：EXIT=0（Finished，无 error/warning）。
- `cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings`：EXIT=0。
- `cargo test -p application --locked speedtest`：30 passed；`cargo test -p bridge_api --locked speedtest`：10 passed（含新增 `late_old_generation_batch_cannot_overwrite_a_newer_run`、`starting_a_run_clears_stale_delay_before_measuring`）。
- `cargo test --workspace --locked`：除 `services/net_host/src/session.rs` 的 `r305_sidecar_failure_after_helper_releases_tun_lease` 在并行负载下偶发失败外全部通过；该用例单跑 `cargo test -p net_host --locked r305_...`：1 passed（偶发/时序，非本卡改动，net_host 未改）。
- `flutter build windows --release`：EXIT=0，`Built build\windows\x64\runner\Release\v2rayn_desktop.exe`（构建前确认无本卡持有的 build 产物 exe；`dist` 下运行的 `net_host` 非本卡会话，未处置）。

## 未完成 / 接口缺口 / 阻塞

- 真实效果未实测：真实 TCP/真实延迟/下载速度/UDP 经节点 session（R3-PROF-05 路径未改，`NetHostTestSession.udp_ping` 仍走 `session.port`）需要真实内核/隔离环境，本轮只有 Rust 逻辑单测 + 合成控制器测试 + release 构建；平台效果标未验证。
- 未新增/修改任何 `#[frb]` 函数或 DTO 字段，未改 `frb_generated`，无需重生成。
- 有界并发：TCPing 并发沿用原版 `page_size` 上界，Realping/UDP/Mixed 沿用 `MixedConcurrencyCount`；`net_host_client.rs` 的 R4-04 命令门（含排队计时的总 deadline）已覆盖测速 job 的临时会话打开/关闭，无需额外改动。
- `flutter analyze` 全项目 EXIT=1 由并行代理的两个测试文件 warning 造成，不在本卡所有权范围。
- `cargo test --workspace` 的 net_host 偶发失败与 R4-22 无关，单跑通过。
