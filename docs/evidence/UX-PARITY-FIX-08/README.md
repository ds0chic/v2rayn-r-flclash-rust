# UX-PARITY-FIX-08 — 参数修改→应用（真实 Windows 窗口证据）

任务卡：`docs/tasks/FIX-08.md`（repair-queue 第 26 行，SET-06/07/08/09/10/11）。

## 运行方式（顺序执行，同一隔离 data dir）

```powershell
$env:V2RAYN_R_DATA_DIR='<isolated dir>'
$env:V2RAYN_R_FIX08_EVIDENCE='<repo>/docs/evidence/UX-PARITY-FIX-08'
$env:V2RAYN_R_OPEN_SETTINGS='1'
$env:V2RAYN_R_FIX08_MODE='cancel'  # 然后 apply，最后 reopen
flutter test integration_test/ux_parity_fix08_test.dart -d windows
```

用例：`apps/desktop/integration_test/ux_parity_fix08_test.dart`
（只用有界 pump；真实壳有常驻定时器，`pumpAndSettle` 永不静默）。
data dir 使用本次运行的 `C:\Users\Colby\AppData\Local\Temp\opencode\fix08-data`
（guiNDB.db 由引擎正常创建；未动用户真实配置目录）。

## 结果（2026-10-03 本机 Windows）

| 模式 | 断言 | 结果 |
|---|---|---|
| cancel | 改端口 10808→11821 后取消：窗口关闭、端口仍 10808、revision 不变 | 5/5 pass（`cancel-observations.json`） |
| apply | 改端口→应用：窗口关闭、端口 11822、revision 0→1；apply 返回结构化 `error.no_active_profile`（零节点，无崩溃、无伪造运行态） | 6/6 pass（`apply-observations.json`） |
| reopen | 新进程重开：端口仍 11822 | 2/2 pass（`reopen-observations.json`） |

截图：`cancel-edited/cancel-after/apply-edited/apply-after/reopen-window.png`
（`apply-edited.png` 可见端口框 11822 与 取消/应用/保存三按钮）。

## 约束遵守

- 未触碰 127.0.0.1:10808；端口值只做存储（11821/11822），无监听、无绑定。
- 未切换 AutoRun（无 Run 键读写）、未改系统代理/注册表/路由/TUN。
- 无订阅 URL/凭据；合成端口值仅本地断言。
- 已知环境噪声（非本卡缺陷）：`tray init failed: Bad Arguments`（测试资产条件，
  与发布包行为分开复验）、`runtime event gap` 调试行。

## 覆盖边界

本目录只证明第一唯一流程“参数修改→应用/取消→重开”。
DNS 导入/应用完整流与路由草稿余量分别见后续卡
`docs/tasks/FIX-08B-DNS-APPLY.md`、`docs/tasks/FIX-08C-ROUTING-DRAFT.md`。
