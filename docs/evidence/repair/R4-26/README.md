# R4-26 托盘热键自启动作 — 证据

- 任务卡：`docs/repair/tasks/R4-26.md`
- 应用基线：`77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`
- 冻结原版：v2rayN 7.25.4 / `7d6a967c18c697f28dc6917122ed3a4993fcf336`
- 本次工作树 HEAD：`7e9a4b49de8c287abc920b5a3eff1c1b2cb74a1b`（未提交，工作树已存在其它卡的无关改动，未触碰）
- 状态：`implemented`（真实 OS 注册/剪贴板/Run-key 未实测，见 blocked）
- armed=${armed}：本次未使用任何武装环境变量，未启动内核，未监听端口。

## 改动文件（仅本卡允许范围）

- `apps/desktop/lib/app/shell/tray_menu_model.dart`
  - 新增 `buildProxyCommandText` / `proxyCommandPort` / `resolveTrayProxyCommand`：
    按上游 `StatusBarViewModel.CopyProxyCmdToClipboard` 生成 Windows `set` / POSIX
    `export` 六行代理环境变量；端口取实际 applied session 端口，无会话则诚实失败。
  - 新增 `appliedSysProxyMode(PlatformView)`：从平台读模型事实（enabled/server/
    pacRunning/autoConfigUrl）推导“实际已应用”的模式。
  - `TrayReadModel` 增加可选 `iconMode`：图标按实际已应用模式，缺省回退 `desiredMode`
    以保持 R3-09 四状态映射与既有测试不回退。
- `apps/desktop/lib/app/shell/desktop_integration.dart`
  - `_onTrayAction` 补 `ACT-TRAY-012` 分支（D30：原缺 dispatch）。
  - 新增 `_copyProxyCommandToClipboard`：真实 `Clipboard.setData`，成功/失败写共享消息。
  - `_syncTray` 用 `appliedSysProxyMode(platform)` 作为 `iconMode`，radio 勾选仍用持久化
    `desiredMode`（上游 `SystemProxySelected`）。
- `apps/desktop/lib/features/settings/hotkeys.dart`
  - `registerAll` 在派发处加 `_paused` 门（对齐上游 `HotkeyManager.IsPause`）：编辑暂停期间
    或注销/重注册竞态中触发的组合不执行保存的动作。
- 测试新增：`apps/desktop/test/r4_26_contract_test.dart`、`apps/desktop/test/repair/r4_26_repro_test.dart`。

## 命令与结果

| 命令 | 结果 |
|---|---|
| `flutter analyze` | 0 issue |
| `dart format --output=none --set-exit-if-changed <5 files>` | 0 changed（已格式化） |
| `flutter test test/r4_26_contract_test.dart test/repair/r4_26_repro_test.dart --reporter expanded` | 20/20 通过 |
| 回归批跑：`r3_09_tray_icon_today`、`recheck_rr08_tray_sync`、`fix15_hotkey`、`t12a_hotkey`、`t13_hotkey`、`sr05_sr06_hotkey`、`fix15c_autostart_timing`、`fix15_tray_pac`、`r4_05_contract`、`r4_16_contract`、`t13_statusbar`、`t15a_statusbar` | 65/65 通过 |
| `flutter build windows --release` | 成功，`build\windows\x64\runner\Release\v2rayn_desktop.exe`（62.6s） |
| `flutter test`（全量） | `+813 ~2 -3`：3 条为已知批跑 `did not complete` 引擎抖动；5 个相关文件单独运行全部通过 |

命令日志：`cmd_repro_prefix.txt`、`cmd_new_tests.txt`、`cmd_regression.txt`、`cmd_build_release.txt`、`cmd_full_test.txt`。

## 断言复现（先失败→后通过）

- 复现文件：`apps/desktop/test/repair/r4_26_repro_test.dart`
  - 断言：热键编辑器 `beginEdit()` 暂停后，排队/竞态触发的组合不得执行保存动作。
  - 现版（临时还原 `hotkeys.dart` 派发门）：失败，`Expected: empty / Actual: [systemProxySet]`，
    日志见 `cmd_repro_prefix.txt`。
  - 修复后：通过。断言按上游 `IsPause` 语义编写，未按现错误实现改预期。
- 合同文件：`apps/desktop/test/r4_26_contract_test.dart` 覆盖 D30 复制命令六行文本、
  无会话诚实失败、端口以实际 applied 为准、图标按 applied 事实、tray 叶子 dispatch 映射、
  同组合多动作分组与一次注册全派发、暂停派发门、`unregisterAll`、隐藏/启动态不回退。

## blocked / 未验证

- 真实 Windows 全局热键 `RegisterHotKey` 占用/失败/派发：需授权隔离机，本次未实测（fake registrar/probe）。
- 真实 OS 剪贴板写入：仅用纯函数校验文本内容，未在宿主执行粘贴；未执行任何命令。
- 自启 Run-key 写入/失败反馈：未写宿主注册表（FIX-15C 语义仅回归测试，未实测写盘）。
- 真实系统代理/PAC/TUN 外部效果：未调用、未修改宿主；端口约束满足（测试无监听，端口均为 11808/11809 字符串，未占用）。
- DPI/多屏视觉：未实测。

## 下一步前置

- 在授权隔离机执行 15 托盘/9 热键逐动作真机回归与真实剪贴板/Run-key/热键占用验证后，方可从 `implemented` 提升 `verified`。
