# FIX-16C ClashUIItem 刷新/排序字段的 Clash UI 消费者

状态：`implemented`（字段保存→重开→monitor/proxies 视图按设置刷新/排序已在本机 widget 测试验证；真实内核/窗口集成未运行，见「未完成」）。

任务 ID：FIX-16C（来源 `docs/tasks/FIX-16.md` 后续卡表；`docs/evidence/parity-review-2026-10-03/repair-queue.md` 第 41/43 行；findings SET-17、RT-17，关联 SCH-006/007）。

本次唯一用户流程：在「参数设置 → 显示 → 历史保留（原版 Clash UI 设置）」编辑 `ProxiesSorting` / `ProxiesAutoRefresh` / `ProxiesRefreshInterval` / `ConnectionsAutoRefresh` / `ConnectionsRefreshInterval` →「保存」→ 关闭再重开，字段值从 `guiNConfig.json` 深拷贝草稿恢复；进入 monitor 的 Clash 代理/连接页，代理列表按 `ProxiesSorting`（0=延迟 / 1=名称）排序，自动刷新按 `*AutoRefresh` 与 `*RefreshInterval`（秒，`<=0` 关闭）驱动；设置窗口与 monitor 同开时保存后立即生效。

前置任务及已验证证据：FIX-16 已收敛五页分组并把 ClashUIItem 放进「历史保留」段；FIX-08 草稿保存语义、FIX-11/11B 统计轮询与 Clash 动作语义本卡不回退。审计证据：`settings-report.md` §5、`runtime-report.md` RT-17、`all-items.csv` FLD-CFG-131..136、SCH-006/007。

对应 feature / field / action / layout ID：`FLD-CFG-131`（ProxiesSorting）、`FLD-CFG-132`（ProxiesAutoRefresh）、`FLD-CFG-133`（ProxiesRefreshInterval）、`FLD-CFG-134`（ConnectionsAutoRefresh）、`FLD-CFG-135`（ConnectionsRefreshInterval）、`FLD-CFG-136`（ConnectionsColumnItem，仍缺）、`LAY-CLASHPROXY-001`、`LAY-CLASHCN-001`、`SCH-006`、`SCH-007`、`SET-17`、`RT-17`。

必读上游文件、符号和固定 commit（`7d6a967c18c697f28dc6917122ed3a4993fcf336`）：
- `v2rayN/ServiceLib/Models/Configs/ConfigItems.cs:210`（`ClashUIItem` 8 属性，`ProxiesRefreshInterval`/`ConnectionsRefreshInterval` 默认 2）
- `v2rayN/ServiceLib/ViewModels/ClashProxiesViewModel.cs:33/48/88/121/126/258`（`AutoRefresh`/`SortingSelected` 读设置并回写；`GetClashProxiesTask` 1s tick 门控；`RefreshProxyDetails` 0=Delay 1=Name 排序）
- `v2rayN/ServiceLib/ViewModels/ClashConnectionsViewModel.cs:8/14/127/137/142`（`ConnectionsAutoRefresh` 读设置并回写；同款 interval 门控）

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：设置草稿的 `ClashUIItem` 组；monitor 视图对 `ref.read(settingsControllerProvider).document` 的只读派生。
- 输出：保存走既有 `saveDocument`（无新增 IPC）；视图内联切换写回走既有 `saveGroup('ClashUIItem', ...)`（追加不改生成文件）。
- 错误：设置文档未加载时不做写回，不伪造成功。
- 取消：视图 dispose 取消定时器；设置「取消」丢弃草稿（沿用 FIX-08）。
- 权限：仅本机 UI + FRB/SQLite；不启动内核、不写系统代理/TUN、不监听端口。
- 持久化：`guiNConfig.json` 的 `ClashUIItem`；重开一致。
- 生效：`immediate`。设置文档变化经 `clashUiConfigProvider` 通知已打开的代理/连接视图，重建定时器并按新排序刷新。

允许修改的模块：`apps/desktop/lib/features/monitor/**`（新增 `clash_ui_config.dart`；改 `proxies_view.dart`、`connections_view.dart`）、`apps/desktop/lib/features/settings/option_setting_window.dart`（Clash 历史段）、`apps/desktop/test/**`、`docs/evidence/UX-PARITY-FIX-16C/**`、`docs/evidence/UX-PARITY-FIX-16/field-matrix.md`、本卡、`compat/`（仅追加）。未改 Rust、未改生成文件、未改 `main_shell.dart`/`app.dart`/`features/profiles|subs|runtime|routing|update|backup`。

禁止改变的已有行为：FIX-08 草稿保存、FIX-11/11B 统计轮询与 Clash 动作语义、`settings-apply`/`settings-save`/`settings-local-port` 键、菜单结构；不删入口、不降分母、不伪造平台效果。

测试夹具和原版预期：`FakeMonitorBridge`（内存代理/连接）；`SettingsController` 测试子类注入固定 `ClashUIItem` 文档。原版预期：`ProxiesSorting=0` 按延迟升序（超时最后）、`=1` 按名称升序、其它值保持核心顺序；`AutoRefresh && interval>0` 才按 interval 秒刷新。

本次必须通过的命令（实际运行见「本轮实际结果」）：
- `dart format`（改动文件）
- `flutter analyze lib/features/monitor lib/features/settings`（含新测试与夹具）
- 针对性 `flutter test`（新测试 + `fix11b_clash_panel_test`、`t15a_proxies_test`、`t15a_connections_test`、`fix16_settings_field_test`、`t12a_option_window_test`）
- 真实窗口集成测试：本卡未运行

证据文件位置：`docs/evidence/UX-PARITY-FIX-16C/README.md`、`observations.json`；字段状态并入 `docs/evidence/UX-PARITY-FIX-16/field-matrix.md`。

完成条件：ClashUIItem 的刷新/排序字段从设置到 `proxies_view`/`connections_view` 的真实消费者链路可验证（保存→派生→视图刷新/排序），既有测试不回退，门禁通过。已达成（真实内核/窗口集成除外）。

接口缺口处理（登记，不自行削减）：
- `ClashUIItem.ConnectionsColumnItem`（连接表列名/宽度/索引恢复）：需要列拖拽/宽度状态与 `StorageUI` 回写，属 LAY-CLASHCN-002；本卡不接，登记后续。
- `ClashUIItem.EnableIPv6` / `EnableMixinContent`：`restart_core`，Rust `crates/config_codegen` 未消费（`grep` 无命中），需 cfg_codegen 接线；本卡不实测，登记后续。
- `SCH-006/007` 原版额外门控 `ShowInTaskbar && IsRunningCore(sing_box)`：本卡只按 `AutoRefresh && interval>0` 刷新；核心运行态门控依赖 runtime 会话，登记后续（本卡不改 FIX-11/11B 语义）。

本轮实际结果：
- 新增 `features/monitor/clash_ui_config.dart`：`ClashUiConfig` + `clashUiConfigFromDocument` + `clashUiGroupWith` + `clashUiConfigProvider`。
- `proxies_view.dart`：删除硬编码 2s 与本地 `_sortDescending`，改为读配置初始化、`ref.listen` 响应设置变化；排序对齐上游（0=延迟升序超时最后 / 1=名称升序）；自动刷新间隔取自 `ProxiesRefreshInterval`；手动开关/排序写回 `ClashUIItem`。
- `connections_view.dart`：同样接入 `ConnectionsAutoRefresh`/`ConnectionsRefreshInterval`，写回配置。
- `option_setting_window.dart`：Clash 历史段把 `ProxiesSorting` 改为 0/1 下拉，补 `ConnectionsAutoRefresh`/`ConnectionsRefreshInterval` 控件。
- 测试：`test/fix16c_clash_ui_config_test.dart` 8/8 通过；回归 5 个相关测试文件 14/14 通过；`flutter analyze` 目标目录 `No issues found`。
- 未改 Rust / 生成文件；未跑全量 workspace 测试与 `flutter build windows`（按要求）。

## 后续（登记，未在本卡完成）

| 项 | 范围 | 未完成原因 |
|---|---|---|
| FIX-16C-2 | `ConnectionsColumnItem` 列状态持久化（Index/Width 恢复 + 写回） | 需列拖拽/宽度状态与 StorageUI 回写；LAY-CLASHCN-002 |
| FIX-16C-3 | EnableIPv6 / EnableMixinContent 的 config_codegen 消费者 | `crates/config_codegen` 未接线；需内核/生成断言 |
| FIX-16F | 真实 Windows 窗口集成测试（设置保存→重开→Clash 页刷新/排序） | 环境抖动，留给根代理统一跑 |
