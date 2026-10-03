# UX-PARITY-FIX-16C evidence

FIX-16C：`ClashUIItem` 刷新/排序字段的 Clash UI 消费者（代理/连接页）。

## 交付

- 新增 `apps/desktop/lib/features/monitor/clash_ui_config.dart`：`ClashUiConfig`、`clashUiConfigFromDocument`、`clashUiGroupWith`、`clashUiConfigProvider`。
- `proxies_view.dart`：排序由 `ProxiesSorting`（0=延迟 / 1=名称）驱动；自动刷新由 `ProxiesAutoRefresh` + `ProxiesRefreshInterval`（秒，`<=0` 关）驱动；设置变化经 `ref.listen` 即时生效；视图内切换写回 `ClashUIItem`。
- `connections_view.dart`：`ConnectionsAutoRefresh` + `ConnectionsRefreshInterval` 同款接入与写回。
- `option_setting_window.dart`：Clash 历史段 `ProxiesSorting` 改 0/1 下拉，新增 `ConnectionsAutoRefresh`/`ConnectionsRefreshInterval` 控件。

## 上游对照

- `ServiceLib/Models/Configs/ConfigItems.cs:210` `ClashUIItem`（`ProxiesRefreshInterval`/`ConnectionsRefreshInterval` 默认 2）。
- `ClashProxiesViewModel.cs`：`AutoRefresh`/`SortingSelected` 读设置并回写；1s tick 按 `interval` 门控；`0=Delay`、`1=Name` 排序。
- `ClashConnectionsViewModel.cs`：`ConnectionsAutoRefresh` 读设置并回写；同款 interval 门控。

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `dart format <改动文件>` | 4 changed / 6 formatted |
| `flutter analyze lib/features/monitor lib/features/settings test/fix16c_clash_ui_config_test.dart test/support/fake_monitor_bridge.dart` | No issues found |
| `flutter test test/fix16c_clash_ui_config_test.dart` | 8/8 通过 |
| `flutter test fix11b_clash_panel_test t15a_proxies_test t15a_connections_test fix16_settings_field_test t12a_option_window_test` | 14/14 通过 |

## 状态

- `verified`（本卡）：`ProxiesSorting`、`ProxiesAutoRefresh`、`ProxiesRefreshInterval`、`ConnectionsAutoRefresh`、`ConnectionsRefreshInterval`（widget 测试：配置→视图排序/刷新）。
- `registered`（未完成）：`ClashUIItem.ConnectionsColumnItem`。
- 未改 Rust / 生成文件；未跑全量 workspace 测试、`flutter build windows`、真实窗口集成。

字段状态已并入 `docs/evidence/UX-PARITY-FIX-16/field-matrix.md`。
