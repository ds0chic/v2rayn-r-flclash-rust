# R4-34 资源源和后台任务 — 证据

状态：implemented（Geo/SRS 资源周期任务后端已实现并用虚拟时钟 + 本地 mock 端点验证；Dart 契约/复现测试先失败后通过）。真实远端下载、逐消费者全覆盖、每日检查更新与 FRB/Dart 消费者登记为 blocked/未验证。

HEAD：`06f61f6`（本轮执行起点）。冻结上游：v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
上游对照：`UP/v2rayN/ServiceLib/Manager/TaskManager.cs`（`UpdateTaskRunGeo` / `UpdateTaskRunSubscription` / `UpdateTaskRunCheckUpdate`）、`UP/v2rayN/ServiceLib/Services/UpdateService.cs:139`（`UpdateGeoFileAll`）、`:515`（`GetSrsFileRequest`）。
armed=false；未占用/未修改 127.0.0.1:10808；未改宿主系统代理/注册表/路由/TUN/自启；未读写用户凭据；测试端口 ≥11808 且先探测；只停止本项目启动且持有会话记录的进程（本卡未启动任何内核/服务）。

## 本次唯一用户流程

保存资源源或全局 `GuiItem.AutoUpdateInterval` 后，Geo/SRS 资源任务按上游周期实际下载、成功落盘、失败明确报错不假成功；停止后无残留计时器。

## 对应库存与状态

| 台账 ID | 内容 | 本卡状态 | 说明 |
|---|---|---|---|
| BG-TASK-001 | 定时任务循环（每 1 分钟） | implemented | `ResourceScheduler` 线程按 tick 循环；与订阅 scheduler 同启停 |
| BG-TASK-002 | 订阅自动更新周期 | preserved_only | FIX-09D 既有 `SubScheduler`，本卡未改语义 |
| BG-TASK-003 | Geo 文件自动更新周期 | implemented | `engine.run_resource_pass` + `ResourceScheduler`，`AutoUpdateInterval` 小时门控 |
| BG-TASK-004 | 每日检查更新任务 | blocked | 见下「接口缺口/阻塞」；R4-29/32 域 |
| BG-TASK-005 | 统计采集后台循环 | not_applicable | 运行/监控域（R4-23），不在本卡 |
| BG-TASK-006 | 配置持久化定时（每 20 分钟） | blocked | 未实现；需确认生效语义与文件所有权 |
| SCH-003 | 下载文件到本地 Geo 文件 | implemented（部分消费者） | Geo `.dat` + 默认 geosite SRS 已下载；DNS 派生的 geoip SRS 名单 blocked |
| SCH-004 | 每日检查更新 | blocked | 同上 BG-TASK-004 |
| ACT-MAIN-032/033/034 | 区域预置（默认/俄罗斯/伊朗） | preserved_only | R4-16B 资源源解析已存在；区域预置写 URL 由既有实现保留 |

## 关键改动

- `crates/application/src/engine.rs`：
  - `build_resource_requests(const_item, bin_dir)`：按 `effective_geo_source`/`effective_srs_source`（内置或设置覆盖）生成 Geo `.dat`（geoip/geosite）与默认 geosite SRS 下载集，目标路径镜像上游 `Utils.GetBinPath`（`<bin>/<name>.dat`、`<bin>/srss/<type>-<name>.srs`）。
  - `AppEngine::run_resource_pass(bin_dir, now_hours, cancellation)`：上游 `UpdateTaskRunGeo` 语义——`AutoUpdateInterval<=0`、`now_hours==0` 或非整数倍时 `due=false` 不下载；每文件先下到 `<target>.part`，成功才 rename 覆盖，失败清理暂存并落 `ResourceFailure{url,code,detail}`，绝不假成功、绝不覆盖旧文件。
  - `ResourceScheduler`：独立线程 + 当前线程 Tokio runtime，tick 分钟计数，`ticks % 60 == 0` 触发资源 pass；`stop()` 经 channel 立即唤醒，`Drop` 只发停止信号并 detach（不 join，避免拖慢退出，保 FIX-09D「不阻塞进程退出」）。
  - `start_sub_scheduler` 同时启动资源任务；`stop_sub_scheduler` 同时停止两者；新增 `start/stop/resource_scheduler_running`。
- `apps/desktop/lib/features/settings/resource_auto_update.dart`：`ResourceAutoUpdatePlan` + `resourceAutoUpdatePlan` + `resourcePassDue`，UI 侧周期/资源源契约（与上游节奏一致）。
- `apps/desktop/test/r4_34_contract_test.dart`（新）、`apps/desktop/test/repair/r4_34_repro_test.dart`（新）。

## 断言复现（先失败后通过）

- pre-fix：`flutter test test/repair/r4_34_repro_test.dart` → 编译失败：`Method not found: 'resourceAutoUpdatePlan'` / `'resourcePassDue'`（见 repro-prefix.log），exit≠0。
- post-fix：同一复现测试 3/3 通过；契约测试 6/6 通过；合并 9/9 通过。

## 命令与结果

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | exit 0（无输出） |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | exit 0（`Finished`，无 warning） |
| `cargo test -p application --lib --locked resource` | exit 0，6/6（含 11808+ mock 下载、失败保旧文件、停无残留、同启停） |
| `cargo test --workspace --locked` | exit 0，全部 `test result: ok`，0 failed |
| `flutter analyze` | exit 0，`No issues found!` |
| `flutter test test/r4_34_contract_test.dart` | exit 0，6/6 |
| `flutter test test/repair/r4_34_repro_test.dart` | exit 0，3/3 |
| `flutter build windows --release` | 未运行（无桥/FRB 改动，按卡条件不需要） |

## 接口缺口 / 阻塞

1. FRB/Dart 消费者缺口：本卡按「不改 `frb_generated`」约束，未新增 Dart 可调用的资源任务入口。资源任务经既有 `start_sub_scheduler` 链路被普通启动的 Dart `startScheduler` 间接启动，但缺少显式「立即更新 Geo」「资源任务状态/上次结果」入口。建议由整合者固定新桥函数（提供方 bridge_api/subs.rs；调用方 subs_controller/settings；输入 now/消费者；输出结构化报告；错误可读可重试）。
2. 每日检查更新任务（BG-TASK-004 / SCH-004）：需复用 `update_service`/`updater`，涉及网络与版本策略，与 R4-29/32 重叠；未实现，登记 blocked。
3. 逐消费者覆盖不完整：当前实现 Geo `.dat`（geoip/geosite）+ 默认 geosite SRS（google/cn/geolocation-cn/category-ads-all，上游 append 集）。尚未覆盖：从 sing-box DNS 配置派生的 geoip/geosite SRS 名单、`GetOtherFilesRequest`、区域/证书源（`cert_sources`）、路由 DNS 模板源（`RouteRulesTemplateSourceUrl`）与 SubConvert 的下载消费者。均已登记，未自行删需求。
4. 真实远端下载：仅本地 mock（127.0.0.1:11808+）验证，真实 GitHub 源下载未验证，登记 blocked/未验证。
5. 「重开保持」的真实重开：Rust 侧 `AppEngine::open` 已复用同一 `data_dir/bin`，但未在真实 GUI 重开后核对下载文件，登记未验证。

## 未完成 / 下一步前置

- 需整合者固定资源任务桥函数并再生成 FRB，才能由设置窗/底栏直接触发与展示结果。
- 需补 DNS 派生 SRS 名单、other/证书/区域/路由模板消费者，并做真实重开与真实远端验证。
