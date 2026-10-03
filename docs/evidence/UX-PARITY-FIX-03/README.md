# UX-PARITY-FIX-03 — 特殊节点按 configType 分派专用编辑器

状态：`implemented`（Custom 与 PolicyGroup 真实 Windows 窗口 + 真实 FRB/Rust/SQLite 验证；未做原版实机双窗口逐事件对照，故不写 `verified`）。

本目录只放本卡证据，未进 `dist/`，未碰 `work/` 与 `outputs/`，未 commit。

## 1. 问题

`editSelectedProfile` 无条件进通用编辑器；`editSelectedCustom`/`editSelectedGroup` 有定义但右键/Ctrl+D/双击三条入口都不分流。特殊节点一旦进入通用编辑器，会被 core/port/network clamp 改写文件/出站语义（PR-02）。组编辑器手工候选排除了全部复杂类型，原版手工选择仅排除 Custom，可嵌套组并由环检测兜底（PR-20）。Custom 文件浏览/外部编辑缺入口，透传原文执行链在 RT-08 跟踪（PR-07/RT-08）。

## 2. 修改

- `apps/desktop/lib/features/profiles/profile_actions.dart`：新增纯函数 `resolveEditorKind`（Custom/Outbound→custom，PolicyGroup/ProxyChain→group，其余→generic；对齐上游 `ProfilesViewModel.EditServerAsync:479/484` 与 `IsGroupType`），`editSelectedProfile` 按其分派（覆盖右键/Ctrl+D/双击三入口）；新增/编辑组统一带 `previewChildren`。
- `apps/desktop/lib/features/profiles/profiles_controller.dart`：新增 `groupChildPreview`（bridge `groupChildren` 的尽力封装，失败返回空预览，不破坏编辑器）。
- `apps/desktop/lib/features/profiles/group_editor_dialog.dart`：手工候选改为仅排除 Custom（注释引用上游 `AddChildAsync:SetConfigTypeFilter([EConfigType.Custom], exclude:true)`；订阅派生子项的严格规则留在 Rust `is_eligible_child`，不套用到手工 `ChildItems`）；单一下拉逐项加入改为多选节点选择器（全选/全不选，多选确定，按列表序追加并去重，对齐 `ProfilesSelectWindow.MultiSelect`）；新增“已落库组合预览”区（`groupChildren`：订阅匹配优先+手工保序，保存后刷新确认；草稿态预览是已存解析，属实标注）；五模式 LeastPing/Fallback/Random/RoundRobin/LeastLoad（0-4）保持与 `EMultipleLoad` 一致。
- 未改：`main_shell.dart`、FRB 生成文件、`crates/subscriptions/**`、通用编辑器、Rust（本卡无需 Rust 改动；未知顶层键按上游无列原则落库时丢弃，属有意 parity，见 §4）。

## 3. 证据

真实 Windows 窗口 + 真实 FRB/Rust/SQLite，隔离 data dir，合成 loopback 节点（127.0.0.1:11981/11982，未连接；预置端口 11820 ≥ 11808），未动 10808、未改宿主代理/TUN/注册表/路由，未启动内核：

| 文件 | 内容 |
|---|---|
| `observations.json` | 11/11 通过，`recordingComplete=true`，`failures=[]` |
| `fix03-run.log` | `flutter test integration_test/ux_parity_fix03_test.dart -d windows --reporter expanded` 通过日志 |
| `01-custom-editor.png` | Custom 右键编辑打开专用编辑器（非通用） |
| `02-custom-saved.png` | Custom 保存后落库 |
| `03-custom-reopen.png` | Custom 重开同编辑器同值 |
| `04-group-editor.png` | PolicyGroup 右键编辑打开组编辑器，子节点保序+T/U/D/B |
| `05-group-saved.png` | 组保存后落库 |
| `06-group-reopen.png` | 组重开同编辑器同子节点 |

关键断言（`observations.json`）：`custom-edit-opens-custom-editor`（genericEditor=0）、`custom-save-keeps-text-and-extras`（customConfigText/futureFlag/futureTransport/HeaderType 保留，preSocksPort=11820）、`custom-reopen-same-editor-and-values`、`custom-cancel-keeps-database`、`group-edit-opens-group-editor`（genericEditor=0）、`group-editor-shows-ordered-children`、`group-save-keeps-children-mode-filter`（childItems 保序/multipleLoad=3/filter/groupType）、`group-reopen-same-editor-and-children`、`group-cancel-keeps-database`。

## 4. 落库保留合同（实证结论）

- `proto_extra.extra` 未知键（如 `futureFlag`）与 `customConfigText`：落库保留（blob flatten）。
- `transport_extra.extra` 未知键：落库保留。
- 顶层 `extra`：仅 legacy 列（如 `HeaderType`）保留；无列的自由顶层键在 `row_from_profile` 被丢弃——与上游 `ProfileItem` 无对应列一致，属有意 parity，非本卡缺口。
- 引用完整性：`childItems` 保序、`subChildItems`、`filter`、`multipleLoad`、`groupType` 经保存/重开一致；悬空引用与环由 Rust `validate_group` 拒绝（已有单测）。

## 5. 门禁

```
dart format --output=none --set-exit-if-changed <6 个改动文件>   # 0 changed
flutter analyze                                                  # No issues found
flutter test test/fix03_special_editors_test.dart                # 7/7
flutter test test/t10_group_editor_test.dart                     # 6/6
flutter test test/t10_custom_editor_test.dart                    # 5/5
flutter test test/t10_menu_test.dart test/profiles_keyboard_test.dart test/profiles_pointer_test.dart test/t06a_editor_cancel_test.dart test/t06a_editor_save_test.dart  # 逐文件均绿（并发引擎抖动 did not complete 重试即过，见任务卡）
flutter test integration_test/ux_parity_fix03_test.dart -d windows  # 11/11（本目录日志+截图）
```

未跑 `flutter build windows --release`（根代理统一跑）与全仓 Rust 门禁（本卡无 Rust 改动）。

## 6. 未完成/后续卡

- FIX-03b（Outbound）：Custom/Outbound 共用对话框且分派已通；文件浏览导入（`BrowseServer`→配置目录拷贝→Address 改写）与外部编辑器打开需 file-picker 依赖 + bridge 导入 API（新 bridge 函数缺口，不改生成文件），待立卡。
- FIX-03c（Chain）：ProxyChain 分派/保存/重开与组共路已通；链式内核端到端（Xray/sing-box chain 出站组合）校验待立卡。
- RT-08 仍由运行时领域跟进：文件型 Address 的受控读取与原文透传（Mihomo YAML/Mixin）。
- 未做原版实机双窗口逐事件对照；macOS/Linux/ARM64/Avalonia 未验证。
