# R3-ROOT-03 — preservedOnly 菜单入口：管理员重启 / UWP 回环 / 区域预置 / 核心网站

状态：`implemented`（命令构造与 stub 测试通过；菜单入口已去掉 `preservedOnly`；`main_shell.dart` 属禁止修改文件，接线补丁见下，由根代理落地。真实提权重启 / 回环系统写入 / 浏览器打开未在本轮执行，故不写 `verified`）。

任务 ID：R3-ROOT-03

本次唯一用户流程：主菜单「设置」下的「以管理员身份重启」「解除 Win10 UWP 应用回环代理限制」「区域预置设置（默认/俄罗斯/伊朗）」以及「帮助/核心网站」四个上游入口，不再是 `preservedOnly` 禁用占位；点击后按上游语义构造并执行对应动作，取消/失败有明确提示。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `cd141f5`。上游语义来源：`ProcUtils.cs:7-68`（`ProcessStart`/`RebootAsAdmin`）、`Global.cs:88`（`RebootAs = "rebootas"`）、`MainWindow.xaml.cs:251-253`（UWP `EnableLoopback.exe`）、`MainWindowViewModel.cs:220-250`（`RebootAsAdminCmd`/`RegionalPreset*Cmd`）、`ConfigHandler.cs:2894-2957`（`ApplyRegionalPreset`）、`CoreInfoManager.cs:97-303` + `Global.cs:649-665`（核心网站 URL）、`MainWindow.xaml.cs:431-453`（`AddHelpMenuItem`/`MenuItem_Click`）。别名证据见 `docs/evidence/parity-recheck-2026-10-04/round3-root.md` 的 R3-03 段。

对应 feature / field / action / layout ID：`ACT-MAIN-029`、`ACT-WIN-004`、`ACT-MAIN-032/033/034`、`ACT-WIN-008`、`F-DESKTOP-005/006`。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 下 `ServiceLib/Common/ProcUtils.cs`、`ServiceLib/Global.cs`、`ServiceLib/ViewModels/MainWindowViewModel.cs`、`ServiceLib/Handler/ConfigHandler.cs`、`ServiceLib/Manager/CoreInfoManager.cs`、`v2rayN/Views/MainWindow.xaml.cs`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 管理员重启（029）：输入=当前 exe 路径；输出=`rebootas` 参数经 `runas` 提权重启；错误/取消=UAC 取消或启动失败返回 false 并提示，不伪装成功；权限=用户显式点击、Windows `runas`；无持久化。
- UWP 回环（004）：输入=是否找到 `bin/EnableLoopback.exe`；输出=上游工具路径，或等效 `CheckNetIsolation LoopbackExempt -a/-d -n=<PackageFamilyName>`（可逆提示）；本轮不执行系统写入，测试用记录型 stub。
- 区域预置（032/033/034）：输入=Default/Russia/Iran；输出=对应路由/Geo/SRS 源与内置 DNS 预设的写入（离线，不联网）；合成测试断言内容与写入。
- 核心网站（008）：输入=核心名；输出=上游 `Global.CoreUrls` 去 `/releases` 后的主页 URL，经 `cmd /c start` 交系统浏览器；应用自身不发网络请求。
- 持久化：区域预置经既有 `settings`/DNS 保存链落库；其余无。

允许修改的模块：`apps/desktop/lib/app/menu/main_menu.dart`（去 `preservedOnly`）、`apps/desktop/lib/features/settings/settings_actions.dart`（新增动作函数/命令构造）、`apps/desktop/test/**`、本卡、`docs/evidence/recheck-fixes/R3-ROOT-03-R3-04/**`、`compat/actions.yaml`（追加 notes / 更新状态）。

禁止改变的已有行为：`main_shell.dart`（只给补丁）、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、profiles/subs/runtime/monitor/update/backup/routing 各 feature；菜单结构/条目/顺序/分隔不动，不删入口、不降分母。

测试夹具和原版预期：纯命令构造断言（exe/参数/verb、PowerShell argv、CheckNetIsolation argv、URL/`cmd start` argv、区域预置消息）；launcher 用记录型 stub，不启动任何宿主进程。原版预期：`rebootas` + `runas`；`EnableLoopback.exe` 或 `CheckNetIsolation ... -n=<name>`；核心主页 URL 与 `Global.CoreUrls` 一致。

本次必须通过的命令/真实场景：
- `dart format`（改动文件）
- `flutter analyze`
- `flutter test test/r3_root_03_actions_test.dart -r expanded`
- `flutter test test/t17_menu_structure_test.dart -r expanded`
- `flutter test test/t17_disabled_entry_test.dart -r expanded`
- 真实窗口点击验证（提权/UWP/浏览器打开）未执行，需用户授权隔离实测。

证据文件位置：`docs/evidence/recheck-fixes/R3-ROOT-03-R3-04/`。

完成条件：四个入口不再 `preservedOnly`；命令构造与 stub 测试覆盖重启/UWP/区域预置/网站；区域预置内容与写入有断言；`main_shell.dart` 接线补丁给出。未做真实平台效果实测，保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：`main_shell.dart` 为禁止修改文件，本轮只给补丁；未落地前点击新入口会走 `default` 分支的 `notImplemented` 提示（诚实，不伪装成功）。
- 接口缺口（登记）：上游「核心网站」是按核心动态展开多条目；当前 Dart 模型是单一入口，打开默认 Xray 主页。完全展开需动态菜单模型，不在本卡。
- 接口缺口（登记）：bridge 的 Russia/Iran `apply_regional_preset` 会尝试下载远程模板（`crates/bridge_api/src/api/dns.rs`，本轮不可改）；离线菜单路径应由应用层 `engine.apply_regional_preset`（已离线写源+内置 DNS）暴露到 FRB，需改 `frb_generated`，越界。

## main_shell.dart 接线补丁（由根代理落地，禁止本子代理直接改）

在 `apps/desktop/lib/app/shell/main_shell.dart` 的 `_onMenuAction` `switch` 中：

```dart
      case 'ACT-MAIN-029':
        // F-DESKTOP-006 / ACT-MAIN-029: elevated self-restart (`rebootas`).
        final relaunched = await relaunchAsAdmin();
        shell.setMessage(relaunched
            ? '已请求以管理员身份重启'
            : '以管理员身份重启已取消或失败');
      case 'ACT-WIN-004':
        // F-DESKTOP-005 / ACT-WIN-004: resolve the bundled tool, else show the
        // reversible CheckNetIsolation command. Report-only this round.
        final hasTool = ref.read(platformBridgeProvider).resolveUwpLoopbackTool();
        final command = buildLoopbackExemptionCommand(
          bundledToolPath: hasTool ? 'EnableLoopback.exe' : null,
        );
        shell.setMessage('${command.summary}'
            '${hasTool ? '' : '（未找到 EnableLoopback.exe，命令：${command.program} ${command.arguments.join(' ')}）'}');
      case 'ACT-MAIN-032':
        shell.setMessage(applyRegionPreset(ref, 'Default'));
      case 'ACT-MAIN-033':
        shell.setMessage(applyRegionPreset(ref, 'Russia'));
      case 'ACT-MAIN-034':
        shell.setMessage(applyRegionPreset(ref, 'Iran'));
      case 'ACT-WIN-008':
        // F-DESKTOP-005 / ACT-WIN-008: open the core home page via the OS.
        openCoreWebsite();
        shell.setMessage('已在浏览器打开核心网站 (Xray)');
```

并在 `main_shell.dart` 顶部的 `settings_actions.dart` import 处保持现有 `import ... settings_actions.dart`；新增符号（`relaunchAsAdmin`/`buildLoopbackExemptionCommand`/`applyRegionPreset`/`openCoreWebsite`）均由本卡在 `settings_actions.dart` 提供。若开关函数为同步上下文，`relaunchAsAdmin` 的 `await` 需与相邻 case 保持一致。

本轮实际结果：`main_menu.dart` 去掉 6 处 `preservedOnly`；`settings_actions.dart` 新增 `buildAdminRelaunchRequest`/`buildElevationPowerShellArgs`/`relaunchAsAdmin`、`buildLoopbackExemptionCommand`、`coreWebsiteUrl`/`buildOpenUrlArgs`/`openCoreWebsite`、`applyRegionPreset`/`regionPresetMessage`。命令与结果见证据目录。未运行真实提权/UWP 写入/浏览器打开。
