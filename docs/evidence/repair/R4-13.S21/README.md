# R4-13.S21 evidence (SystemProxyItem)

- 状态：implemented（存储链 + applied 对账）；真实 WinINET 写读 blocked。
- 字段：FLD-CFG-137..142（SysProxyType、SystemProxyExceptions、NotProxyLocalAddress、SystemProxyAdvancedProtocol、CustomSystemProxyPacPath、CustomSystemProxyScriptPath）。
- 缺陷（现版失败断言）：`PlatformController` 只有 runtime-session 监听（R4-24），设置保存改了 `SystemProxyItem` 但在同一 applied session 下不重新对账系统代理。上游 `OptionSettingViewModel.SaveSettingAsync` → `Reload` → `LoadCore` → `UpdateSysProxy`。
  - 复现修复前：`Expected: contains SysProxyMode.forcedChange / Actual: [SysProxyMode.forcedClear]`。
- 最小修复：`apps/desktop/lib/features/settings/platform_controller.dart` 新增 `settingsControllerProvider` 监听：`SystemProxyItem` 变化且 runtime 有 applied endpoint 时调用 `syncAppliedMode()`；同 session 去重仍由 R4-24 保证。
- Dart 合同：`test/r4_13_s21_contract_test.dart`（5 例，pass：四模式保存→重开 + 缺省）。
- 复现：`test/repair/r4_13_s21_repro_test.dart`（修复前 fail、修复后 pass）。
- blocked：真实 WinINET/注册表读写、授权隔离机四模式/所有权恢复未实测（无授权隔离 VM；硬约束禁止写宿主系统代理）。
- 命令见 `observations.json`。
