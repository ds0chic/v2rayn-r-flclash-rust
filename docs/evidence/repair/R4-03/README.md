# R4-03 缺核恢复与配置校验有界 — 证据

任务：docs/repair/tasks/R4-03.md
HEAD（执行前）：3426a1a336967d58aa0c7d6a4c7717398ab3e516
时间：2026-10-05（本地）
armed=false；未连接命名管道；未碰 127.0.0.1:10808；未改宿主系统代理/注册表/路由/TUN/自启；
未读用户凭据；夹具均为合成数据。

## 结论（本卡合同）

1. 正式下载目录定位：`CoreLocator` 默认受管根 `<data>/cores`（与 `AppEngine::managed_cores_root`
   一致，RR-01），生产以 `V2RAYN_R_CORES_ROOT` 为唯一根；开发树 `tools/cores` 与
   `V2RAYN_R_XRAY_BIN` 仅当显式 `V2RAYN_R_DEV_MODE` 时生效。已实现。
2. 坏核（0 字节）/空格路径/端口冲突可读可重试、不假 Running：0 字节核在定位阶段返回
   `error.core_invalid`；空格路径用路径 API/`OsString` 正常解析；端口冲突由预检
   `preflight_port` 在停旧会话前返回可读错误；非空但损坏的核由核自身 `test_args`
   预检返回 `error.config_check_failed`，不产生假 Running（预检失败保旧会话）。
3. 预检 `Command.output` 有执行 deadline 且超时会终止受管子进程：`run_config_check` 改走
   `run_bounded_output`（tokio Child `kill_on_drop` + `tokio::time::timeout`，
   默认 `V2RAYN_R_CONFIG_CHECK_TIMEOUT_MS=30000`，错误 `error.config_check_timeout`）。
4. 修好可重试：`CoreLocator` 每次预检重新解析受管根，安装/替换核后再次 apply 会重新定位；
   与 R4-01 的重试路径衔接（R4-01 已有）。
5. 缺核 UI：update 窗口新增“缺少内核”区块与“安装缺失内核”入口，接既有 T16
   check→apply 流程；失败逐字可见且保留可重试。

## 改动文件

- crates/runtime/src/adapter.rs（定位/dev 门控/0 字节校验；测试）
- services/net_host/src/session.rs（有界预检执行 + 超时测试；测试 harness dev 模式）
- apps/desktop/lib/features/update/update_controller.dart（missingCores + installCores）
- apps/desktop/lib/features/update/check_update_view.dart（缺核入口 UI）
- apps/desktop/test/r4_03_contract_test.dart（新增行为测试）
- docs/repair/tasks/R4-03.md、docs/evidence/repair/R4-03/**、compat/features.yaml（追加注释）

未改动：main_shell.dart、app.dart、两处 frb_generated、lib/bridge/api/**、features/runtime/**、
crates/application/src/{subs,monitor,speedtest,backup_service,net_host_client}.rs、
crates/updater/**、crates/application/src/update_service.rs。engine.rs 未改（无需）。

## 上游对照

- `U/ServiceLib/Manager/CoreInfoManager.cs:32-51 GetCoreExecFile`：按 `CoreExes` 在 bin 路径查找，
  未找到给 `NotFoundCore`。本实现定位语义一致，并额外对 0 字节核给可读 `core_invalid`。
- `U/ServiceLib/ViewModels/MainWindowViewModel.cs:664-752 Reload`：Reload 串行
  （`_hasNextReloadJob` 合并），核心加载 `LoadCore`。本卡只保证预检有界与缺核入口。
- `U/ServiceLib/ViewModels/CheckUpdateViewModel.cs:320-329 UpdateFinishedResult`：更新完成发布
  `ReloadRequested`，即“装好”后走重载/重试。本卡 UI 在安装成功后提示“可再次启动”，与 R4-01
  重试路径衔接。

## 未完成 / 未实测（honest）

- 未运行真实 GUI：未在缺核→安装→同一 GUI 再次启动的真实入口复跑；`flutter analyze`、
  `flutter test test/r4_03_contract_test.dart` 通过，但均为合成桥。
- 真实内核缺核/坏核/空格路径/端口冲突的端到端运行未在新包装上实跑（无武装包）。
- 未实测 OS 级效果（系统代理/TUN/路由），本卡不涉及。
- `cargo test --workspace` 观察到 net_host 会话测试存在既有的共享环境变量
  `V2RAYN_R_XRAY_BIN` 并发竞争偶发（重跑通过）；非本卡引入，未在本卡修复。
