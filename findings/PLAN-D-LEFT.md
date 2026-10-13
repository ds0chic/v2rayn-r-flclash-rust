# PLAN-D-LEFT — Dart 侧遗留项实施计划（PLAN-FIRST，未获批前不动产品代码）

- 基线：`audit/2026-10-13 @ d42f598`（merge: U-FIX3），本地 worktree `worktrees\D-LEFT`，分支 `fix/D-LEFT`。
- 状态：本计划已写；产品代码零改动（`git status` 干净，仅本文件未提交）。
- 范围：D3a-11、D1-09、3 个 UX leftover、UI-DEVIATIONS sweep；PHASE 0 发布构建（计划写完后并行执行，见 §6）。
- 日期：2026-10-14。

## 0. 已确认的 HEAD 现状（读码结论，非记忆）

| 项 | HEAD 状态 | 证据 |
|---|---|---|
| D1-09 路径守卫 | 已部分修（fail-safe 拒绝），`!` 未覆盖 | `custom_editor_dialog.dart:60-74`（`isSafeExternalOpenPath` 正则无 `!`），提交 `59ebc9b`；测试 `test/b13_d109_open_path_test.dart` |
| D1-09 启动方式 | 仍走 `cmd /c start`，两处 | `custom_editor_dialog.dart:43-48`（文件）、`settings_actions.dart:285-294`（URL）；无其它 `runInShell:true`/`'cmd'` |
| D3a-11 同步解码 | 未修，注释自认同步 | `scan_image_qr.dart:44-48`；调用点 `scan_screen_qr.dart:76-79` |
| http 警告 | 已修为瞬态 SnackBar | `sub_edit_window.dart:160-167`，提交 `f92d7bc`；`isPrivateNetworkHost` 对等 `Utils.IsPrivateNetwork`（回环/RFC1918/链路本地） |
| recovery 文案 | 未修：Dart 无映射；另有两处裸 key 直达 UI（见 §3b-附） | Rust `crates/application/src/engine.rs:2861` 发 `error.recovery_required`；`error_localizer.dart:87-123` 的 `_errorKeyMap`（35 条）无该 key → 本地化路径落到通用文案；`sub_setting_window.dart:494-496`＋`backup_and_restore_view.dart:472-474` 另逐字显示裸 key |
| rt- id 分叉 | 未修 | 嵌入式 `routing_windows.dart:909` 用 `id: ''`，独立窗 `:3298` 用 `rt-<微秒>` |
| l10n 机制 | 手工 flat map，无代码生成步骤 | `strings.g.dart:1-13` 头注：纯数据表，加 key 只需编辑 map，无 regen |

## 1. D3a-11：全屏 QR 解码移出 UI isolate

- HEAD 现状：`decodeQrImage(Uint8List)` 为同步纯函数（解码＋逐像素 RGBA 拷贝＋两次 zxing）；屏幕扫描路径对整屏数 MB PNG 全程跑 UI 线程（`scan_screen_qr.dart:76-79` 在 hide→capture→decode→show 之间）。
- 设计：
  1. 保留 `decodeQrImage` 同步体不动（小图路径与现有单测继续直调它）。
  2. 新增顶层入口 `String? _decodeQrImageEntry(Uint8List bytes) => decodeQrImage(bytes);`（`compute` 要求顶层/静态函数；`Uint8List` 可过 isolate 端口）。
  3. 新增 `typedef QrDecoder = Future<String?> Function(Uint8List bytes);` ＋ 默认实现 `decodeQrImageAsync`（经 `compute(_decodeQrImageEntry, bytes)`）。
  4. `scanScreenQr` 与 `scanImageQr` 各加可选命名参数 `{QrDecoder decoder = decodeQrImageAsync}`，分别替换 `:78` 与 `scan_image_qr.dart:106` 的直调。屏幕路径默认走 isolate；图片路径（用户小图）默认同样走 isolate（统一语义，开销一次 isolate 约数十 ms，可接受；若实测回归则图片路径保留同步并在计划评审时说明）。
  5. 不做解码失败的单独 toast：isolate 抛错沿现有 `try` 进统一失败分支（屏幕路径报"屏幕截图失败"稍有偏差，接受；不值得加新文案）。
- 不 hang 测试的缝：
  - 所有既有 `scan_screen_qr_test.dart` / `scan_image_qr_test.dart` / `ux_parity_fix05_scan_test.dart` 用例改为注入 `decoder: (_) async => <canned>`（成功/空/null 三档），全程无真实解码、无 isolate、无 `runAsync` 计时依赖。
  - 另加一个纯 `test()`（非 widget）：真实 `decodeQrImageAsync` 解 ≤ 200px 合成 QR，断言与同步结果一致，证明默认缝是通的（小图，isolate 开销可控；设 `timeout` 60s，失败即 flaky 候选、 solo 裁决）。
- 红→绿：先把新测试写成注入 fake 前断言"默认 decoder 在测试环境被调用"失败？不——`compute` 在 tester 里能跑，红绿不能靠它。红→绿锚点改为：(a) 新增单测断言 `scanScreenQr`/`scanImageQr` 接受 `decoder` 参数且 fake 的 canned 文本进入 `importShareText`（修复前签名无该参数，编译失败＝红）；(b) 静态断言 `scan_screen_qr.dart` 不再直调 `decodeQrImage`（grep 零命中）。
- 验收：`dart format` 0、`flutter analyze` 干净；三个扫描测试文件串行全绿；全量回归无新增红（exit 79 按 solo 裁决）。
- 风险：`compute` 在 release 下的首次 spawn 抖动只影响单次扫描延迟，不影响正确性；低端机收益即目标。

## 2. D1-09：`!` 路径与 `cmd /c start` 启动器替换

- HEAD 现状：`59ebc9b` 的守卫拒绝 `"&|<>^%`$;`＋控制字符， fail-safe；但 `!` 放行。`Process.run('cmd', ['/c','start','',quoted])` 下：cmd 默认关闭延迟展开时 `!` 无害，但用户/企业环境可经注册表/`/V:ON` 开启，且 `start` 对首个引号参数有标题语义（已用 `""` 占位）。结论：守卫正确但机制仍是 shell。
- 设计（无新依赖；pubspec 无 `url_launcher`/`open_file`，不引入）：
  1. 新增共用 `buildOsOpenArgs(String target) => <String>[target]` ＋ `osOpenProgram == 'explorer'`（Windows）：`explorer.exe "<path|url>"` 经 CreateProcess 直接传参，无 cmd 重解析，`!&|^%` 全部字面，到达 ShellExecute 等价语义（文档/URL 均按关联打开）。`explorer` 路径含空格由 Dart 的 Windows 引号规则处理，无需手工包引号。
  2. `openConfigFileExternally` 改调 `Process.run('explorer', buildOsOpenArgs(fullPath))`；`openCoreWebsite` 改调 `launcher('explorer', buildOsOpenArgs(url))`（`DesktopLauncher` 缝已存在，`settings_actions.dart:208-211`）。
  3. 守卫保留做纵深，但 `!` 改为放行（无 shell 后 `!` 无特殊语义；`C:\dir\a!b.json` 类合法路径恢复可用）。其余拒绝集不动。
  4. 备选（若评审不接受 explorer 语义）：保留 `cmd /c start` 但把 `!` 加入拒绝集。代价：合法 `!` 路径被 fail-safe 拒绝。本计划主推 explorer 方案，备选一行改动。
- `openCoreWebsite` 是否同洞：是同一机制，但其 URL 来自内部 `coreRepoSlugs` 常量（`settings_actions.dart:151-163`），无用户输入，不可注入；顺手同换以收敛机制，测试 `r3_root_03_actions_test.dart:111-127` 同步更新（`buildOpenUrlArgs` 改名/改期望）。
- 红→绿：
  - `b13_d109_open_path_test.dart` 加例：`isSafeExternalOpenPath('C:\\d\\a!b.json') == true`（修复前 false＝红）；保留全部现有拒绝例（防倒退）。
  - 新增 `d_left_launcher_test.dart`：fake `DesktopLauncher`/`CustomFileOpener` 录制 `program == 'explorer'` 且 args 无 `'cmd'`/`'/c'`/`'start'`（修复前红）；全程无真实启动（既有 recording 模式可仿）。
  - `r3_root_03_actions_test.dart` 期望更新为 explorer（随改）。
- 验收：format 0、analyze 干净；三文件串行绿；`grep -rn "'cmd'" lib/` 在产品代码零命中（测试期望除外）。
- 越界声明：`relaunchAsAdmin` 的 PowerShell `Start-Process -Verb runas`（提权重启本 exe）不在本项范围，仅检查不改。

## 3. UX leftover 三件

### 3a. http 订阅警告 → 持久样式（OPEN：瞬态已修，持久未做）

- HEAD：`f92d7bc` 在 `_save` 里对非私网 `http://` 弹一次 SnackBar，关对话框即消失；用户点保存时若没看清，下次打开仍无任何提示。
- 设计：`_SubEditWindowState` 加 `bool get _showHttpWarning`（由 `_url.text` 实时计算，复用 `isPrivateNetworkHost`＋`Uri.tryParse` 现有逻辑）；在 Url 输入行下方常显一条 warning 色 inline banner（key 如 `sub-http-warn`，文案沿用"该订阅使用 http 明文传输，请注意安全"），非阻塞、可保存（与上游 `InsecureUrlProtocol` 非阻塞语义一致）。`_save` 内 SnackBar 保留（双通道：编辑时常显＋保存时提醒）。
- 红→绿：widget 测试 pump 含 `http://8.8.8.8/x` 的 `SubEditWindow`，断言 `find.byKey('sub-http-warn')` 命中（修复前红）；再测 `http://192.168.1.1/x` 与空 URL 无 banner；保存仍成功 pop draft。
- 验收：format/analyze；`sub_edit*` 相关测试串行绿；视觉：banner 用 warning 容器色，不引入新 token。

### 3b. `error.recovery_required` → 用户可读中文（OPEN；Rust 侧无改动点，纯 Dart 映射）

- HEAD：Rust 在持久化恢复未完成时以 `error.recovery_required` 拒绝写入（`engine.rs:2861,2944`）；Dart `_errorKeyMap`（`error_localizer.dart:87-123`，35 条）无此 key。经本地化路径（状态条 `errorKeyText`→`ErrorLocalizer.error/key`）用户看到通用"操作失败"，不知道等一下重试即可。
- 设计：在 `strings.g.dart` 加 `recoveryRequired` 资源行（`zh-Hans`＝"恢复中，请稍后重试"，`en`＝同义英文；其余 locale 沿表内惯例，无上游原文则与 en 一致），`_errorKeyMap` 加 `'error.recovery_required': 'recoveryRequired'`。`error()` 走 `_byMessageKey` 即命中，无需改逻辑。key 名冻结不动（contract），只加映射。
- 红→绿：`ErrorLocalizer.key('error.recovery_required')` 断言含"恢复中，请稍后重试"（修复前得 `OperationFailed` 通用文案＝红）；`error()` 同断。
- 验收：format/analyze；localizer 单测绿；Dart 侧 grep `recovery_required` 仅 localizer＋测试。

### 3b-附：其它直达 UI 的裸 messageKey 普查（只映射真到 UI 的 key，不改 key 名）

普查结论（读码，非记忆）：本地化路径（`errorKeyText`/`ErrorLocalizer.error/key`）永不显示裸 key（`fallbackFor` 兜底通用文案）；**逐字显示裸 key 的是两处"人工消息＋技术 detail"槽**：

| # | 位置 | 现状 | 到 UI 的形态 |
|---|---|---|---|
| 1 | `sub_setting_window.dart:494-496` 渲染 `'${message} — ${detail}'` | `detail` 来自 `subs_controller.dart:298`（存盘失败直接放 `result.error?.messageKey`）与 `:392` 经 `_firstError`（`:664` 放 `'备注: <messageKey>'`，`:671` 放 `'CODE: <messageKey>'`） | 订阅窗失败行直接显示 `error.url_invalid` 等裸 key |
| 2 | `backup_and_restore_view.dart:472-474` 同样模板 | `detail` 来自 `backup_controller.dart:92-96/146` 的 `'CODE / error.key[: detail]'` | 备份页 technical detail 栏（人工 message＋技术 key，有意诊断） |
| 3 | `subs_controller.dart:380` | bridge 抛错时把 Dart 异常 `toString()` 塞进 `messageKey`，再经 `_firstError` 显示 `'E_UNAVAILABLE: <异常文本>'` | 订阅窗（异常文本原文，非 key，但同样绕过本地化；且本地化路径遇到它会因 `retryable:true` 显示"后端不可达"，两条路径文案分叉） |

- key 缺口（Dart/Rust 产出 `error.*` 但 `_errorKeyMap` 无映射；map 内 35 条其余已覆盖 `bridge_port.dart` 全部产出 key）：`error.recovery_required`（本项主目标）、`error.sub_update_failed`（`subs_controller:456,521` 的默认 key，真到订阅窗）、`error.storage`（`bridge_port.dart:2135` 导入路径产出；是否到 UI 待实施时沿 `SubStatus` 链确认，contingent）。
- 设计（与 3b 主目标同批，三个小 commit）：
  1. 加映射：`error.sub_update_failed` → 复用/新增"订阅更新失败"资源行；`error.storage`（若确认到 UI）→"存储失败"行。key 名一律不动。
  2. 订阅窗 detail 走本地化：`_firstError` 与 `:298` 的 `messageKey` 在入 `SubStatus.detail` 前过 `ErrorLocalizer.key(...)`（view 层已有 `context`/`l10n`，`sub_setting_window` 可直接调；controller 保持只传 key，不动语义）。裸 key 从此只出现在日志/诊断，不再面向用户。
  3. 备份页 `detail` 保持原样（人工 message＋技术 key 是有意的诊断格式，不动；已有映射的 key 在其本地化路径自动生效）。
  4. `:380` 异常文本不再冒充 `messageKey`：改为 `messageKey: 'error.bridge_unavailable'`（已有映射）＋异常文本进 `detail`（诊断保留）。消除两条路径文案分叉。
- 红→绿：`key('error.sub_update_failed')` 命中人类文案（修复前通用＝红）；widget/单测断言订阅窗失败 `detail` 不含 `error.` 裸 key（修复前含＝红）；`:380` 场景断言 `messageKey` 为契约 key 而非异常文本。
- 验收：format/analyze；subs/backup 相关测试串行绿；`error.*` 产出 key 清单（bridge_port＋subs_controller＋Rust engine 已知三处）与 `_errorKeyMap` 对账表记入 FIXREPORT。

### 3c. 独立路由窗 `rt-` id vs 嵌入式空 id（OPEN，D3a-08）

- HEAD：嵌入式 `_save`（`:909`）新建 `id: ''` 靠后端分配；独立窗（`:3298`）自造 `rt-<微秒>`。另 `newRuleId()` 微秒＋自增在双引擎（两 isolate 各自 `_ruleSeq` 从 0）同微秒可碰撞，且稳定 id 写入 RuleSet 影响按 id 编辑/选中。
- 设计：独立窗 `_save` 改为 `id: existing?.id ?? ''`（与嵌入式/导入 `:1711` 一致，后端分配为唯一真相）；`newRuleId` 加随机段（如 `Random().nextInt(1<<32)` 后缀）消除跨 isolate 碰撞。先确认 Rust `save_routing` 对空 id 的分配语义（D3a 跨切片 C3 的遗留确认：读 `crates/*/routing*` 存档逻辑，只读 Rust，不改 Rust；若 Rust 对空 id 不分配，则本项降级为"两处统一用 `rt-`＋随机段"，计划评审时定）。
- 红→绿：单测断言独立窗 `_save` 新建 profile id 为 `''`（或统一后的形态；修复前 `rt-`＝红）；10k 个 `newRuleId()` 无重复且含随机段。
- 验收：format/analyze；routing 相关测试串行绿；Rust 侧零改动。

## 4. UI-DEVIATIONS 未完成/待核实 sweep（8 用户决策项不动）

-  sweep 方法（低负载、只读）：逐行核 `FIXREPORT-U-A/B1/B2/C1/C2` 原文＋HEAD 代码/test，结论只记三类：已核实（翻备注为"已核实＋证据"）、便宜安全可补（进实施清单）、不动（用户决策 8 项＋需后端/新动作面）。
- 8 用户决策项（§待用户决策项 (1)–(8)）：全部 untouched，不评估、不改。
- 便宜安全可补（docs-only 为主，零产品风险）：
  1. 约 30 行"以报告为准/待核实"（主题选项回退、字号、间距、折叠、徽标、确认框、托盘 hint、设置 help/分组/三态、DNS/路由文案行等）：逐项 grep 代码＋跑对应单测，属实则把备注翻为"已核实（证据：文件:行/测试名）"。纯文档改动，可与其它 commit 同批。
  2. UI-SUBEDIT-FOLD 等"默认分级、展开即用"类：同上验证即可关闭。
- 明确不补（not-worth-it / 不安全）：
  1. UI-SETTINGS-SEARCH 行级隐藏：FIXREPORT-U-B2 §4-9 已注 fix16 tap 风险，不碰。
  2. 待决策 (3)(4) 撤销/回滚/恢复门：需 Rust 快照 API，保持禁用＋说明。
  3. 待决策 (5) 托盘"停止"/"测速当前节点"：dropped，做即新动作面，另案。
  4. 待决策 (6) 智能默认：需订阅切片定规则，本批不做。
- 验收：UI-DEVIATIONS.md 只增行（新"核实"列或备注后缀），`compat/` 不动；diff 仅本文件。

## 5. 实施纪律（获批后）

- test-first 小步 commit（一项一 commit＋双 trailer）；`dart format` 0、串行 `flutter test -j 1`（持 flutter.lock，scan 仍跳过直跑、走 fake 缝）；analyze 全干净；触及＋引用测试串行跑，exit 79 按 solo 裁决；行为/UX 取舍不动并上报；不碰 Rust/生成物（strings.g.dart 为手工表，可编辑）；不 push。

## 6. PHASE 0（计划写完后并行）：干净 HEAD 发布构建

- 在干净 `d42f598` worktree 持 flutter.lock 跑 `flutter build windows --release`，记录 exit/耗时/产物（exe＋dll＋app.so＋flutter_assets），门禁位供 U-FIX3；随后删 `build/`。结果记入 FIXREPORT-D-LEFT（或本文件附录），不记入产品 diff。
