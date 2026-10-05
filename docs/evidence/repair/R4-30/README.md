# R4-30 完整语言和错误反馈 — 证据

状态：implemented（synthetic/合成 widget + 单元验证通过；真实 DPI/RTL 截图与子引擎语言绑定未验证，见 gap）。
HEAD：`db3371a06ad8b6d0b45b4be82e9d815179323fdd`；冻结原版 v2rayN 7.25.4 / `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
armed=false；未触碰 10808、宿主代理/注册表/路由/TUN/自启、用户凭据；未启动内核或监听端口；只用合成数据与内存桥。

## 本次唯一用户流程

切原版语言后主子窗口、菜单、状态栏与错误反馈均切换，重开保持；缺翻译回退英文，错误无裸 key。

## 方案定位（§2 D24/D36、§3）

- D24：`locale_config.dart` 只设 Material locale；菜单/表单硬编码中文，缺完整资源消费。
- D36：字号/主题/语言在表格或原生子窗口不一致；子 engine 固定 light/default font/locale。
- §3：先把真实用户操作合同冻结，逐流程重验，语言属 J18 界面与平台。

## 关键设计

1. 资源表 `lib/shared/l10n/strings.g.dart`：从冻结 `ResUI*.resx` 提取，键名保持上游（`menuServers`、`TbSettingsCore`…），每键提供 9 语言值，`en` 为回退。未提取的窗口正文串登记于 `untranslated-inventory.md`。
2. `lib/shared/l10n/l10n.dart`：`L10n` 查表 + `productLanguageProvider`（由 `app.dart` 从 `UiShellState.language` 绑定）。`L10n.of()` 优先读 provider，无 scope 的裸 widget harness 回退上游默认 `zh-Hans`。缺翻译→英文→原 key（不崩、不空）。
3. `lib/shared/l10n/error_localizer.dart`：`error.*`、`settings.saved*`、`validate.*`、runtime/platform 投影 → 可读“原因+行动”；裸 `namespace.key` token 降级为通用文案，已是人话的消息原样透传，绝不显示裸 key。
4. 消费点：`main_menu.dart`（每条目带 `l10nKey`+`labelFor`）、`main_shell.dart`（菜单/工具栏/状态消息）、`status_bar_view.dart`（含 runtime/platform 错误）、`option_setting_window.dart`、`theme_setting_dialog.dart`、`global_hotkey_window.dart`、`routing_windows.dart`、`dns_window.dart`。
5. 语言切换生效：主题窗口改 `CurrentLanguage` → `applyThemeSelection` → `productLanguageProvider` → 全部消费点重建；theme/hotkey 弹窗监听语言变化后关闭，用户重开即新语言（不做静默半窗换字）。

## 复现（先失败后通过）

- `test/repair/r4_30_repro_test.dart`：断言产品文字走 ResUI 键、缺翻译回退、错误 key 本地化、菜单 key 完整、状态栏不显示裸 key。
- pre-fix（stash 掉本次 lib 改动、移走 `lib/shared/l10n/`）运行：exit 1，见 `repro-prefix.log`（`l10nKey`/`tr` 等基础设施不存在，旧实现无产品资源与错误本地化）。
- post-fix：`test/repair/r4_30_repro_test.dart` 6/6 通过；`test/r4_30_contract_test.dart` 14/14 通过；合并运行 20/20。

## 命令结果

| 命令 | 结果 |
|---|---|
| `flutter analyze` | exit 1；唯一告警是别的卡文件 `test/repair/r4_18_p09_repro_test.dart` 未用 import，与本卡无关 |
| `flutter test test/r4_30_contract_test.dart` | exit 0，14/14 |
| `flutter test test/repair/r4_30_repro_test.dart` | exit 0，6/6 |
| `flutter test test/r4_30_contract_test.dart test/repair/r4_30_repro_test.dart` | exit 0，20/20 |
| `flutter build windows --release` | exit 0，产出 `build\windows\x64\runner\Release\v2rayn_desktop.exe` |
| `flutter test`（全量） | 768 通过、2 pending；全部失败均为既有批跑 `did not complete` 抖动（49 条），无任何真实断言失败；受影响文件单跑各自通过 |

日志：`test-r4_30.log`、`analyze.log`、`build-windows-release.log`、`repro-prefix.log`、`full-suite.log`。

## 回归覆盖（单跑通过）

- 菜单结构与分隔：`t10_menu_test`、`t11_menu_test`、`t12a_menu_test`、`t17_menu_structure_test`、`t17_disabled_entry_test`、`r3_wpf_main_chrome_test`（17/17）。
- 设置/路由/DNS/热键/主题：`r3_wpf_option_window_test`、`r3_wpf_routing_window_test`、`r3_wpf_routing_structure_test`、`fix08_*`、`fix15_hotkey_test`、`t12a_theme_test`、`t12a_hotkey_test`、`sr_*`、`t13_statusbar_test`、`fix16_settings_field_test`、`r4_11_contract_test` 单跑通过。

## 未提取字符串清单

见 `untranslated-inventory.md`：窗口正文的逐字段标签本轮未全部迁入资源表（按卡允许“分批提取”，已登记不静默遗漏）。菜单/主窗口/状态栏/设置窗口外壳/路由与 DNS 顶层/错误反馈为最低集已覆盖。

## 接口缺口

见 `observations.json.gaps`：子引擎 presentation/locale 绑定（GAP-01，属 UFS-13/D36）、跨 engine outcome 消息内嵌 key（GAP-02）、`AppMenuEntry.label` 语义（GAP-03）。

## 清理

未启动任何本项目进程；无端口、无内核、无 OS 副作用，无需清理。仅停止过本代理自身持有的 flutter 构建进程（正常退出）。
