# 完整审计报告（2026-10-09）

范围：Flutter 界面（`apps/desktop/lib`）+ Rust 后端（`crates/*`、`services/*`），对照冻结上游 v2rayN 7.25.4（`7d6a967c`）。
本报告只记录发现，**未修改任何代码**（前两轮已修复项见 `code-quality-2026-10-09.md`）。

## 0. 方法与可信度

| 来源 | 说明 |
|---|---|
| 主会话直接核实 | 在代码/上游源码中逐行确认，标 **[已核实]** |
| 子代理报告（8 个域） | 只读审阅；其中 6 个由 Haiku 执行，未逐条复核的标 **[代理]** |
| 工具扫描 | clippy（Windows 目标）、cargo-audit、`flutter pub outdated`、死代码扫描、重复依赖树 |

**[代理]** 项为静态阅读结论，置信度见各条；修复前建议先写失败测试复现。
**未运行**：Windows 构建、任何运行时验证、`net_host` 测试、`scan_image_qr`/`scan_screen_qr` 测试（会挂起）。
**未验证**：Windows 专有行为（文件锁、注册表、命名管道、TUN）、Linux/macOS 的 TUN 与更新流程均只做了静态阅读。

## 1. 工具门禁结果

| 检查 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace`（排除 net_host，Linux） | 通过 |
| Windows 目标 clippy（`net_host`、`privileged_helper`、`runtime`，`x86_64-pc-windows-msvc`，`-D warnings`） | 通过（exit 0） |
| `cargo test --workspace`（排除 net_host） | 1706 通过 / 17 失败 / 1 忽略（失败集与基线一致，含 1 个已知临时目录竞态 `sp14_preview_custom_leaves_no_files`） |
| `dart format` / `flutter analyze` | 通过 / 无告警 |
| `flutter test`（排除 scan 测试） | 1219 通过 / 10 跳过 / 14 失败（与基线同一集合：陈旧测试 r4_11/r4_23/r4_31/t18b/t11、Linux 缺 FRB 库、Windows 路径分隔符） |
| `cargo audit`（RustSec 1295 条，Cargo.lock 399 crate） | 5 个漏洞 + 1 个 unmaintained，见 §3 |
| `flutter pub outdated` | 见 §3 |

## 2. 高危发现（HIGH）

### 2.1 生成的核心配置错误

| # | 问题 | 位置 | 上游 | 状态 |
|---|---|---|---|---|
| H1 | 完整模板 `AddProxyOnly=false` 时 `ProxyDetour` 被加到 freedom/blackhole/dns 出站，直连与 DNS 被绕进代理 | `config_codegen/src/xray/config.rs:593` | `V2rayConfigTemplateService.cs:184-191`（if/else-if 结构，这三类协议永不走 detour） | 已核实 |
| H2 | 自定义出站节点 + EnableFinalFragment：替换命中的是 tag=`proxy` 的 freedom 克隆，真正占位符（改名为 `fragment-proxy`）被留下 | `xray/config.rs:185-231`、`389-463` | `V2rayOutboundService.cs:58`（按对象引用） | 已核实 |
| H3 | Xray TUN 入站 sniffing 缺 `fakedns`（FakeIP 开启时） | `xray/inbound.rs:112-123` | `V2rayInboundService.cs:88-90,164-172`（共享 socks 的 sniffing 对象） | 已核实 |
| H4 | `has_global_ipv6_address` 和 `protect_core_executables` 生产路径从不赋值（只有 examples/测试）→ TUN 不路由 `::/0`，IPv6 绕过隧道；核心进程直连/DNS 保护规则（xray 与 sing-box 均）从不生成 | `config_codegen/src/input.rs:464-465`；`application/src/codegen.rs` `settings_from_app` 未设置 | `CoreConfigContextBuilder.cs:52-53`、`V2rayRoutingService.cs:16-30,239-278` | 已核实 |
| H5 | 测速用「单节点 + 默认设置」生成配置：策略组/链式节点必报 `dangling_reference`；自定义出站节点报 `custom_outbound_missing`；用户的 Fragment/mux/UA/sendThrough 等全部被忽略；且忽略「按配置类型绑定内核」 | `bridge_api/src/api/speedtest.rs:223-243`，核心类型 `:112-114` | `CoreConfigV2rayService.cs:87-200`；`SpeedtestService.cs:155-165` | 已核实 |
| H6 | sing-box DNS：直连「预期 IP」规则只保留 `respond`，丢弃 `evaluate`，`match_response` 无对应 evaluate，预期 IP 分支永不命中；`speculative` 从未设置 | `singbox/dns.rs:404-418` | `SingboxDnsService.cs:385-407`（`AddRange` 全部 + speculative） | 已核实 |

### 2.2 数据完整性 / 状态一致性

| # | 问题 | 位置 | 状态 |
|---|---|---|---|
| H7 | 导入提交：事务已提交且 revision 已 bump，之后 `persist_config` 失败 → `Err` 分支删除已暂存的自定义配置文件，数据库行仍指向它们 | `application/src/engine.rs:4188-4192`、`4339-4341` | 已核实 |
| H8 | 取消测速：进行中节点结果写成 `failed(-1)` 并被接受；Dart「移除无效节点」按 `delay == -1` 删除 → 误删未真正测出失败的节点 | `application/src/speedtest.rs:976-983,1217-1218`；`bridge_api/src/api/speedtest.rs:321-345`；`profiles_controller.dart:2056` | 已核实 |
| H9 | 备份恢复后测速内存覆盖层（`loaded` 只载一次）未重置，下一次 flush 用恢复前的延迟/排序覆盖刚恢复的 `ProfileExItem` | `bridge_api/src/api/speedtest.rs:294-302,347-363`；`t16.rs:156-170` | [代理] |
| H10 | 保存/删除节点：先写 store 再写 guiNConfig，后者失败时无回滚（行已变、revision 已 bump、界面报错） | `engine.rs:946-978`；`store_repo.rs:442-451` | [代理] |
| H11 | 备份/WebDAV 打包整个 `data_dir`（含 `cores/*` 二进制、`.previous`、`.staging`、日志）；整包在内存中构建，WebDAV 30 s 总超时；恢复只合并不清理，旧包会把 `install-manifest.json` 退回旧版本 | `application/src/backup_service.rs:729-759,839-885`；`t16.rs:342-344`；上游仅复制 guiConfigs 顶层文件 | 已核实（范围）；超时 [代理] |

### 2.3 更新与内核运行

| # | 问题 | 位置 | 状态 |
|---|---|---|---|
| H12 | 更新运行中内核：不停止、不重载；先删 `<dir>.previous` 再 `apply_atomic`（Windows 上重命名运行中目录大概率失败，回滚副本已丢）；Linux 上旧二进制继续运行 | `application/src/update_service.rs:813-817`；`updater/src/install.rs:264`；`t16.rs:712-837`；上游 `CheckUpdateViewModel.cs:293-327` | 已核实（代码）；Windows 具体失败模式未验证 |
| H13 | Xray 从不设置 `XRAY_LOCATION_ASSET`/`_CERT`，更新的 Geo 文件对 Xray 无效 | `runtime/src/adapter.rs:347`（默认 `env_vars`）；上游 `CoreInfoManager.cs:166-169` | 已核实 |
| H14 | 非 Windows 解压后核心文件无可执行权限（全仓库非测试代码无 `set_permissions`） | `updater/src/unpack.rs`；上游 `CheckUpdateViewModel.cs:401-408` | 已核实 |
| H15 | mihomo 在 Linux/macOS 上选 `.gz` 资源，但安装只接受 `.zip/.tar.gz/.tgz` → 恒报 `UnsupportedArchive` | `updater/src/metadata.rs:147-155` vs `update_service.rs:~1050` | 已核实 |

### 2.4 Windows 平台与特权（均未在 Windows 运行）

| # | 问题 | 位置 | 状态 |
|---|---|---|---|
| H16 | 特权 helper 的 `session_token` 从不比对（`HelperServerConfig.session_token` 只存不读；契约只校验非空）；同用户任意进程可发 `RunElevatedCore`/`SetTunAdapterAddress`/`RemoveRoutes`/`StopElevatedCore`；token 还在 argv 中 | `privileged_helper/src/server.rs:44,278`；`ipc_contract/src/helper.rs:393-407`；`net_host/src/helper_client.rs:1226` | 已核实（未比对）；利用面取决于威胁模型 |
| H17 | 核心意外退出不释放 TUN 租约：reconcile 无 TUN 处理，看门狗要求有 session，idle 退出被 `tun_lease` 阻塞 → 路由/地址留在宿主，直到 GUI 发 Stop | `net_host/src/session.rs:657-681,744-953` | [代理] |
| H18 | 系统代理部分写入失败：循环内 `?` 直接返回，已写字段不进账本，`restore` 无法撤销，重试也不会记录所有权 | `platform/src/sysproxy/mod.rs:277-279`；`application/src/platform_service.rs:311` | 已核实（代码） |
| H19 | 系统代理所有权账本只在内存；崩溃/被杀后 HKCU `ProxyEnable=1` 指向死端口，启动时无对账（方案 §117/§318 要求持久恢复日志） | `platform_service.rs:154,178` | [代理]，启动路径缺失置信度中 |

### 2.5 前端流程

| # | 问题 | 位置 | 状态 |
|---|---|---|---|
| H20 | 退出清理吞掉失败：`stop()` 返回 `Future<void>` 丢弃 bool；`restoreOnExit`/`stopSubScheduler` 结果被忽略；随后立即 `destroy()`，「退出清理未完成」消息看不到 | `desktop_integration.dart:505-538`；`runtime_controller.dart:609-620` | 已核实 |
| H21 | 非 Windows 点关闭按钮直接 `destroy()`，不走 `runShutdown`（不停核心、不恢复系统代理/PAC） | `desktop_integration.dart:469-483` | 已核实 |
| H22 | 保存完整配置模板后只 `reload()` 列表，不重载运行中的核心（上游 `MainWindowViewModel.cs:623-631` 会 `Reload()`） | `profile_actions.dart:758-760`；`bridge_api/src/api/groups.rs:103-116` | 已核实；运行时影响为推断 |

## 3. 依赖审计

### Rust（cargo-audit，DB HEAD `550efd3d`）

| crate | 公告 | 可达性 | 建议 |
|---|---|---|---|
| `h2 0.4.13` | RUSTSEC-2026-0258（空 DATA 帧无限排队，低） | 已编译（reqwest http2） | `cargo update -p h2`（≥0.4.16） |
| `rustls 0.23.38` | RUSTSEC-2026-0285（握手消息层级校验，5.3，转录仍被认证） | 已编译 | `cargo update -p rustls`（≥0.23.45） |
| `rustls-webpki 0.103.12` | RUSTSEC-2026-0104（CRL 解析 panic） | 代码中无 CRL 使用 → 不可达 | 随 rustls 一并升级 |
| `quinn-proto 0.11.14` | RUSTSEC-2026-0185（7.5） | 仅在 Cargo.lock（reqwest 可选 http3，未启用），不编译 | 随更新清理 |
| `rsa 0.9.10` | RUSTSEC-2023-0071（Marvin，无修复版本） | `pgp` 仅用于验签（`updater/src/signature.rs:17`；私钥仅在测试中） → 不适用 | 记录，跟踪上游 |
| `rustls-pemfile 2.2.0` | unmaintained | `application/src/speedtest.rs:518` | 改用 `rustls-pki-types` 的 PEM 解析 |

重复版本：`thiserror 1`（core_adapters/subscriptions/updater）与 `2`；`rand 0.8`（pgp/rsa）与 `0.9`；另有 getrandom、hashbrown、miniz_oxide、serdect、syn 双版本。均无功能风险，仅增加体积与编译时间。

### Dart
- `cupertino_icons`：`lib/` 中无引用（模板遗留）→ 可移除。
- `system_tray ^0.1.1`（最新 2.0.3，仅 `desktop_integration.dart` 使用）：落后大版本，仅提示。
- `meta`/`vector_math` 可在现有约束内升级。

## 4. 中危发现（MED）

**Xray / 通用 codegen**
- 日志文件名：`log_date` 固定 `"2026-01-01"`，从不更新（`application/src/codegen.rs:43`；影响 `xray/log.rs:16,23` 与 `singbox/log.rs:28`）。[已核实]
- 规则指向 ProxyChain/策略组时误生成 observatory+balancer（`xray/routing.rs:288-299`，上游 `V2rayRoutingService.cs:205` 按 tag 前缀计数）。[代理]
- Hysteria2 realm finalmask 未规范化、缺 `stunServers`、解析失败仍输出（`xray/outbound.rs:933-934`；上游 `HyRealm.cs`）。[代理]
- Xray TUN 接口名固定 `v2rayn-tun`，上游 `xray_tun`/`utunN`（`codegen.rs:438`、`tun_plan.rs:29`；`tun_plan` 依赖该名解析索引，属有意，建议记入台账）。[已核实]
- ProtectDomainList 收集全部节点，且缺 ECH SNI 与 xhttp 下载地址（`engine.rs:4586-4595`；上游 `CoreConfigContextBuilder.cs:377-413`）。[代理]
- 链克隆路径丢 `custom_tags`（`xray/outbound.rs:233-270`）。[代理]

**sing-box / mihomo**
- 非并行 DNS 回退：先输出全部 evaluate 再输出 respond，上游为逐服务器交错（`singbox/dns.rs:573`）。[代理]
- 假 IP 过滤表为裁剪版固定列表，上游完整表（31 域名/43 后缀等）（`util.rs:246-266`）。[代理]
- sing-box utls 指纹对所有 TLS 节点回退到 `DefFingerprint`，上游仅在节点自带指纹时输出（`singbox/outbound.rs:879-888`；台账 `codegen-map.singbox.yaml:267,297` 引用了上游死分支）。[代理]
- `realm+http://` 被输出为 https（`singbox/outbound.rs:723-769`）。[代理]
- 自定义 DNS 中 protect 规则位于 Global/Direct clash 规则之后（`singbox/dns.rs:742-752`）。[代理]
- mihomo：Mixin.yaml 缺失时不应用默认 mixin（`engine.rs:4700-4710`；上游 `CoreConfigClashService.cs:171-176`）。[代理]
- 嵌套逻辑规则内的 `geoip` 未转换（与上游同缺陷，`singbox/routing.rs:382-407`）。[代理]

**订阅 / 分享链接**
- 订阅刷新始终去重（`subscriptions/src/merge.rs:206`、`application/src/subs.rs:490-497`）；上游只有手动「移除重复」才去重；`keep_older=false` 时输出被反转后未还原，IndexId 顺序倒置。[已核实]
- Hysteria2 realm 分享链接导出丢失会合端口（`subscriptions/src/fmt/hysteria2.rs:~224`；上游 `Hysteria2Fmt.cs:160`），重导入变 443。[已核实]
- 订阅请求默认不发 User-Agent，上游发 `v2rayN/<version>`（`download.rs:116-121`、`platform/src/http.rs:40-43`）。[代理]
- 订阅更新后活动节点可能悬空：`find_matched_profile` 缺上游「仅地址匹配」末步（`subs.rs:722-766`；上游 `ConfigHandler.cs:1361-1369`）。[代理]
- 全配置/Clash 订阅节点忽略 `SubItem.PreSocksPort` 与订阅名（`subs.rs:612-628`）。[代理]
- WireGuard `.conf` 键区分大小写（`fmt/wireguard.rs:90-135`；上游 OrdinalIgnoreCase）。[代理]
- 订阅编辑窗口对非法数字静默改值（`sub_edit_window.dart:107,112,114`）。[代理]

**核心引擎 / 持久化**
- 删除活动节点后 `active`/`index_id` 悬空，直到重开才修复；Dart `prepareStartTarget` 取到已删 id（`engine.rs:956-978,3218`；`profiles_controller.dart:716-735,1050-1095`）。[代理]
- 切换节点时上一个 apply 任务不关闭（`engine.rs:3350-3372`）。[代理]
- ProfileEx flush 无事务、逐行自动提交、持锁、不删孤儿行（`engine.rs:1016-1045`）。[代理]
- `ResourceScheduler` 的 stop 不取消进行中的资源下载（`engine.rs:326-348`）。[代理]
- `guiNConfig.json` 写入无 fsync（含 WebDAV 凭据）（`engine.rs:5609-5616`）。[代理]
- 节点 IndexId 格式 `p-<nanos>-<seq>`，上游为十进制 Int64 字符串（`repository.rs:19-26`；需 ADR 说明）。[代理]
- Realping 无失败重测、不按页大小/延迟间隔分批（`speedtest.rs:992-1015`；上游 `SpeedtestService.cs:240-275`）。[代理]
- tcping 每页为每个节点起一个 OS 线程（页大小 1000）（`speedtest.rs:975-978`）。[代理]
- `seed_synthetic_profiles` 作为生产 FFI 暴露，会把 SQLite 存储换成内存存储（`bridge_api/src/api/engine.rs:1162`）。[代理]
- 多个 `#[frb(sync)]` 函数在 Dart 线程执行 SQLite/大量工作（`speedtest_remove_invalid`、`export_client_config`、`stats_flush` 等）。[代理，取决于 FRB sync 语义]

**更新 / 备份**
- 架构校验常选中 `LICENSE`，解析失败即放行，等于未校验（`update_service.rs:1069-1082,1192-1210`）。[代理]
- GeoFiles 与周期资源任务只处理 geoip/geosite 两个 dat，缺 SRS 规则集与 OtherGeoUrls（`updater/src/geo.rs`、`engine.rs:279-298`；上游 `UpdateService.cs:139-456`）。[代理]
- 更新窗口成功后仍保留旧检查结果，「应用」按钮仍可点（`update_controller.dart:370,390-397,443,463-471`）。[代理]
- `SelectedCoreTypes` 以 `xray` 小写持久化，上游为 `Xray`（`update_controller.dart:136-221`）。[代理]
- 单个内核检查失败中断整批检查/应用（`t16.rs:689-701,777-829`）。[代理]
- 下载 180 s 总超时，慢网下大文件永远失败（`update_service.rs:35-37`、`updater/src/download.rs:44,116`）。[代理]
- WebDAV 列表把集合自身计入（`webdav.rs:233-304`）。[代理]
- 备份窗口状态条在滚动区底部而非固定（`backup_and_restore_view.dart:69-76,271-275`）。[代理，布局未渲染验证]
- 暂存目录 `cores/.staging` 从不清理（`update_service.rs:771-779`）。[代理]

**Windows 平台**
- helper 部分失败不回滚路由/地址；补偿 `RemoveRoutes` 结果被丢弃（`privileged_helper/src/windows.rs:399-453`；`helper_client.rs`）。[代理]
- helper 管道客户端无读超时，reconcile 持 `inner` 锁做阻塞 I/O（`helper_client.rs:1043-1066`；`session.rs:762-779`）。[代理]
- 客户端 apply/stop 预算 60 s < 最坏路径 75 s（`ipc_contract/src/lib.rs:37`、`net_host_client.rs:489-512`）。[代理]
- 失败路径 `let _ = release_tun_lease()` 吞错并置 Stopped（`session.rs:1321,1888,1922,2435`）。[代理]
- `StopElevatedCore` 对已退出进程返回 `UnknownHandle`（`privileged_helper/src/windows.rs:573-589`）。[代理]
- helper 操作未按调用方租约限定范围（`server.rs:344-347,370-372,414-416`）。[代理]
- net-host 连接任何占用 helper 管道名的进程并先发 token（`helper_client.rs:999-1012`）。[代理]
- 暂存配置 ACL 失败仍写入含凭据文件（`session.rs:1727-1742`）。[代理]
- 提权核心退出观测与续租只在读路径触发，看门狗不调 reconcile（`session.rs:684,700,495`）。[代理]

**Flutter 控制器 / 视图**
- 流量/速度在核心停止后冻结（`monitor_controller.dart:344`；Rust 在未 applied 时不发批）。[已核实]
- 会话变化后 Clash 代理/连接面板留空直到手动刷新（`monitor_controller.dart:319-350`，自动刷新默认关）。[代理]
- 监控错误无任何读者，面板空白无原因（`monitor_controller.dart:498,532,560,577,731`）。[代理]
- 「清除统计」不清节点表（`monitor_controller.dart:699-713`；`main_shell.dart:440-446`）。[代理]
- 路由「设为默认」对当前默认方案显示原始错误键（`routing_windows.dart:287-289`；`engine.rs:1610-1614`）。[代理]
- `setDefaultAndReload` 在已有重载进行时读到旧状态（`routing_controller.dart:340-356`）。[代理]
- DNS 简单设置读取失败把 revision 覆盖为 0，下次保存必冲突（`dns_controller.dart:86`；`dns.rs:225`）。[代理]
- 路由草稿「替换导入」后选中集不清理（`routing_windows.dart:735-742,3008-3015`）。[代理]
- Clash 延迟探测 URL 从不下发（`monitor_controller.dart:585` 无调用方；`SpeedPingTestUrl` 对测速本身有效）。[代理]
- 150 ms 轮询在测速期间每次全量读结果+统计并重建覆盖层（10k 节点量级未测）（`profiles_controller.dart:559-575`；`bridge_port.dart:506-514`）。[代理]
- 设置读取失败仍以默认文档打开设置窗口（`settings_actions.dart:20-26`）。[代理]
- 导入提交失败回执被缓存并按 mutation id 重放（`import_persistence.dart:181-201`）。[代理]
- 「移除无效」批量删除失败静默（`profiles_controller.dart:2056-2098`）。[代理]
- 托盘左键对已最小化窗口执行 hide（`desktop_integration.dart:225-276`；`isVisible` 对最小化窗口为 true）。[代理]
- 分组编辑器下拉持有已不存在的订阅 id（调试断言/发布空白并保存悬空 id）（`group_editor_dialog.dart:240,399-409,660-668`）。[代理]
- 运行时修改 Hide2TrayWhenClose 在非 Windows 不生效（`desktop_integration.dart:151-158`）。[代理]
- AutoHideStartup 不检查托盘是否初始化成功，且在所有平台生效（上游仅 Windows）（`desktop_integration.dart:177-212`）。[代理]
- 主菜单「切换双击激活」只改内存，不持久化，且上游无此菜单项（`main_menu.dart:211-216`；`profiles_controller.dart:1772-1778`）。[代理]
- 用户可见处直接显示 i18n 键/错误码（`profile_editor_dialog.dart:184,611`、`template_window.dart`、`dns_controller.dart`、`routing_controller.dart`、`sub_setting_window.dart`、`subs_actions.dart`）；`ErrorLocalizer` 仅状态栏使用。[代理]
- 编辑器切换协议保留旧协议字段且不应用新协议默认值（`profile_editor_dialog.dart:216-231`；`profile_draft.dart:208-210`）。[代理]
- 状态栏/日志视图整状态 watch + `visibleLogs` 每次构建全量过滤并重编译正则（`monitor_controller.dart:105-116,224-227,431-436`）。[代理]

## 5. 低危发现（LOW，摘要）

条目较多，仅列类别；位置见各代理原始输出（可由重跑审计复现）：
- **Xray**：Pre-socks+StrictRoute 未清 BindInterface/SendThrough；DNS `regionName` 取首个匹配（上游取末个）；`use_direct_dns` 条件与上游不同（sing-box 同）；`merge_selector` 缺 subjectSelector；ProxyDetour 私网判断额外读 `settings.address`；freedom `domainStrategy` 未限 tag=direct；TUN 排除地址不接受无前缀 IP；`serveStale:false` 恒输出；socks `routeOnly` 未在 TUN 下强制 true。
- **sing-box**：WebSocket early-data 仅匹配 `?ed=`；SS 方法表含 sing-box 不支持的 `plain`；预期 IP 区域前缀大小写敏感；WireGuard endpoint 链克隆不完整；悬空策略组子节点使整次生成失败（上游跳过）；macOS TUN 名固定 `utun0`；Global/Direct 在配置期截断用户规则；mixin `removed-` 空项语义。
- **订阅**：CRLF 下解析偏移漂移；TUIC 无 `:` 用户信息当作密码；分组子节点过滤非法正则时匹配为空（上游全匹配）；导入预览摘要不含密码/传输字段；三份 `CompareProfileItem` 已漂移；VMess 导出 `aid` 为字符串。
- **更新/备份**：校验和下载失败时放行；`verify_payload` 在真实 app 升级路径被跳过（release 下 `app_repo` 为 None，不可达）；upgrade_runner 失败后不重启应用、macOS `/proc` 轮询、FlatOverlay `validate` 比较绝对路径与裸名、GnuPG VALIDSIG 首字段对比子密钥、`V2RAYN_R_GPG` 环境变量在 release 生效；RootCertProvider 未应用到更新/Geo 下载；WebDAV 连接检查不验证写权限且不保存字段；PROPFIND 解析仅识别 `d:` 前缀且不解码 href；WebDAV 下载与上游 zip 导入无大小上限；发布标签直接用于路径；缺 riscv64/loong64 资源并含虚构 X86 名；tar 长名/保留名边缘情况；更新窗口无取消、不显示已装版本。
- **核心引擎**：取消的未测节点持久化为延迟 0 + 「Speedtesting」；文本过滤在 SQLite 与内存后端对 `%`/`_` 语义不同；`speedtest_start` 静默截断 100000；暂存自定义配置权限默认；导出文件扩展名未净化。
- **Windows**：`terminate_identity` 先查后杀存在 PID 复用竞态；`CreateJobObjectW` 失败后挂起子进程泄漏；net-host 非挂起启动再入 Job；创建时间查询失败回落 0；`set_tun_address` 覆盖逐接口记录；命令行引号转义不完整；WinINET 查询回退读法可疑；`write_registry` 泄漏句柄且先写 Enable；不更新 RAS 拨号项；开机自启只用 HKCU Run（上游管理员计划任务未实现，需文档化）；accept 循环无退避；sidecar 退出不停 helper 核心；PAC 服务器单线程且 `read_line` 无上限；journal Finalized 写入忽略错误。
- **Flutter**：合并后的 apply 继承旧入队时间戳；关机停止步骤超时 5 s vs 命令 2 min；runtime refresh 4 次后丢弃快照却正常返回；路由窗口 `_commitAction` 无 busy 保护；DNS 保存后设置 revision 缓存陈旧、`save_dns` 无期望 revision；损坏的 UI 状态文件会被下一次分段保存覆盖；`stopScheduler` 忽略结果；订阅 `cancel`/`lastJobId` 处理；Windows 专有菜单项在所有平台显示；`_ComboField` 在 build 中创建控制器；备注对话框控制器未释放；主题对话框用打开时快照写整个 UiItem 组；快捷键录制把 Esc 记为绑定；多处硬编码颜色；预览不显示备注；smoke/bench/`V2RAYN_R_OPEN_*` 钩子随发布版发布。
- **Rust（Dart 范围外）**：区域预设 Default 分支忽略 DNS 行删除错误，且 persist 失败不回滚 revision（`engine.rs:1818-1837`）。

## 6. 死代码（扫描确认，零引用）

**Rust**：`monitor.rs` `active_index`/`clear_node`/`refresh_interval`/`set_visible`；`recoverable_commit` `stored_completion`；`store_repo` `to_domain_error`；`update_service` `with_app_exe_name`；`config_codegen` `diff::semantically_equal`、`util::put_opt_f64`；`core_adapters` `log_stream::push_event`、`stats/xray::with_secret`；`domain` `prev_profile_ref`/`next_profile_ref`/`outbound_ref`/`invalid_argument`/`security_kind`/`ledger_id`/`outbound_reference`/`needs_core_restart`/`needs_app_restart`；`platform` `text_lossy`/`bound_addr`/`acquire_default`/`default_proxy_string`/`windows_bypass`；`runtime` `resolve_exe_str`；`subscriptions` `remark_fragment`/`to_uri_for_finalmask`；`updater` `from_current_exe`；`privileged_helper` `clear_already_gone`/`pending_cleanup`；另有 `domain::routing` 的 `DOMAIN_STRATEGIES_SBOX`/`RULE_PROTOCOLS`/`RULE_NETWORKS`。
另（代理）：`engine.rs` `set_profile_sort`、`refresh_subscriptions`、`write_app_log`、`commit_import_batch`；`backup_service::zip_bundle`；`UpdateService::apply_app_upgrade`；`resource_auto_update.dart` 及 `resource_auto_update_now`；`ping_profile`。

**Dart**：`isPreservedOnly`、`selectRouting`、`clearNodeStat`、`groupChildIds`、`supportsMux`/`supportsUot`、`handleDoubleClick`（与 `profiles_table` 的 `_onDoubleTap` 重复）、`pingSelected`、`runBlockingProbe`、`defaultText`/`validateSimple`、`clearAvailabilityNotice`、`forAction`、`startPacFromFile`、`expireForTest`、`exportProfilesToFile`；`reloadPaged`、`selectAllAcrossPages`、`queryAllSummaries`。
排除误报：`onWindowClose`、`didChangeAppLifecycleState`、`shouldReload`、`L10nContext`、`AppSemanticContext`（框架回调/扩展）。

## 7. 复核结论

| 项 | 结论 |
|---|---|
| U1 托盘左键对最小化窗口 | 确认 |
| U2 非 Windows 关闭跳过关机序列 | 确认 |
| U3 分组下拉悬空订阅 id | 确认 |
| U4 模板保存不重载 | 确认 |
| U5 预览备注分歧 | **推翻**（预览不显示备注，提交路径不改写备注） |
| U6 切协议保留旧字段 | 确认（部分） |
| C1/C3/C4/C5/C6/C8/C11 | 确认 |
| C2 SpeedPingTestUrl | 部分：测速已下发；Clash 延迟探测未下发 |
| C7 路由并发提交 | 部分：控制器提交同步，竞态在窗口层 `_commitAction` |
| C9 | 部分：设置 revision 陈旧、DNS 读取失败 revision=0 确认；「Default 预设不 bump revision」**推翻**（`engine.rs:1818` 已 bump） |
| C10 | 部分：替换导入后选中陈旧确认；状态消息重现未追踪 |
| E1–E10 | 全部确认（E7 为生产无调用方） |
| R1–R3 | 确认；R4 部分（各操作均写状态，但状态条不可见、忙时被「处理中…」遮盖、选择器异常被当取消） |

## 8. 覆盖范围与限制

- 已审：`config_codegen`（xray/singbox 全部）、`subscriptions` 主体、`updater`、`upgrade_runner`、`application`（update_service/webdav/backup_service/subs/speedtest/部分 engine）、`bridge_api` 关键 API、`persistence` 部分、`platform`/`runtime`/`ipc_contract`/`net_host`/`privileged_helper` 主体、Flutter 控制器/视图/桥接主体。
- **未审或仅抽样**：`engine.rs` 约 7000 行中大部分；`persistence` 的 candidate/backup/blobs/references/validate/mapping；`fmt/anytls`、`fmt/naive`、`fmt/wire`；`bridge_port.dart` 约 2800 行；`routing_windows.dart` 仅定向读取；`tray_menu_model`、`side_tabs`、`column_settings_dialog` 等；托盘菜单与上游状态栏菜单的对齐；各 crate 测试代码。
- 8 个代理中有 6 个因额度中断后以 Haiku 重跑；代理报告未逐条复核（§0）。
- 未运行：Windows 构建与任何运行时验证、`net_host` 测试、`scan_image_qr`/`scan_screen_qr` 测试；未验证：Windows 注册表/命名管道/TUN/文件锁行为、Linux/macOS 更新与 TUN。
- 所有发现均为静态阅读 + 上游对照；未通过运行复现。

## 9. 建议的处理顺序

1. **先修（生成配置/数据正确性）**：H1、H2、H3、H4、H6、H7、H8、H9、H10。
2. **更新与内核可用性**：H12–H15（含停止/重载流程、Xray 资源路径、Linux 可执行位、mihomo gz）。
3. **特权与网络状态安全**：H16（token 比对）、H17–H19（TUN 租约与系统代理恢复）。
4. **前端流程**：H20–H22，其余 MED 中的状态陈旧/静默失败类。
5. **依赖**：`cargo update -p h2 -p rustls -p rustls-webpki`，替换 `rustls-pemfile`，移除 `cupertino_icons`。
6. **清理**：§6 死代码；统一三份 `CompareProfileItem`；发布版中移除 smoke/bench/evidence 钩子。
