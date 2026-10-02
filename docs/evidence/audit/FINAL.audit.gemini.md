# 终审审计报告 — Gemini 3.8 Flash
- 时间: 2026-10-02
- 审计者: Gemini 3.8 Flash (antigravity/gemini-3.8-flash-high)
- 子代理: explore (`ses_f058875ffffeh11Q696eEtt4U1`, OpenCode Muse Spark 1.3 contributor 内置探索引擎，仅单层派发并经亲自抽查复核)
- HEAD: `5c33fa5f08bc063e938c0c339d2a96bf01662313`（工作区仅包含 `FINAL.audit.prompt.md` 提示词，无任何污染改动）

---

## 结论

**有条件可发布（Windows x64 Release Candidate）**。

工程在宿主安全性（严格隔离 `10808`、零触碰宿主 WinINET/系统代理/注册表/TUN、精准进程树管理）、GPL-3.0 归属合规（不捆绑第三方内核二进制、完整 LICENSE 与 NOTICE）、合成夹具脱敏以及单机无特权最小闭环方面均具备极高的工程严谨性。然而，打包发布物与启动钩子中仍存在轻量环境变量后门暴露风险（`V2RAYN_R_AUTO_SMOKE` 在 release 构建下直接暴露于任意外部环境变量且未设限定保护），且台账条目存在大面积 `verified` 字段空缺（台账 verified=0，与矩阵环境冒烟范围语义未对齐），需完成必改项并在发布文档中显式声明限制后方可放行。

---

## 必查项结论表（A1..F）

| 编号 | 审计项 | 判定 | 关键依据与核查结果 |
|---|---|---|---|
| **A1** | 发布真实性与洁净度 | **通过** | 1. `dist/v2rayN-R-1.0.0+1-windows-x64.zip` SHA-256 经实测复算为 `4853F06D66791ADD9C68234C91FF5CA3C4AC9DC5CC6E97E2A159FFDC5A208255`，与 `dist/SHA256SUMS` 完全一致。<br>2. 压缩包内包含：`v2rayn_desktop.exe`、`net_host.exe`、`privileged_helper.exe`、`bridge_api.dll`、`flutter_windows.dll`、各插件 dll、`LICENSE`、`NOTICE.md`、`README.md`、`CORE-NOTES.txt`、`build-info.json` 与 Flutter 资源。<br>3. 严格不捆绑 Xray/sing-box 二进制，`CORE-NOTES.txt` 正确指明运行时下载与锁定政策。 |
| **A2** | T20 冒烟证据链与 ISSUE-08 回归审查 | **存在中危风险 (P1)** | 1. 真实 apply 链具备实测闭环依据：`capture_apply_evidence.ps1` 驱动的独立运行产出了 `s-1790907963960-2_config.json`（真实 VLESS+mixed 入站 11808）与 `journal.json`（hash 与 rev 自洽）。运行态截图 `T20-smoke-applied-running.png` 呈现 Running 态。<br>2. **风险点（ISSUE-08 回归隐患）**：`apps/desktop/lib/main.dart:16-19` 中的 `applyOnLaunch` 读取 `Platform.environment['V2RAYN_R_AUTO_SMOKE']`。在 **Release** 构建下，任何设置了该环境变量的宿主环境在启动应用时均会被静默触发 `applyActive()`，绕过了生产环境用户界面的主动交互意图。虽有默认关闭防御，但未做 build mode 限制或限定一次性令牌保护。 |
| **A3** | 版本号与复现性诚实度 | **通过** | 1. `build-info.json` 的 `pubspec_version: 1.0.0+1` 与 `apps/desktop/pubspec.yaml:19` 一致，`rust_version: 0.1.0` 与 `Cargo.toml:20` 一致。<br>2. `build-info.json` 记录了 commit `54fcebe`（T18b 基线），并在 `docs/evidence/T20.md:70-78` 中诚实登记：由于 Windows `Compress-Archive` 写入时间戳导致 zip 二进制哈希不可复现，改用排除 `build-info.json` 后的成员清单哈希作为可复现性度量。 |
| **B** | 全产品反造假抽查（≥10项） | **全部通过** | 抽查 10 项核心业务逻辑（含订阅事务、11 协议增删改查、180 字段持久化、Fake 系统代理编排、TUN dry-run、核心错误注入等），代码逻辑、测试套件与方向均真实存在且断言明确。抽查项全部归为“可信”。 |
| **C** | 宿主安全与隔离 | **通过** | 1. 全仓检索 `10808` 端口：所有出现均为防御性常量守卫（如 `RESERVED_LIVE_PORT = 10808`）、配置校验拦截（遇到 10808 报错拒绝）或负向断言测试，没有任何实际监听或发起连接的行为。<br>2. 测试运行端口均自适应分配或使用 `>=11808`。<br>3. 系统代理与注册表在测试中全部使用 `FakeRegistry` 与 `FakeSystemProxyBackend`，未触碰真实 WinINET API。<br>4. 冒烟测试脚本 `smoke_windows.ps1` 与 `capture_apply_evidence.ps1` 仅通过明确记录的 PID 及其子进程树执行清理，未进行按进程名的盲杀。 |
| **D** | 秘密与合规 | **通过** | 1. `fixtures/synthetic/` 与 `fixtures/source/` 经检索未见任何真实订阅 URL、节点密码或敏感令牌；仅见 `192.0.2.10`（RFC 5737 测试地址）与 `cloudflare-dns.com` 公共 DoH。<br>2. `NOTICE.md` 清晰载明 2dust/v2rayN 上游 GPL-3.0 归属与 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`，并明确了不捆绑内核与各自许可证。 |
| **E** | 台账终态与缺口 | **存在统计不一致 (P2)** | 1. `compat/*.yaml` 台账中 `status` 分布为：`identified: 584`，`implemented: 608`，`preserved_only: 11`，**`verified: 0`**。<br>2. 而 `compat/platform-matrix.md` 将 Windows x64 标为 `verified`。两者在“已验证”语义定义上分离未说明。抽查台账声称 `implemented` 的条目代码均存在，但部分缺乏显式 `test_ids` 字段索引。 |
| **F** | 交付定论 | **有条件可发布** | 允许作为 Windows x64 RC 内部测试发布，但必须修复/收紧环境变量钩子，并在发布 Release Notes 中明确说明未测试平台及未经验证的真实 TUN/系统代理特权执行边界。 |

---

## 发现清单

| 编号 | 严重度 | 类型 | 文件:行 / 证据 | 反证 / 事实 | 最小修复建议 |
|---|---|---|---|---|---|
| **ISSUE-F-01** | **P1** | 安全隐患 / 测试钩子泄漏 (回归 ISSUE-08) | `apps/desktop/lib/main.dart:16-19` 及 `apps/desktop/lib/app/app.dart:41-43,96` | 历史 ISSUE-08 将 `V2RAYN_R_AUTOSTART` 限制在 `kDebugMode`；而 T20 为完成干净环境打包冒烟，在 `main.dart` 中引入了 `V2RAYN_R_AUTO_SMOKE`。该变量在 **Release** 构建下依然可被外部环境传入并直接触发 `applyActive()`，使发布包留有非交互式自动启动后门。 | 将 `applyOnLaunch` 限制为仅在特定非生产编译标识（如 `--dart-define=ENABLE_SMOKE_HOOK=true`）或仅在附加专用临时认证握手文件时生效；或者在 release 构建代码中彻底去除环境变量检测。 |
| **ISSUE-F-02** | **P2** | 证据记录不一致 | `docs/evidence/T20.screenshots/T20-smoke-summary.json:70,75-76` vs `docs/evidence/T20.md:128-130` | `T20-smoke-summary.json` 中第三个测试项 `seed_apply` 的原始记录显示 `applied=false`、`has_core_descendant=false`、`port_11808_listening=false`。虽然 `T20.md` 解释此系检测时延与提前拆除导致的偏差，并在 `capture_apply_evidence.ps1` 的独立运行中补全了证据，但这会导致自动化元数据解析器误判冒烟失败。 | 重新运行一次整合的 `smoke_windows.ps1`（使用修复后的 300ms 探测时延），使得 `T20-smoke-summary.json` 中的 `applied` 字段最终落地为 `true`。 |
| **ISSUE-F-03** | **P2** | 台账与平台矩阵语义割裂 | `compat/*.yaml` 全体 vs `compat/platform-matrix.md:10` | 台账总计 1203 条记录中 `verified` 数量为 0（全部为 `identified` 或 `implemented`）；而 `platform-matrix.md` 中声称 Windows x64 为 `verified`。缺少在文档中明确解释“台账 item 级 verified 需全量端到端自动化覆盖，目前仅平台级冒烟完成 verified”的层级定义。 | 在 `compat/platform-matrix.md` 与 `compat/SCHEMA.md` 中补充说明平台矩阵与功能台账的验证颗粒度差异说明。 |
| **ISSUE-F-04** | **P2** | 依赖未完全脱敏验证 | `docs/evidence/T20.md:164-171` | T20 诚实登记：未使用 Process Monitor 进行系统级系统调用/DLL 路径监控，未能严格证明打包产物在完全未安装 VS/Rust/Flutter 的裸机上无隐式 VC++ Runtime 或开发路径依赖。 | 在发布说明中明确注明：运行环境需 Windows 10/11 x64 并建议预装 Microsoft Visual C++ 2015-2022 Redistributable。 |

---

## 全产品反造假抽查表（10项详验）

针对台账与核心功能进行代码、测试及断言方向的端到端真实性抽样核验（HEAD=`5c33fa5`）：

| 序号 | 抽样模块 / 条目 | 业务声明 | 核心实现代码位置 | 测试用例及验证断言 | 结论 |
|---|---|---|---|---|---|
| 1 | **节点增删改查 (F-PROFILE-001..004)** | 11 种协议全量持久化、编辑、深拷贝带“(副本)”后缀、STALE 版本保护 | `apps/desktop/lib/features/profiles/profile_actions.dart:31,80`<br>`crates/application/src/engine.rs:323-345` | `crates/persistence/tests/profiles_persist.rs:107`<br>`save_reopen_round_trips_all_eleven_protocols` 断言 11 种协议全部 round-trip 且重新打开一致；断言 `REVISION_STALE` 拦截。 | **可信** |
| 2 | **订阅事务与更新 (T09)** | 订阅拉取更新采用事务，支持冲突回滚与安全覆盖 | `crates/subscriptions/src/update.rs`<br>`crates/application/src/engine.rs:980` | `crates/subscriptions/tests/update_transaction.rs`<br>模拟网络中断并验证 SQLite 未破坏，数据一致回滚。 | **可信** |
| 3 | **内核配置生成 (T07/T08)** | Xray 与 sing-box 独立流水线，严禁 Mihomo 强制中转，包含字段格式化校验 | `crates/config_codegen/src/xray/`<br>`crates/config_codegen/src/singbox/` | `crates/config_codegen/tests/xray_golden.rs`<br>`singbox_golden.rs`<br>与黄金输出比对 JSON 结构，并包含多组协议语法验证。 | **可信** |
| 4 | **180 字段配置持久化 (T12)** | guiNConfig.json 兼容 upstream 全部配置项，类型与层级完全一致 | `crates/domain/src/settings.rs:70-400`<br>`crates/application/src/engine.rs:2200` | `crates/domain/src/settings.rs:1246-1350`<br>`all_groups_round_trip_with_unknown_keys` 验证未知字段保留与类型保真。 | **可信** |
| 5 | **测试隔离安全守卫 (10808)** | 运行时拒绝 10808 端口绑定，杜绝污染宿主代理 | `crates/config_codegen/src/util.rs:43,259`<br>`RESERVED_LIVE_PORT = 10808` | `crates/config_codegen/tests/xray_errors.rs:85`<br>`singbox_errors.rs:90`<br>显式将入站端口配为 10808，断言生成器返回结构化错误。 | **可信** |
| 6 | **Fake 系统代理编排 (T13)** | Windows 代理四种模式编排，测试环境下完全隔离系统注册表 | `crates/application/src/platform_service.rs`<br>`crates/platform/src/windows/sysproxy.rs` | `crates/application/tests/t13_platform.rs:20-24`<br>`crates/platform/tests/sysproxy_ownership.rs:34`<br>断言在 `FakeRegistry` 与 `FakeSystemProxyBackend` 上模拟设值与恢复。 | **可信** |
| 7 | **TUN 与提权架构 (T14)** | 提权 helper 独立进程执行，白名单路径与令牌验证，非特权干跑校验 | `services/privileged_helper/src/main.rs`<br>`crates/ipc_contract/src/helper.rs` | `crates/ipc_contract/tests/` (经 `cargo test -p ipc_contract` 亲自验证 28 passed)<br>断言拒绝路径穿越、非白名单可执行文件及跨目录调用。 | **可信** |
| 8 | **运行时编排与真机连接 (T18b)** | 真实启动内核，应用混杂入站与出站路由，端口监听感知 | `services/net_host/src/session.rs`<br>`crates/application/src/engine.rs:build_runtime_plan` | `crates/application/tests/t18b_runtime_e2e.rs`<br>启动临时内核，并实际发起本地回环网络请求校验数据连通性。 | **可信** |
| 9 | **测速流水线与取消 (T15)** | Real ping 与 speedtest 支持流式结果，支持优雅并发取消 | `crates/bridge_api/src/api/speedtest.rs` | `crates/bridge_api/src/api/speedtest.rs:550-565`<br>测试驱动模拟批次推送与取消任务幂等性校验。 | **可信** |
| 10 | **备份与更新流水线 (T16)** | 本地 zip 与 WebDAV 备份还原，更新元数据版本比对与哈希强校验 | `crates/updater/src/`<br>`crates/bridge_api/src/api/t16.rs` | `crates/updater/tests/`<br>单元测试验证损坏 zip 拒绝载入、哈希不匹配阻断写入及回滚流程。 | **可信** |

---

## 台账终态统计与抽样核对

### 1. `compat/*.yaml` 状态分布汇总统计

经由子代理检索全量台账并复核：

| 账本文件 | identified | implemented | verified | preserved_only | blocked | not_applicable | 合计 |
|---|---|---|---|---|---|---|---|
| `compat/actions.yaml` | 92 | 90 | 0 | 10 | 0 | 0 | 192 |
| `compat/features.yaml` | 104 | 105 | 0 | 1 | 0 | 0 | 210 |
| `compat/fields.entities.yaml` | 25 | 152 | 0 | 0 | 0 | 0 | 177 |
| `compat/fields.settings.yaml` | 0 | 180 | 0 | 0 | 0 | 0 | 180 |
| `compat/fields.yaml` | 271 | 81 | 0 | 0 | 0 | 0 | 352 |
| `compat/layouts.yaml` | 92 | 0 | 0 | 0 | 0 | 0 | 92 |
| **总计** | **584** | **608** | **0** | **11** | **0** | **0** | **1203** |

*注：`codegen-map.*.yaml` 与 `domain-map.yaml` 为转换映射规则表，不含 `status` 字段。*

### 2. 抽查 10 条 `implemented` 条目匹配度

1. `F-PROFILE-001` (11 协议添加): 实现与持久化测试完全存在，断言方向正确。
2. `F-PROFILE-002` (协议编辑与版本冲突校验): 实现完整，`REVISION_STALE` 断言正确。
3. `F-PROFILE-003` (批量删除与级联清理): 实现完整，`removed == 2` 断言正确。
4. `F-PROFILE-004` (克隆复制): 包含 `(副本)` 后缀处理，测试断言一致。
5. `ACT-MAIN-001` (主界面添加 VMess 动作): 菜单挂载与分发已实现，测试断言正确。
6. `ACT-MAIN-002` (主界面添加 VLESS 动作): 菜单结构测试通过。
7. `ACT-MAIN-003` (主界面添加 Shadowsocks 动作): 菜单分发逻辑完全匹配。
8. `CFG-002` (Custom 自定义出站编辑): 窗口已接入，但台账仅有 evidence 字段未显式列出 `test_ids`。
9. `FLD-CFG-001` (设置字段 IndexId): `domain/src/settings.rs` 实体字段与序列化保真测试具备。
10. `FLD-CFG-002` (设置字段 SubIndexId): 具备完整的加载/存储 round-trip 测试。

---

## 发布阻断项与建议

### 1. 发布阻断项清单 (P0 / P1)

* **[P1] 环境变量自动启动后门安全收敛 (ISSUE-F-01)**:
  * **影响**: 在生产 Release 环境下，若外部环境变量注入 `V2RAYN_R_AUTO_SMOKE=1`，应用启动时将无视用户交互直接调度 `applyActive()`。
  * **处置建议**: 在 `apps/desktop/lib/main.dart` 中，将 `applyOnLaunch` 封装为仅在 `--dart-define=V2RAYN_RELEASE_SMOKE=true` 编译时开启，或在正式打包时完全剥离该变量读取。

### 2. 重要建议 (P2)

* **未签名可执行文件与 SmartScreen 警告**:
  * 当前构建的 `v2rayn_desktop.exe`、`net_host.exe` 与 `privileged_helper.exe` 均未进行数字签名（Authenticode）。在未安装自签名证书的 Windows 系统上首次运行会被 SmartScreen 拦截。需在下载页面与 Release 说明中提供哈希校验与安全提示。
* **仅限 Windows x64 平台支持**:
  * 必须在 GitHub Release 资产标题与说明中明确标明 `windows-x64-only`，注明 macOS/Linux 源码未打包，Windows ARM64 由于当前锁定 Flutter 工具链限制无法产出。
* **避免使用 10808 端口作为默认端口提示**:
  * 虽然内核配置中入站监听端口自适应，但在应用首次打开时应显式提示用户检查本地端口冲突。

---

## 盲区与审计局限

1. **未在干净无任何开发环境的 Windows 机器上实机冷启动**:
   * 当前冒烟测试虽在独立的 `$env:TEMP` 临时目录运行，但仍处于具有 Visual Studio 运行库环境的主机上。DLL 动态加载是否存在隐式 Visual C++ CRT 路径依赖未经过 API Monitor 或无运行时裸机验证。
2. **GUI 自动化点击未覆盖**:
   * 本次冒烟测试的对话框与窗口截图主要通过环境钩子（`V2RAYN_R_OPEN_*`）触发驱动，未采用 Windows UIAutomation 模拟真实用户的鼠标点击交互路径。
3. **真实特权 TUN 设备驱动安装与流量捕获**:
   * 基于项目安全红线规定，本次审计未实际安装 Wintun 驱动或创建真实虚拟网卡隧道，仅验证了 IPC 特权隔离与 DRY-RUN 配置生成的正确性。
