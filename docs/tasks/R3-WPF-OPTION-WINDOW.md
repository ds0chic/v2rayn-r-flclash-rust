# R3-WPF-OPTION-WINDOW — 参数设置改为独立顶层窗口

状态：`identified`（原版独立 WPF 窗口，当前 Flutter `showDialog`；尚未实施）。

任务 ID：R3-WPF-OPTION-WINDOW。

本次唯一用户流程：主窗口选择“设置→参数设置”后打开一个可由 Windows 识别的独立、隶属主窗口的“设置”窗口；在其中编辑设置并确定/取消/按关闭按钮，焦点、再次打开、保存与运行生效时机均与冻结原版一致。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；[三窗口真机截图](../evidence/recheck-fixes/R3-WPF-COMPARE/README.md)；Wave K 标签/字段/按钮已对齐，不能回退。先做最小 Flutter Windows 多窗口 spike：原生顶层句柄、owner/modal/focus、关闭与打包可验证后再迁移本窗口，不凭插件名称推断可行。

对应 feature / field / action / layout ID：`ACT-MAIN-024`、`ACT-OPT-001`、`LAY-OPTSET-001`、`F-DESKTOP-001`。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/ViewModels/MainWindowViewModel.cs:583-595`（`ShowDialogAsync` 返回 true 才刷新/Reload）、`v2rayN/v2rayN/Views/OptionSettingWindow.xaml` 与 `.xaml.cs`；当前 `apps/desktop/lib/features/settings/option_setting_window.dart:25-35`、`settings_actions.dart:13-20`、`app/shell/main_shell.dart`。

输入、输出、错误、取消、权限、持久化及生效语义：输入是当前 settings 快照；打开失败要可见反馈，不得伪装已打开。确定经现有设置写入路径保存，成功后才按原版刷新主窗口与运行；验证失败留在子窗口。取消、Esc、标题栏关闭均不写草稿。主窗口隐藏/最小化或第二次点菜单时不得丢失/复制编辑状态。仅本机窗口权限，不启动代理、不写系统代理/TUN。

允许修改的模块：`apps/desktop/lib/features/settings/option_setting_window.dart`、`settings_actions.dart`、专用桌面窗口宿主/通信模块、`apps/desktop/windows/**`（仅必要原生窗口代码）、相关 widget/integration 测试、此卡与证据。新增依赖须固定版本并经 release 构建验证。

禁止改变的已有行为：五页分组与字段、Wave K 文案、FRB/Rust 设置保存合同、设置修订冲突/取消语义；不改 `work/`、`outputs/` 或 10808，不借独立窗口重写业务后端。

测试夹具和原版预期：空隔离数据目录 + 合成设置，不读用户配置。原版窗口是独立 Windows 顶层、由 `ShowDialogAsync` 打开；保存返回 true 才触发主窗口更新。对照原版/RC 两进程窗口句柄、owner、焦点、Esc/关闭、两次打开、100%/150% DPI 的截图与事件。

本次必须通过的命令/真实场景：`dart format --output=none --set-exit-if-changed lib test integration_test`、`flutter analyze`、目标 widget/integration tests、`flutter build windows --release`；真实 Windows 隔离目录启动发布包，窗口探针确认两个独立 HWND，确定/取消/关闭与重新打开均正确。无需宿主代理或 TUN 写入。

证据文件位置：`docs/evidence/recheck-fixes/R3-WPF-OPTION-WINDOW/`（spike、窗口探针、截图、测试日志、失败/取消记录）。

完成条件：以上实际窗口与设置生效流程通过，并与冻结 WPF 窗口逐事件/布局对照；仅组件渲染或 `showDialog` 通过不得记 `verified`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减独立窗口需求。
