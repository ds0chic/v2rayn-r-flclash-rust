# RE-PROF-03 证据 — Ctrl+C 分享导出 / Ctrl+F 分享窗口

状态：`implemented`。日期 2026-10-04；开始 HEAD `eeb2932`（工作树干净）；冻结上游 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

本回合只改 `apps/desktop/lib/features/profiles/{table_actions,profiles_table,profile_actions}.dart` 与 `apps/desktop/test/**`；未改 `main_shell.dart`、`features/subs/**`（仅调用 `shareProfilesQr`）、生成桥接、`crates/**`。测试用 `SyntheticBridgePort` 合成两节点 + 合成剪贴板 mock，无内核、无端口、无系统代理/注册表改动。

## 上游对照

- `ProfilesView.xaml.cs:220-246`：`Ctrl+C -> ViewModel.Export2ShareUrlAsync(false)`；`Ctrl+F -> ViewModel.ShareServerAsync()`。
- `ProfilesViewModel.cs:799-830`：`Export2ShareUrlAsync(false)` 用 `GetProfileItems(true)` 取 `SelectedProfiles` 全部；空集合时 `GetProfileItems` 返回 null，函数静默返回（`:443-449`）。
- `ProfilesViewModel.cs:593-608`：`ShareServerAsync` 用单一 `SelectedProfile.IndexId`；为空提示 `PleaseSelectServer`。
- 当前修复：`actionForKey(Ctrl+C)=export-share-url`、`(Ctrl+F)=share`；`profiles_table._onKey` 对两者 `handled` 并分别调用 `exportSelectedShareUrls`（写剪贴板、不克隆）与 `shareProfilesQr`（既有 QR 窗口）。

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `dart format --output=none --set-exit-if-changed <changed>` | exit 0，0 改变 |
| `flutter analyze lib/features/profiles test/...` | exit 0，No issues（5 items） |
| `flutter test test/table_actions_test.dart` | exit 0，6/6 |
| `flutter test test/profiles_share_shortcut_test.dart` | exit 0，1/1（单/多选 Ctrl+C + 行数不变 + Ctrl+F 窗口） |
| `flutter test test/profiles_share_shortcut_empty_test.dart` | exit 0，1/1（空选中 Ctrl+C 静默 / Ctrl+F 提示） |
| `flutter test test/profiles_keyboard_test.dart` | exit 0，1/1（首两次 exit 79 原生崩溃，重跑通过） |

未运行（任务约束）：`flutter build windows --release`、全仓 `flutter test`、真实 Windows 窗口集成测试。

## 结论与边界

- `Ctrl+C` 不再克隆：`SyntheticBridgePort.visible` 行数前后不变，剪贴板为 `vless://syn-syn-000000`；多选时同一次写入两条 URI。
- `Ctrl+F` 打开 `profile-share-qr` 对话框并显示所选节点；空选中提示 `请先选择节点`。
- 未做原版实机双窗口逐事件对照；多选 `Ctrl+F` 按“分享窗口单选”处理（上游按 primary），primary 语义缺口已登记在任务卡。
- 未追加 `compat/actions.yaml` notes（受本卡 15 文件读取预算限制）；建议根代理对 `ACT-PROF-024`/`ACT-PROF-006`/`ACT-PROF-004` 追加一行 notes。
