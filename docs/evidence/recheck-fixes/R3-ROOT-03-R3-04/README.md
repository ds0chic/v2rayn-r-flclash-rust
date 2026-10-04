# R3-ROOT-03 / R3-04 证据（2026-10-04）

基线：冻结 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `cd141f5`。
执行环境：Windows 11 x64，PowerShell 7；Rust `C:\Users\Colby\.cargo\bin\cargo.exe`；Flutter `C:\Users\Colby\toolchains\flutter\bin\flutter.bat`。
边界：未占用/修改 `127.0.0.1:10808`；未改宿主系统代理/注册表/路由/TUN；未做真实提权重启/UWP 回环系统写入/浏览器打开；未读用户凭据。真实设备创建保持 blocked。

## R3-04（TUN 首次创建代码路径）

改动：
- `crates/application/src/tun_plan.rs`：新增 `TUN_DEFERRED_PROCESS_ID`（`tun-deferred`）、`build_tun_spec_fields`、`tun_deferred_spec_from_settings`、`attach_deferred_tun_to_plan`；`tun_spec_from_settings` 保持“索引未知即拒绝”的严格语义，新增 3 个单测。
- `crates/application/src/engine.rs`：`build_runtime_plan_with_hints` 在 `enable_tun && interface_index==0` 时挂 deferred 描述符（不再拒绝），已知索引时走原 resolved 路径；新增 `first_tun_without_interface_builds_a_deferred_plan` 单测。
- `services/net_host/src/session.rs`：`PreparedPlan` 增 `deferred_tun`；新增 `deferred_tun_from_plan`、`tun_discovery_timeout/interval`（`V2RAYN_R_TUN_DISCOVERY_TIMEOUT_MS`/`_INTERVAL_MS`）、`parse_tun_interface_index`、`discover_interface_index_os`（netsh）、`HostState::set_tun_discovery` seam、`resolve_deferred_tun`、`apply_tun_spec`；`precheck_plan` 检测 deferred 后不再走严格解析；`prepare_sidecars` 跳过 `tun-deferred`；核心 spawn + job 绑定后执行“轮询发现→helper→ready”，失败走既有 `rollback`（R3-05 清理 scope）。新增 4 个测试。
- `crates/application/tests/fix13_tun_presocks_plan.rs`：原“无接口=结构化错误”用例按新语义改为“构建 deferred 计划”。

命令与结果：
- `cargo fmt -p application -p net_host -- --check`：exit 0。
- `cargo clippy -p application -p net_host --all-targets --locked -- -D warnings`：exit 0，0 警告。
- `cargo test -p application --lib --locked`：232 passed；0 failed。
- `cargo test -p application --test fix13_tun_presocks_plan --locked`：6 passed。
- `cargo test -p net_host --locked`：62 passed；含 `r304_defers_helper_until_the_core_created_interface_is_discovered`、`r304_discovery_timeout_rolls_back_without_a_lease`、`r304_helper_denial_after_discovery_cleans_up`、`r304_parses_netsh_adapter_index_like_the_application_side`。
- TUN 相关 `--lib tun_plan`：18 passed。

上游对照：
- 冻结 `CoreManager.cs:88-96`：先清理旧设备再启动核心，设备由核心创建。当前实现改为：计划阶段不拒绝，核心先起，net-host 轮询发现接口，再 helper 设置地址/路由 → ready。语义对齐“设备由核心创建”。
- 已知接口路径保持原顺序（helper 先于核心），未回归。

blocked（需授权隔离实测）：
- 真实 TUN 设备由核心创建、UAC 授权/取消、地址/路由/DNS 生效、退出回收、重开；`privileged_helper` 真实 `netsh`/地址写入未执行。
- `netsh` 生产发现路径仅在无测试 seam 时启用，本轮测试全部注入 stub，未触碰宿主网络栈。

## R3-ROOT-03（菜单入口）

改动：
- `apps/desktop/lib/app/menu/main_menu.dart`：`管理员重启(ACT-MAIN-029)`、`UWP 回环(ACT-WIN-004)`、`区域预置设置+默认/俄罗斯/伊朗(ACT-MAIN-032/033/034)`、`核心网站(ACT-WIN-008)` 去掉 `preservedOnly`；结构与顺序不变。
- `apps/desktop/lib/features/settings/settings_actions.dart`：新增管理员重启（`rebootas` + PowerShell `Start-Process -Verb RunAs`）、UWP `CheckNetIsolation` 命令构造、核心网站 URL/`cmd start`、区域预置消息与 `applyRegionPreset`；均带可注入 launcher seam。
- `apps/desktop/test/r3_root_03_actions_test.dart`（新）：7 项纯构造/stub 断言。
- `apps/desktop/test/t17_menu_structure_test.dart`、`t17_disabled_entry_test.dart`：改为断言新入口已启用、仅 `推广(ACT-WIN-003)` 仍 preserved_only。
- `compat/actions.yaml`：六个 action 的 `implementation_location`/`test_ids`/`status` 更新并追加 notes（未删行、未降分母）。

命令与结果：
- `dart format`（改动文件）：exit 0。
- `flutter analyze`：No issues found（exit 0）。
- `flutter test test/r3_root_03_actions_test.dart -r expanded`：7/7 passed。
- `flutter test test/t17_menu_structure_test.dart -r expanded`：6/6 passed。
- `flutter test test/t17_disabled_entry_test.dart -r expanded`：2/2 passed。

上游对照：
- `ProcUtils.cs:49-68` + `Global.cs:88`：`rebootas` + `runas`。Dart 无 verb，改 PowerShell `Start-Process -Verb RunAs`（ShellExecuteW runas 等价），当前 exe 路径经 `Platform.resolvedExecutable` 解析。
- `MainWindow.xaml.cs:251-253`：启动 `bin/EnableLoopback.exe`；本实现同时解析该工具并按 `CheckNetIsolation LoopbackExempt -a/-d -n=<PackageFamilyName>` 构造可逆命令。
- `MainWindowViewModel.cs:238-250` + `ConfigHandler.cs:2894-2957`：区域预置写 Geo/SRS/路由源并配置 DNS。Rust `engine.apply_regional_preset` 离线写源+内置 DNS，已有 `regional_preset_writes_upstream_sources_into_settings` 断言内容与写入。
- `MainWindow.xaml.cs:431-453` + `Global.cs:649-665`：核心网站=release URL 去 `/releases`，`ProcessStart` 打开。本实现 `cmd /c start` 打开默认 Xray 主页。

未完成 / 接口缺口：
- 上游按核心动态展开多个「<核心> 网站」条目；当前为单一「核心网站」入口。
- bridge `apply_regional_preset`（`crates/bridge_api/src/api/dns.rs`，不可改）的 Russia/Iran 分支尝试下载远程模板；完全离线菜单路径需把应用层离线预设暴露到 FRB（改 `frb_generated` 越界）。
- `main_shell.dart` 接线未落地（禁止修改），补丁见 `docs/tasks/R3-ROOT-03.md`；未落地前点击走 `default` 的 `notImplemented` 提示（不伪装成功）。

## 未运行（边界）
全 workspace 门禁、`flutter build windows`、真实提权/UWP 写入/浏览器打开、真实 TUN 设备与 helper 系统写入、原版双窗口对照、10808 与用户凭据操作均未执行。
