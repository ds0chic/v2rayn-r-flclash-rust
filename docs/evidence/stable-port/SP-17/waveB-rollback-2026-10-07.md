# Wave B rollback — FLD-CFG-131..135 (2026-10-07)

Owner: SP-17 (consumer). Local-only window: synthetic/fake bridges, no cargo,
no release build, no commit. 10808 untouched; no proxy/registry/TUN writes.

## 唯一用户流程

Clash 代理/连接页改排序/自动刷新/刷新间隔→保存→重开→轮询按 canonical
启停/变频；保存失败→可见报错 + 开关回滚到持久值（永不保留未持久 UI 值），
轮询保持 canonical（未持久值不驱动 timer）。

## 改动（仅写锁内 5 个文件）

- `apps/desktop/lib/features/monitor/clash_ui_config.dart`
  - 新增共享 `clashPollPeriod({autoRefresh, intervalSeconds})`：开 + 正间隔
    才返回周期，否则 null（停轮询，原版 parity）。两 monitor 页统一经此
    计算 timer 周期。
- `apps/desktop/lib/features/monitor/proxies_view.dart`
  - `_setAutoRefresh` / `_toggleSorting` 改 persist-first：先 `saveGroup`，
    失败→保持旧开关 + SnackBar（`proxies-save-error`，
    “代理选项保存失败，已恢复上次保存的值”），成功→从 canonical
    `_applyConfig` 同步开关 + timer。`_persistClash` 改返回 bool。
  - `initState` / `_applyConfig` 的 timer 改经 `clashPollPeriod`（canonical）。
- `apps/desktop/lib/features/monitor/connections_view.dart`
  - `_setAutoRefresh` 同 persist-first 契约，失败经既有 `_report`
    （`connections-action-error`，“连接选项保存失败，已恢复上次保存的值”）。
  - `initState` / `_applyConfig` 的 timer 改经 `clashPollPeriod`。
  - 列回写 `_persistColumns` / 右键关闭 / 日志区未动（他 workstream 在动
    日志区，本卡零触碰）。
- `apps/desktop/lib/features/settings/option_setting_window.dart`（仅 ClashUI 块级）
  - 新增 `_clashUiRollbackKeys` + `_rollbackClashUiDraft()`：in-process 保存
    失败分支在置 `_error`（已可见）的同时，把 draft 内 5 个 ClashUI 键
    reseed 回持久 canonical；host 路径跳过（canonical 归 host 引擎）。
- `apps/desktop/test/repair/wave_b_rollback_131_135_test.dart`（新，11 项）
  - `_WaveBSettings`（saveGroup 成功/恒失败注入）+ `FakeMonitorBridge`；
    合成数据，无 socket/代理/10808。

## 定向检查（exit 均为 0）

- `dart format lib/features/monitor lib/features/settings test/repair/wave_b_rollback_131_135_test.dart`
  → `Formatted 25 files (0 changed)`（事前已格式化 3 文件）。
- `flutter analyze` → `No issues found!`。
- `flutter test`（逐文件独立进程；fix16c 首跑 1 次 engine flake，重跑绿）：
  - `test/repair/wave_b_rollback_131_135_test.dart` → 11/11 pass。
  - `test/fix16c_clash_ui_config_test.dart` → 8/8 pass（retry 1）。
  - `test/t15a_proxies_test.dart` → 3/3 pass。
  - `test/repair/sp_20_connections_full_test.dart` → 10/10 pass。
  - `test/fix11b_clash_panel_test.dart` + `test/r4_13_s02_contract_test.dart` → 6/6 pass。
  - `test/fix08_option_error_test.dart` → 1/1；`fix08_option_apply/cancel` → 2/2 pass。
- 未运行：cargo 全套、`flutter build windows --release`、整套 flutter test。

## Per-id 结果

| id | 项 | 结果 |
|---|---|---|
| FLD-CFG-131 | ProxiesSorting | pass：失败→“排序: 延迟”保持 + 可见报错 + 延迟序不变；成功→名称序 + 重开保持 |
| FLD-CFG-132 | ProxiesAutoRefresh | pass：失败→开关保持关 + 可见报错 + 1s 零新增轮询；成功→开 + 1s 轮询 + 重开保持开位与轮询 |
| FLD-CFG-133 | ProxiesRefreshInterval | pass：`clashPollPeriod` 开+5s/0/负/关四象限 + 文档 5s 解析 + 开&0 间隔零轮询 |
| FLD-CFG-134 | ConnectionsAutoRefresh | pass：失败→开关保持关 + 可见报错 + 1s 零新增轮询；成功→开 + 1s 轮询 |
| FLD-CFG-135 | ConnectionsRefreshInterval | pass：文档 5s 解析 + 非正间隔停轮询 + 开&0 间隔零轮询 |

## 未完成/缺口

- 真实 core API 轮询联动（请求频率观测、隐藏暂停/取消、core 切换）与正式
  代理/连接页→FRB→保存→重开 E2E 未跑，仍归最终门禁 SP-34。
- 设置窗 host 路径（独立窗口）失败时 draft 保留用户值 + 可见错误，重开
  reseed 回 canonical；in-process 路径已回滚。非法值（坏类型/负间隔）在
  解析层回退默认值（既有 `asInt/asBool` 语义，未动）。
- 工作树另有并行 workstream 未提交改动（profiles、wave_b_g15、SP-24 证据），
  本卡未触碰、未 commit。
