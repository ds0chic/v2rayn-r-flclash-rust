# RR-05 / RR-06 修复证据（2026-10-04）

状态：`implemented`（真实 TUN/提权/多核整链保持 `blocked`）。

开始 HEAD `cbf4fa7`（工作树干净）。冻结上游 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

本回合只改允许清单内文件，未触碰 `main_shell.dart`/`app.dart`/两处 `frb_generated`/`lib/bridge/api/**`/`apps/desktop/lib/features/**`/只读服务与 updater/subscriptions；未跑全仓 fmt、未跑 `flutter build windows`、未 git add/commit；测试端口均 ≥ 11808，未占用/修改 10808，未写宿主系统代理/注册表/路由/TUN，未触发真实 UAC，未杀非本项目进程。

## 改动文件

| 文件 | 变更 |
|---|---|
| `crates/runtime/src/adapter.rs` | 12 个新 adapter（V2fly/V2flyV5/Mihomo/Hysteria/NaiveProxy/Tuic/Juicity/Hysteria2/Brook/OverTls/ShadowQuic/Mieru）；`CoreAdapter::exe_names()` 候选；`CoreLocator` 候选回退 `find_candidate_exe`；`PROXY_CORES` 全 `Some`，仅 `App` `None` |
| `crates/application/src/tun_plan.rs` | `discover_interface_index`/`parse_interface_index`/`resolve_interface_index`；`tun_hints_from_env` 由发现驱动，显式覆盖优先 |
| `crates/application/src/codegen.rs` | `generate_pre_socks_config(core, listen, port)` 用冻结生成器产出真 SOCKS 配置 |
| `crates/application/src/engine.rs` | sidecar 体改真配置；`custom_default_core` 解析 `CoreTypeItem` 的 `Custom` 绑定（缺省 Xray）；更新 FIX-13B 陈注释；新增 3 测试 |
| `services/net_host/src/helper_client.rs` | `effective_token`/`generated_helper_token`/`default_helper_bin`；`launch_helper` 走 `shell_execute_runas`（UAC，取消=1223→denied）；`elevation` 直连 shell32 FFI；新增 3 测试 |
| `services/net_host/src/session.rs` | `PreparedSidecar`；`prepare_sidecars`；`start_prepared` stage/spawn sidecar + `wait_socks_port` + 逆序 `stop_sidecars`；`Session.sidecars`；`SidecarSession`；新增 5 测试；`rr10_precheck_rejects_unsupported_core` 改用 `App` |
| `services/privileged_helper/src/main.rs` | `--token`/`--run-roots` 命令行参数（env 兼容） |

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `cargo fmt -p runtime -p application -p net_host -p privileged_helper -- --check` | clean（0 diff） |
| `cargo clippy -p runtime -p application -p net_host -p privileged_helper --all-targets --locked -- -D warnings` | clean（clippy.log） |
| `cargo test -p runtime --lib --locked` | 52 passed（runtime-tests.log） |
| `cargo test -p application --lib --locked tun_plan` | 15 passed（application-tunplan-tests.log） |
| `cargo test -p application --lib --locked pre_socks` | 4 passed（application-presocks-tests.log；含 custom::rejects_bad_pre_socks_port） |
| `cargo test -p net_host --locked` | 57 passed（net-host-tests.log） |
| `cargo build -p net_host -p privileged_helper -p application --locked` | exit 0（FFI/链接通过） |

未运行：真实 TUN 设备创建、真实 UAC、宿主路由写入、真实多核整链、全 workspace 门禁、Flutter 构建。

## 上游对照结论

- **RR-06 多进程顺序**：`CoreManager.cs:94-96` 主核→`WaitForProxyPort(preContext)`→`CoreStartPreService`；本实现 `prepare_sidecars`(停旧前) → 逆序停侧车 → 主核，`wait_socks_port` 实现 `CoreManager.cs:230-286` 的 `05 01 00`→`05` greeting。一致。
- **RR-06 预 SOCKS 真配置**：`CoreStartPreService` 调 `CoreConfigHandler.GenerateClientConfig(preContext)` 生成前置配置；本实现 `generate_pre_socks_config` 走冻结生成器，替换描述 JSON。一致（配置体层面）。
- **RR-06 Custom 默认核**：`ConfigHandler.cs:1555-1576` 用 `GetCoreType(null, EConfigType.Custom)`；本实现 `custom_default_core` 读 `core_type_item` 的 `Custom` 绑定，缺省 Xray。一致。
- **RR-06 adapter 候选**：`CoreInfoManager.cs:120-283` 每核 `CoreExes`/`Arguments`；本实现候选名对齐（mihomo `GetMihomoCoreExes` 全表；naive/hysteria2/brook 平台后缀；sing-box client 别名）。主要 `Arguments` 对齐（mihomo `-f`+`-d`、overtls `-r client -c`、juicity `run -c`、shadowquic `-c`、v2fly_v5 `run -c -format jsonv5`）。**未逐核核对全部 env/参数**（见 blocked）。
- **RR-05 接口发现**：上游 `WindowsUtils.RemoveTunDevice`/核心创建设备；本实现用受控 `netsh interface ipv4 show interfaces` 发现 index，替代人工 env。与上游“设备由核心创建”一致，helper 只写已有 interface 地址（`windows.rs:408-420`，未改）。
- **RR-05 授权**：上游 Windows 无 runas 前置服务（Linux/macOS 有 `CoreAdminManager`）；本实现按卡要求补 `runas` UAC + 取消分支，属本项目新增的普通用户授权闭环。
- **RR-05 token**：本实现从配置生成并随命令行传递，不再要求用户设 env。

## blocked 清单

1. 真实 TUN 设备创建/路由下发：无授权隔离主机，未实测“核心创建→发现 index→helper 写地址/路由”。最小路径见 `docs/tasks/RR-05.md`。
2. 真实 UAC 提权：本机未触发真实 UAC（只测 1223 映射）。最小路径：隔离 VM 手动提权 helper + Ping 握手。
3. 逐核完整 `Arguments`/`Environment`：`mieru` env 传配置、`hysteria2`/`brook` 平台参数、`test_args` 对无 `-test` 核回退 `version_args`（非绑定）。最小路径：`tools/cores` 真实二进制逐核跑 version/test。
4. 真实 xray+sing-box 预 SOCKS 双进程整链：本机用 cmd stub 覆盖顺序/等待/清理；未用真实核心命中共享 SOCKS 端口。
5. Flutter 侧 TUN 错误文案：`status_bar_view.dart:288-293` 只看 `value=true` 与 runtime.error，属 `apps/desktop/lib/features/**` 禁改区，未改；Rust 侧 `reconcile_applied_session` 已保证 Stopped/Degraded 撤下 applied 端点（事实正确）。

## 未完成 / 接口缺口

- 预 SOCKS 侧车端口在 `RuntimePlan.ports` 非 exclusive（有意保留，避免与主核假冲突）；`prepare_sidecars` 通过 `owner==node.id` 反查端口，若未来多侧车复用同端口需结构化端口 owner 契约。
- 侧车 executable 定位不绑定 target 的 pinned version（侧车可能是与 target 不同的核心，如 Xray 节点下的 sing-box 侧车），按已安装核心 `resolve(core, None)` 解析。
- helper token/pipe/bin 仍经 net-host 自身 env（`net_host_client.rs` 只转发 cores root，且为禁改文件）；本卡用“同目录发现 + 生成 token + 命令行传递”补齐普通路径，未改 IPC 合同。
- `wait_socks_port` 使用主核 `readiness_timeout`（默认 20s）；上游用 `Global.LocalFetch`。若要精确对齐需独立超时配置。
