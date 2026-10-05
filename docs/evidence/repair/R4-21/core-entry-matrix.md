# R4-21 逐核普通入口适用性矩阵（Windows x64）

冻结上游：`2dust/v2rayN@7d6a967`（7.25.4），`CoreInfoManager.cs` / `CheckUpdateViewModel.cs`。
本仓基线：`a95897f`（执行前）。平台：Windows 11 x64 25H2。

分类来源：
- `Auto`（可用，自动下载+校验+安装）：`updater::channel::is_check_update_supported` 为真，
  且上游 `CoreInfoManager` 定义了对应 `DownloadUrlWin64`。
- `Manual`（需手动安装）：14 核适配器与选核路径可用，但上游未定义 `DownloadUrl*`，
  须由用户把二进制放入受管目录 `<data>/cores/<dir>/<version>/<exe>`；更新窗口显式列出并标注
  "需手动安装"，不静默遗漏。
- `Blocked`（不可用）：无适配器或入口受限。14 核中无 `Blocked`。

历史缺陷：`update_service.rs::UI_TARGETS` 缺 `hysteria` 与 `v2fly_v5` 两行（静默遗漏），
且把全部非自动核一律标注 `error.update_unsupported`。本卡补齐为 14 核并区分 `manual`。

| # | 更新键 | 仓库 | 普通更新入口 | 原因 | adapter 启动契约（crates/runtime/src/adapter.rs） | 选核/运行路径 |
|---|---|---|---|---|---|---|
| 1 | xray | XTLS/Xray-core | Auto 可用 | 上游 `DownloadUrlWin64` 全 | `run -c {cfg}` | 可用 |
| 2 | sing_box | SagerNet/sing-box | Auto 可用（上限 1.14.x） | 上游 `DownloadUrlWin64` + `LockedMaxVersion` | `run -c {cfg}` | 可用 |
| 3 | mihomo | MetaCubeX/mihomo | Auto 可用 | 上游 `DownloadUrlWin64` 全 | `-f {cfg} -d {dir}` | 可用 |
| 4 | v2fly | v2fly/v2ray-core | 需手动 | 上游无 `DownloadUrl*` | `-config {cfg}` | 可用 |
| 5 | v2fly_v5 | v2fly/v2ray-core | 需手动 | 上游无 `DownloadUrl*` | `run -c {cfg}` | 可用 |
| 6 | hysteria | apernet/hysteria | 需手动 | 上游无 `DownloadUrl*` | `args=[]`，cwd=配置目录 | 可用 |
| 7 | hysteria2 | apernet/hysteria | 需手动 | 上游无 `DownloadUrl*` | `args=[]`，cwd=配置目录 | 可用 |
| 8 | naiveproxy | klzgrad/naiveproxy | 需手动 | 上游无 `DownloadUrl*` | `{cfg}`（位置参数） | 可用 |
| 9 | tuic | EAimTY/tuic | 需手动 | 上游无 `DownloadUrl*` | `-c {cfg}` | 可用 |
| 10 | juicity | juicity/juicity | 需手动 | 上游无 `DownloadUrl*` | `run -c {cfg}` | 可用 |
| 11 | brook | txthinking/brook | 需手动 | 上游无 `DownloadUrl*` | `" {cfg}"`（脚本文件） | 可用 |
| 12 | overtls | ShadowsocksR-Live/overtls | 需手动 | 上游无 `DownloadUrl*` | `-r client -c {cfg}` | 可用 |
| 13 | shadowquic | spongebob888/shadowquic | 需手动 | 上游无 `DownloadUrl*` | `-c {cfg}` | 可用 |
| 14 | mieru | enfein/mieru | 需手动 | 上游无 `DownloadUrl*` | `run` + env `MIERU_CONFIG_JSON_FILE={cfg}` | 可用 |

附：应用身份 `v2rayN`（非内核）在未配置自有发行源时 Blocked，提示
`error.update_app_source_unconfigured`；发行版内为 `error.update_unsupported`（R3-08）。

一致性：
- `crates/runtime/src/adapter.rs::adapter_for` 对 `CoreType::PROXY_CORES` 全部 14 项返回适配器
  （测试 `app_identity_has_no_adapter_but_every_proxy_core_does`、`update_core_key_round_trips_every_proxy_core`）。
- `crates/application/src/update_service.rs::builtin_targets(false)` 返回 15 行（app + 14 核），
  `proxy_update_cores()` 由 `CoreType::PROXY_CORES` 派生，无法静默遗漏。
- 运行时逐核真实启动 + 合成 HTTP：见 `local-install-launch.json`（xray/mihomo 实测；其余 12 核
  的 mesession 结果见 `docs/evidence/recheck-fixes/R3-CORE-MATRIX/`）。
