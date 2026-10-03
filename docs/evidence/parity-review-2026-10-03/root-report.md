# 主窗口、入口、布局与交叉复核

状态：`identified`。应用基线 `1251cbc6821276e35d082f7739b8b9c15b6f93dd`；冻结原版 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。根代理负责 62 行：30 动作、7 布局、3 主布局、7 窗口/资源、15 枚举。每行来源、实现、差异、证据和下一步见 `root-items.json`。

原版预期来自冻结 WPF/Avalonia 源码，第四轮六个场景运行了真正 Windows Flutter 窗口、FRB 和 Rust/SQLite。没有启动原版双窗口逐事件比较；原生电脑控制工具不可用。本报告不把自动组件驱动称为人工鼠标操作，不声称 800 项都完成运行验收。

## 当前 Windows 界面的直接证据

`integration_test/parity_review_smoke_test.dart` 使用新的 `V2RAYN_R_DATA_DIR` 和实际 FileUiStateStore，构建 V2rayNRApp，不替换业务 BridgePort。`AUTOSTART=0/AUTO_SMOKE=0`，不 apply 内核，没有监听、订阅下载或宿主代理/自启/TUN 写入。合成节点地址为 127.0.0.1:11998，认证标识是合成 UUID。

第四轮 `ui-run-04/observations.json` 的 `recordingComplete=true`，窗口流程全部走完，六个原版合同检查失败退出 1。策略组场景在真实持久化中先将合成节点归入有效分组，再从主窗口选中该组；记录中的 `currentGroupId=nodeGroupId`。因此最后一项满足冻结版生成前提。退出失败是产品差异断言，构建成功。

| 正常操作 | 原版源码合同 | 当前实测 | 关联问题 |
|---|---|---|---|
| 粘贴完整 Xray JSON 导入 | V2rayFmt.ResolveFull 接受完整配置，写为 Custom | 0 个节点；1 个保存失败 E_FIELD_REQUIRED；UI 还显示 1 行未识别 | ROOT-01 / PR-06 |
| 添加 TUIC | Username=UUID，Password=独立认证密码 | username 控件 0，password 控件 1，标签 UUID | PR-03 |
| 新增仅备注的普通分组 | URL 非空才做 URL 校验 | 保存被 URL 必填阻止，列表仍 0，编辑器保持打开 | SET-01 |
| 扫描屏幕二维码 | 截图识别再导入，恢复窗口 | 打开已有节点的分享二维码弹窗 | ROOT-02 / PR-14 |
| 扫描图片二维码 | 文件选择、解码、导入 | 打开粘贴文本窗口 | ROOT-02 / PR-14 |
| 节点右键→一键生成策略组→全部配置项 | 作用于原版定义的当前组，执行组生成 | 节点 1→1，无新策略组，提示“请先选择节点以确定订阅分组” | PR-19 / PR-21 |

完整配置夹具为 `{"log":{"loglevel":"warning"},"inbounds":[],"outbounds":[{"tag":"direct","protocol":"freedom","settings":{}}]}`。冻结 `V2rayFmt.cs:135-165` 的 protocol/settings/tag 三项满足 matchedCounter>=3，`ResolveFull:56-86` 将其认作 Custom。测试只导入，没有执行这个配置。

第一轮 `ui-run-01` 的 outbound 只有 protocol/tag，原版也会拒绝。其 `unsupported format` 观察不作为迁移差异；纠正夹具后的第二轮到达保存并复现 E_FIELD_REQUIRED。第一轮日志保留作审查纠错记录。第二轮虽六项观察都失败，但策略组没有明确选中有效组，原版也会拒绝那种前提；其结果不作为独立差异实证。第三轮在首场景后发生 Flutter Windows 引擎 0xc0000005 崩溃，`recordingComplete=false`，见 `ui-run-03/native-crash.log`；其部分观察不计入结果。第四轮完整通过有效分组前提，确认六项差异。

调试窗口还有 tray init `Bad Arguments` 和事件 seq gap 的日志。本轮不把这些直接归为发布包托盘故障：测试资产/调试启动条件与 RC 包不同，需单独真实发布包复验。没有因此断开本轮 UI 流程。

前四个场景截图在 `ui-run-02/01-complete-config-import.png`、`02-tuic-fields.png`、`03-empty-url-group.png`、`04-screen-scan-entry.png`；第四轮有效分组的策略组场景以 `ui-run-04/observations.json` 和当前源码为证，未拍图。`ui-run-02/05-group-command.png`只展示第一次不完整前提下的画面，不作为原版差异证明。未完成原版同 DPI 像素/字距比较。

## 根代理补充差异

| 编号 | 优先级 | 差异与影响 | 当前定位 / 原版依据 |
|---|---|---|---|
| ROOT-01 | P1 | 完整配置 UI 导入无法保存，inner wire 也不兼容；完整导入导出不是现有自发自收测试可证明的能力。 | subs_actions.dart / bridge subs.rs / subscriptions fmt；原版 V2rayFmt 与 InnerFmt。 |
| ROOT-02 | P1 | 两扫码菜单与 Ctrl+S 接错方向，分享已有节点不能替代扫码导入。 | main_shell.dart:158、328、330；原版 MainWindow.xaml.cs:110、226、265。 |
| ROOT-03 | P1 | F5、管理员重启、UWP、区域预设、动态核心网站等仍 disabled/占位；新增应用按钮不能代替原版 F5 入口。 | main_menu.dart / main_shell.dart:153、354、391；原版 MainWindowViewModel 与窗口命令。推广地址独立低优先处理，不能以关闭入口降低分母。 |
| ROOT-04 | P1 | Windows WPF 点 X 恒隐藏；当前默认 false 的 Hide2TrayWhenClose 允许退出。菜单隐藏已有入口，托盘恢复/退出生命周期仍未完整验证。 | desktop_integration.dart:69、215；原版 MainWindow.xaml.cs:193-196。 |
| ROOT-05 | P1 | 更新按钮永久隐藏；自身更新与内核安装链、目录和来源错位，正常后台检查未启用。 | main_shell.dart:469 / t16.rs / update_controller；与 RT-04/15/16、SET-20 合并修复。 |
| ROOT-06 | P1 | 启动隐藏只是读字段，第二实例/唤回未接应用锁；注销/关机会话结束未接原版 StorageUI+AppExit；RebootAs 无消费。 | main.dart / Win32 runner / DesktopIntegration；原版 App.xaml.cs、MainWindow.xaml.cs:146、162、199、318。 |
| ROOT-07 | P2 | 原版顶右主题 PopupBox 变浅深切换按钮，语言保存未用于 MaterialApp，许多文字固定字号。 | app.dart:24、main_shell.dart:458、theme_setting_dialog；原版 ThemeSettingView.xaml/.cs。WPF 只露前三主题，不能把另外四个 Avalonia 主题误算 WPF 缺入口。 |
| ROOT-08 | P1 | 三布局都显示代理/连接页，没有原版随当前 core 的 ShowClashUI 可见性条件；状态栏存在不证明 TUN/统计/模式实际生效。 | main_shell.dart:249、side_tabs.dart；原版 MainWindow.xaml.cs:383-424。运行链归 RT-05/09/10/14/18。 |
| ROOT-09 | P1/P2 | 布局本地 JSON 与 UiItem 两套来源，启动设置覆盖选择；原版 MainGirdHeight1/2 没消费。主窗口用 exe 旁 INI，各弹窗没有 TypeName 尺寸矩阵。 | ui_shell_controller.dart:184、219、351；runner main.cpp:18；原版 RestoreUI/StorageUI/WindowBase。主布局三种都有代码，不能说全没做。 |
| ROOT-10 | P2 | 确认弹窗只是分散 AlertDialog，订阅删除等触发/默认/取消/选中范围未恢复原版；Avalonia owner/尺寸/键盘未多平台实测。 | profile_actions.dart、subs_actions.dart；原版 UI.ShowYesNo、MessageBoxDialog、各调用处。 |

### 右键的问题必须拆开修

当前 HEAD 已修 `globalToLocal` 坐标、全表点击区域、外点/Esc 关闭、17 根条目/4 分隔、32px 高度，不能继续重复旧报告的“点不掉/没有分隔”。尚存的实测缺陷发生在命令执行：`_onContextAction` 关闭菜单，onClose 清 `_menuSession`；移动/生成后续函数再读这份 live 字段。冻结菜单会话中的目标应该在关闭前捕获并贯穿命令，不能靠关闭后重读选择补救。

随后还要恢复原版“当前分组”与“选中节点”的区别、重复备注映射 ID、子菜单 Esc 退层、按下/松开选择时序、窗口失焦关闭、多屏/DPI 边界。这些后者本轮未逐事件运行。不要为了改消失时机把菜单数据与业务副作用绑在 overlay 生存期上。

### 字体与拥挤的问题必须按结构验收

之前已提升表单行距、恢复顶部分组及本地化列头，新的 TUIC 截图仍显示内部 `CoreType/Network` 标签和不适用传输字段。设置页从冻结有效五页拆成十二页，控件分组、候选和联动行为都有变化。增大 padding 只能改变视觉；要先恢复原版分组/可见性/输入约束，再在默认字体以及 125%/150%/200% DPI 下检查文字、菜单、弹窗滚动和按钮可达性。语言和字号真正消费是必须修的功能，不只美术。

## 测试为何没有挡住这些差异

现有 `t17_menu_structure_test.dart` 与 `context_menu_model_test.dart` 合计 7 项全通过：能证明菜单 ID/顺序/条目结构，并且确实确认 disabled 项不可调用；不能证明它们迁移了完整功能。新增实际 Windows 六场景全部不符合冻结合同。profile 子代理的现有 codec 36 项通过，但以原版 PascalCase wire 构造的 2 条 InnerFmt 回归失败。配置生成 83 项、runtime adapter 5 项、RuntimePlan 11 项和 settings 专项 26 项通过，也不能证明生产启动与 UI 编辑事务完整。

本轮新增回归保留失败断言供修复，暂标 `#[ignore]`：默认 `cargo test -p subscriptions --locked` 退出0，97项通过、2项 ignored；显式 `-- --ignored` 仍2/2失败，详见 `profiles-inner-regression-ignored.log`。修复后应转绿并取消 ignore。全仓 cargo clippy/fmt/test、Flutter 全测试和 release 打包本轮未运行，当前交付是审查证据而非新 RC。
