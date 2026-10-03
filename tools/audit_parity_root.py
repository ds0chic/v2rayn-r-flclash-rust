"""Record root-owned source comparisons; never infer execution from an ID."""

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / "docs/evidence/parity-review-2026-10-03"
UPSTREAM = "work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/"
SHELL = "apps/desktop/lib/app/shell/main_shell.dart"
UI = "apps/desktop/lib/app/shell/ui_shell_controller.dart"
APP = "apps/desktop/lib/app/app.dart"
DESKTOP = "apps/desktop/lib/app/shell/desktop_integration.dart"
MENU = "apps/desktop/lib/app/menu/main_menu.dart"
TABS = "apps/desktop/lib/app/shell/side_tabs.dart"
NATIVE = "apps/desktop/windows/runner/main.cpp"
ENUMS = "crates/domain/src/enums.rs"
MODEL_LOG = "docs/evidence/parity-review-2026-10-03/root-widget-tests.log"
REAL_UI = "docs/evidence/parity-review-2026-10-03/ui-run-04/observations.json"


def detail(expected, actual, difference, refs, next_action, findings=(), status="identified", tests=()):
    return dict(upstream_expected=expected, current_behavior=actual,
                difference=difference, current_refs=list(refs),
                next_action=next_action, finding_ids=list(findings),
                status=status, tests_run=list(tests))


actions = {}
protocols = ["VMess", "VLESS", "Shadowsocks", "SOCKS", "HTTP", "Trojan", "Hysteria2", "TUIC", "WireGuard", "Anytls", "Naive"]
for index, protocol in enumerate(protocols, 1):
    actions[f"ACT-MAIN-{index:03}"] = detail(
        f"当前分组新增 {protocol}，使用 AddServerWindow 的专用字段、默认值、校验；取消丢草稿，保存再刷新/按原版条件重载。",
        f"{protocol} 菜单确实调用 startAddProfile，进入同一 Flutter 普通编辑器；保存经 BridgePort→AppEngine。",
        "入口存在。协议默认/必填/候选、当前组继承和已有值保留不能由菜单结构测试证明；详见 profiles 报告 PR-03/05/26/27。"
        + (" TUIC 实测只一个标 UUID 的 password 字段，没有 username 字段，UUID 与认证密码混写。" if protocol == "TUIC" else ""),
        [f"{SHELL}:{284 + (index - 1) * 2}", "apps/desktop/lib/features/profiles/profile_actions.dart", "apps/desktop/lib/features/profiles/profile_fields.dart", "crates/application/src/engine.rs:265"],
        f"对 {protocol} 单独走新增、字段错误、取消、保存、重开及生成配置；补齐 profiles 报告列出的该协议缺口。",
        ["PR-03"] if protocol == "TUIC" else ["PR-27"],
        tests=[MODEL_LOG] + ([REAL_UI] if protocol == "TUIC" else []))

for ident, name, fn in [
    ("ACT-MAIN-012", "自定义完整配置", "startAddCustomProfile(Custom)"),
    ("ACT-MAIN-013", "自定义出站", "startAddCustomProfile(Outbound)"),
    ("ACT-MAIN-014", "策略组", "startAddGroupProfile(PolicyGroup)"),
    ("ACT-MAIN-015", "代理链", "startAddGroupProfile(ProxyChain)"),
]:
    actions[ident] = detail(
        f"新增{name}使用专用 AddServer2Window/AddGroupServerWindow；重开编辑仍分派同一类编辑器，字段和配置效果保留。",
        f"主菜单新增正确分派到 {fn}；普通节点表 editSelectedProfile 则一律分派普通 editor。",
        "新增专用入口存在，但已有对象重开不能保持专用编辑行为；Custom 文本存放键与配置生成消费链亦需修复。",
        [f"{SHELL}:306", "apps/desktop/lib/features/profiles/profile_actions.dart", "apps/desktop/lib/features/profiles/custom_editor_dialog.dart", "apps/desktop/lib/features/profiles/group_editor_dialog.dart"],
        f"新增{name}后从节点表双击/右键编辑，必须按类型分派、保存不丢未知字段；配置效果用合成数据验收。",
        ["PR-02"])

actions.update({
    "ACT-MAIN-016": detail("Ctrl+V/菜单批量导入原版接受的分享、inner、完整配置格式到当前组，文本框聚焦不劫持粘贴。", "调用 importFromClipboard→importFromText；合成 VLESS 分享实测新增 1 条；合法完整 Xray JSON 实测 0 条，1 个保存失败（E_FIELD_REQUIRED），UI 另提示 1 行未识别。第一轮不合法夹具已排除。", "导入能力不是原版完整集合；InnerFmt 的原版 PascalCase 也有独立失败测试；当前正常分享路径能工作。", [f"{SHELL}:327", "apps/desktop/lib/features/subs/subs_actions.dart:19", "crates/bridge_api/src/api/subs.rs", "crates/subscriptions/src/fmt/inner.rs"], "统一批量格式识别及导入存储键，必须以原版产生的载荷而非自己 roundtrip 证明兼容。", ["ROOT-01"], tests=[REAL_UI, "docs/evidence/parity-review-2026-10-03/profiles-inner-regression-ignored.log"]),
    "ACT-MAIN-017": detail("菜单/Ctrl+S 隐藏窗口、截图识别二维码、恢复窗口，识别成功再导入；没找到给相应反馈。", "菜单和 Ctrl+S 均调用 shareProfilesQr；真实界面开分享已有节点的二维码窗口。", "扫描导入被接成生成分享，方向相反。", [f"{SHELL}:158", f"{SHELL}:328"], "独立实现屏幕 QR 扫描用例，取消/错误/窗口恢复完整；与分享二维码彻底分开 action。", ["ROOT-02"], tests=[REAL_UI]),
    "ACT-MAIN-018": detail("选择图片文件→识别二维码→导入；取消文件选择结束。", "调用 importFromTextDialog，实际打开粘贴文本对话框。", "没有图片文件选择与 QR 解码链。", [f"{SHELL}:330", "apps/desktop/lib/features/subs/subs_actions.dart:44"], "补图片选择、解码、取消、无二维码/格式错误反馈；复用真正的导入服务。", ["ROOT-02"], tests=[REAL_UI]),
    "ACT-MAIN-019": detail("订阅设置退出成功后刷新主窗口分组；普通组允许空网址。", "打开 SubSettingWindow→SubEditWindow；只备注保存实测 URL required 阻止，主分组刷新与设置状态分属 controller。", "原版普通组基本用例被拒绝；主窗口分组刷新/当前组更新对象亦需端到端核对。", [f"{SHELL}:332", "apps/desktop/lib/features/subs/sub_edit_window.dart:132", "apps/desktop/lib/features/subs/subs_controller.dart", "apps/desktop/lib/features/profiles/profiles_controller.dart"], "修 SET-01/02：普通组与 URL 订阅分开校验，提交后主分组同步，更新命令用主窗口当前组 ID。", ["SET-01", "SET-02"], tests=[REAL_UI]),
    "ACT-MAIN-035": detail("F5/重载命令对当前默认节点重建配置、加载核心、更新代理，重入排队。", "顶层重载禁用，F5 只提示 preservedTooltip；旁边新增应用/停止 toolbar 是另一套入口。", "原版 F5 流程未实现，新增按钮存在不能抵销适用入口。", [f"{MENU}:161", f"{SHELL}:153", "apps/desktop/lib/app/shell/main_shell.dart"], "将 F5/原版重载入口接同一受管 apply 用例，保留状态/重复操作语义；不得触及 10808 做测试。", ["ROOT-03"], status="preserved_only", tests=[MODEL_LOG]),
    "ACT-WIN-001": detail("Windows WPF 点击窗口 X 一律取消关闭并隐藏到托盘。", "setPreventClose 取 Hide2TrayWhenClose；默认 false 时没有阻止原生退出。", "把 Avalonia 的可选行为套到了 WPF 主合同；用户点击 X 可能退出而非隐藏。", [f"{DESKTOP}:69", f"{DESKTOP}:215", f"{NATIVE}:83"], "按平台冻结合同实现 X/Alt+F4/菜单关闭/托盘退出的区别；测试前用隔离配置且代理 unchanged。", ["ROOT-04"]),
    "ACT-WIN-002": detail("关闭菜单隐藏到托盘，内核继续由原版生命周期管理。", "有 DesktopIntegration 时调用 hideToTray；没 integration 时只状态消息。", "隐藏入口接线存在，但托盘恢复、窗口/内核运行保持未在本轮真实点击验证。", [f"{SHELL}:381", f"{DESKTOP}:130"], "隔离环境验证关闭→托盘恢复→正式退出三条路径，检查受管运行时和状态持久化。", status="implemented", tests=[MODEL_LOG]),
    "ACT-WIN-003": detail("推广菜单根据原版站点地址打开浏览器。", "顶层推广 preservedOnly，禁用。", "适用入口保留文字但不实现动作。", [MENU, f"{SHELL}:391"], "按冻结来源补 URL 打开及失败反馈，不把 disabled 判完成。", ["ROOT-03"], status="preserved_only", tests=[MODEL_LOG]),
    "ACT-WIN-005": detail("检查更新弹窗按已保存勾选/代理/预览选项检查并更新选择的应用与内核。", "窗口可打开，但选项为内存默认；应用 ID 走核心安装链、路径/仓库/外部 runner 接线均有差异。", "进入窗口是实现了；完整更新、下次重开选项以及应用安装不能认作对齐。", [f"{SHELL}:375", "apps/desktop/lib/features/update/update_controller.dart", "crates/bridge_api/src/api/t16.rs:856"], "按 runtime/settings 更新报告修 app/core 分流与安装消费链，再用自建合成发行源验证，不安装原版 WPF 到 Flutter 目录。", ["ROOT-05", "SET-20"]),
    "ACT-WIN-006": detail("每日检查发现新版本后按钮可见，点击打开更新窗口。", "Visibility visible:false，onPressed 为 notImplemented。", "按钮永久隐藏、通知业务未接线。", [f"{SHELL}:469", "apps/desktop/lib/features/update/update_controller.dart"], "在后台任务建立后接真实版本通知及窗口动作，保存检查选项；不能仅改 visible=true。", ["ROOT-05"], status="preserved_only"),
    "ACT-WIN-008": detail("帮助动态填充核心名称/网站，点击打开对应项目地址。", "一个泛化核心网站 preservedOnly 子项，无动态核心清单与 URL 打开。", "原版动态多条菜单变一条禁用占位。", [MENU, f"{SHELL}:391"], "由冻结核心元数据生成原版条目及地址；故障提示和菜单顺序验收。", ["ROOT-03"], status="preserved_only", tests=[MODEL_LOG]),
    "ACT-WIN-009": detail("Windows SessionEnding 保存窗口/布局，再 AppExitAsync(false) 清理受管运行。", "Flutter runner 消息循环与 DesktopIntegration 关闭处理存在；没有 WM_QUERYENDSESSION/WM_ENDSESSION 对应业务接线。", "系统注销/关闭不能复用未触发的 Flutter dispose 当作已保证收尾。", [NATIVE, "apps/desktop/windows/runner/flutter_window.cpp", f"{DESKTOP}:215", f"{APP}:69"], "接宿主会话结束通知→受管退出与状态落盘；本机不触发真实注销，使用隔离 Windows 会话验收。", ["ROOT-06"]),
    "ACT-WIN-010": detail("RebootAs 重启等候及单实例锁，在应用初始化前生效。", "Win32 转交参数但 Dart 未处理 RebootAs，SingleInstanceGuard 仅平台测试调用。", "平台有锁不代表实际入口加锁；两个进程可能同时打开相同数据目录。", [NATIVE, "apps/desktop/lib/main.dart", "crates/platform/src/single_instance.rs", "crates/platform/tests/single_instance.rs"], "启动前接单实例/IPC 显示窗口/明确重启参数合同，锁住 canonical 数据目录；隔离目录启动两实例验证。", ["ROOT-06"]),
    "ACT-WIN-011": detail("AutoHideStartup 为 true 时 OnLoaded 隐藏，ShowInTaskbar 与显示状态保持。", "CloseBehavior 读取 autoHideStartup，但 DesktopIntegration.start 未调用隐藏；窗口默认创建后显示。", "读取字段并没有执行启动隐藏。", [f"{DESKTOP}:29", f"{DESKTOP}:52", NATIVE], "启动隐藏与托盘创建成功的时序接线，恢复窗口时 taskbar/focus 状态跟随。", ["ROOT-06"]),
    "ACT-WIN-012": detail("第二实例通知第一实例显示/激活窗口后退出。", "没有实际单实例锁，也没有第一实例接收通知显示的 IPC 流程。", "不能只借平台锁单测声称原版第二实例体验。", ["apps/desktop/lib/main.dart", NATIVE, "crates/platform/src/single_instance.rs"], "与 ROOT-06 同一生命周期补第一实例窗口通知，处理启动中/隐藏中/已经退出情况。", ["ROOT-06"]),
})

layout_info = {
    "LAY-MAIN-004": ("顶部原版六组菜单和主题/更新控件结构", "根菜单顺序已有模型测试通过；新增运行 toolbar/布局选择/主界面占位子菜单，F5/部分帮助禁用。", "菜单存在不等于功能原版对齐；新增控件改变了冻结结构和占位用语。", [SHELL, MENU], "ROOT-03"),
    "LAY-MAIN-005": ("原版顶右主题 PopupBox 内嵌 ThemeSettingView，语言/字体/色彩即时生效", "顶右按钮只 light/dark toggle，完整主题弹窗移到设置菜单；主题持久化存在但页面中文与固定字号仍多。", "原版快速入口、语言实际消费和全局字号不一致。", [f"{SHELL}:458", APP, "apps/desktop/lib/features/settings/theme_setting_dialog.dart"], "ROOT-07"),
    "LAY-MAIN-006": ("底部 StatusBarView 宿主与当前有效状态对应", "底部 StatusBarView 已有，运行模式/代理/TUN/统计状态消费链由 runtime 报告核对。", "位置吻合；TUN 未接线、统计链不启动，显示不能证明真实效果。", [f"{SHELL}:173", "apps/desktop/lib/app/shell/status_bar_view.dart"], "ROOT-08"),
    "LAY-MAIN-007": ("保存恢复 UiItem 的分栏星值和各窗口尺寸", "分栏比例存 FileUiStateStore，applySettingsDocument 不读 MainGirdHeight1/2；主窗口尺寸保存在 exe 旁 INI。", "导入原版布局/备份迁移无法消费对应设置；启动设置又覆盖 local layout mode。", [f"{UI}:184", f"{UI}:219", NATIVE], "ROOT-09"),
    "LAY-DIALOG-WPF-001": ("冻结 Windows 确认弹窗的选择/默认/取消及触发条件", "Flutter AlertDialog 局部实现，不是系统 MessageBox；订阅删除没有冻结版确认，节点批量删除范围见 profiles。", "触发/确认合同未统一，不只是视觉风格差。", ["apps/desktop/lib/features/profiles/profile_actions.dart", "apps/desktop/lib/features/subs/subs_actions.dart"], "ROOT-10"),
    "LAY-MSGBOX-AVA-001": ("Avalonia 跨平台 MessageBoxDialog 可滚动只读文本、确认/取消、默认/取消键", "Flutter AlertDialog；本轮没有 macOS/Linux 窗口实测。", "不能声明 Avalonia 各平台按钮键盘/尺寸合同已经一致。", ["apps/desktop/lib/features/profiles/profile_actions.dart", "apps/desktop/lib/features/subs/subs_actions.dart"], "ROOT-10"),
    "LAY-WINSIZE-001": ("WindowBase 按 TypeName 保存/恢复每个窗口宽高", "Flutter showDialog 使用各自固定 constraints；主窗口 runner INI，UiStateStore loadWindowState/saveWindowState 只有定义。", "原版持久化窗体矩阵未消费；弹窗重开尺寸不同，迁移原配置不能恢复。", [NATIVE, "apps/desktop/lib/features/profiles/ui_state_store.dart", "apps/desktop/lib/features/profiles/profile_editor_dialog.dart"], "ROOT-09"),
}

enum_info = [
    ("15 个 EConfigType 数值与类型", "Rust ConfigType 的整数映射全保留；不同编辑器/codec/codegen 的实际使用有 profiles 缺口。", "标识形状一致不等于 15 类全部端到端适用功能一致。", [ENUMS], None),
    ("15 个 ECoreType，v2rayN=99 是应用更新标识", "Rust CoreType 15 值匹配；真正 runtime adapter 只有 Xray/sing-box，app ID 更新分流有错。", "枚举保留而其它适用 custom core 运行能力缺失。", [ENUMS, "crates/application/src/engine.rs:2029"], "ROOT-05"),
    ("Horizontal=0/Vertical=1/Tab=2 布局", "Rust数值与 UiShell orientation 映射存在，默认垂直；持久化来自两个文档。", "切布局/导入/重开的统一状态源和分栏尺寸未对齐。", [ENUMS, UI], "ROOT-09"),
    ("五种全局热键动作值与 WPF KeyCode 编码", "Flutter GlobalHotkeyAction 五值存在，录制为 Flutter keyId；registrar 当 VK，且没有 action 回调。", "原版 WPF Key 枚举必须先转换为 VK，三个编码互不兼容。", ["apps/desktop/lib/features/settings/hotkeys.dart", "apps/desktop/lib/features/settings/global_hotkey_window.dart"], "SET-15"),
    ("入站 0..6/21 值与端口偏移", "Rust InboundProtocol 数值匹配，codegen 按首项协议选择；当前旧测试名字总mixed已不代表实现。", "没有从旧测试名推出协议丢失；完整 LAN/PAC/API/UDP 会话效果仍未验证。", [ENUMS, "crates/application/src/engine.rs:1839"], None),
    ("Top=1/Up=2/Down=3/Bottom=4/Position=5 移动行为", "ProfilesController 提供 Top/Up/Down/Bottom 的内存重排；Position 与持久化 sort 对齐未完成。", "刷新重开丢顺序，节点/规则各自编辑提交语义需区分。", ["apps/desktop/lib/features/profiles/profiles_controller.dart:991", "apps/desktop/lib/features/routing/routing_windows.dart"], "PR-15"),
    ("五种策略组模式按原版整数存储/选择并生成效果", "Rust MultipleLoad 五变体、bridge DTO 使用 mode 值和组生成已有；完整模式效果核对归 profiles/runtime。", "枚举全不代表运行会话和组编辑无误；右键生成实际失去会话目标。", [ENUMS, "crates/application/src/groups.rs", "apps/desktop/lib/features/profiles/profiles_table.dart:838", "apps/desktop/lib/features/profiles/profiles_table.dart:920", "apps/desktop/lib/features/profiles/profiles_table.dart:952"], "PR-19"),
    ("Default/Russia/Iran 三种区域预设下载路由/DNS/资源", "菜单 preservedOnly；后端离线预设记 pending_remote_templates。", "适用原版预设尚未完成远程资源消费，不能改分母。", [MENU, "crates/application/src/engine.rs:700"], "SET-19"),
    ("Rule/Global/Direct 三模式", "Rust RuleMode 数值与生成器存在；状态栏和 Clash 代理页模式控制并未完成相同 session。", "模式配置生成与实际 Clash API 模式同步须验收，不能以枚举对齐代替。", [ENUMS, "apps/desktop/lib/features/monitor/proxies_view.dart", "crates/core_adapters/src/clash_api.rs"], "ROOT-08"),
    ("ALL/Routing/DNS 三种规则资源分类", "Rust RuleType 0/1/2 映射存在；路由详情重构 DTO 可能丢带 Type 的既有值。", "整数一致；编辑保存未知/已有属性仍需保留及远程资源生效验收。", [ENUMS, "apps/desktop/lib/features/routing/routing_windows.dart"], "SET-08"),
    ("15 列含今日/累计上下流量及列宽排序", "本轮之前恢复了中文列头，模型键存在；dtoToSummary 统计值全置0，排序与持久化链不全。", "列能画出，但统计/排序数据与原版不同。", ["apps/desktop/lib/bridge/bridge_port.dart", "apps/desktop/lib/features/profiles/profiles_controller.dart"], "PR-22"),
    ("六类测速动作及选中/当前组范围", "Rust SpeedTestAction 与 Dart 测速动作存在；mixed/fast 空 ids 表示整个 DB。", "原版当前组/可见对象变成全库；操作对象和重开结果一致性需修。", [ENUMS, "apps/desktop/lib/features/profiles/profiles_controller.dart"], "PR-16"),
    ("四种系统代理模式及各自副作用", "Rust SysProxyType 四值匹配，平台实现存在；托盘 PAC/热键链有未接线。", "本轮未改宿主代理，枚举保留不冒充四模式真机验收。", [ENUMS, DESKTOP, "apps/desktop/lib/features/settings/platform_controller.dart"], "ROOT-08"),
    ("ETheme 七值，WPF UI 只取前三；额外四值按 Avalonia 平台审查", "Flutter 支持系统/浅/深和 accent 选择；其余 Avalonia 主题未复现。", "不把 WPF 隐藏的四主题算 Windows 缺入口；Avalonia 主题/语言/字号合同另记未验证。", [APP, "apps/desktop/lib/features/settings/theme_setting_dialog.dart"], "ROOT-07"),
    ("九个 transport 枚举；Global.Networks 控件只暴露六种", "Rust Network 包含九变体；编辑器暴露范围和旧载荷值回显须看 profile_fields。", "不把只保留的 h2/http/quic 误认为原版普通窗口必选项；已有值无损编辑和生成的各核能力仍需验收。", [ENUMS, "apps/desktop/lib/features/profiles/profile_fields.dart"], "PR-26"),
]


def build():
    rows = json.loads((DEST / "root-assigned.json").read_text(encoding="utf8"))
    result = []
    for row in rows:
        ident, kind = row["id"], row["kind"]
        source = row["source_contract"]
        if kind == "action":
            item = actions[ident]
        elif kind == "layout":
            expected, actual, diff, refs, finding = layout_info[ident]
            item = detail(expected, actual, diff, refs, "按该行冻结布局和对应领域报告恢复控件、生命周期和状态源；实际 DPI/字体/键盘/重开验收。", [finding])
        elif kind == "main_layout":
            name = source["mode"]
            item = detail(
                json.dumps(source["panels"], ensure_ascii=False),
                f"{name} 主结构有对应 Flutter 构建；分栏可拖、侧标签存在。",
                "ShowClashUI 随活动内核的可见性条件未接；共用 tabIndex 跨 layout 表示不同对象；布局/分栏导入原版配置与重開状态源不统一。",
                [f"{SHELL}:181", TABS, f"{UI}:219"],
                "按稳定 tab ID 迁移布局选择；接原版 core visibility 条件和配置持久化；三布局×切核×重开×DPI 矩阵。", ["ROOT-09", "ROOT-08"])
        elif kind == "enum":
            expected, actual, diff, refs, finding = enum_info[int(ident[-3:]) - 1]
            item = detail(expected, actual, diff, refs, "保留该行 values 与平台范围，消费端按对应领域用例验收；不得只数枚举项。", [finding] if finding else [], status="identified" if finding else "implemented")
        elif kind == "window":
            name = source["name"]
            if name == "MainWindow":
                item = detail("原版默认 1200×800，按平台最小尺寸、WindowBase 与主窗口布局恢复；不改变菜单结构。", "Windows runner 默认同尺寸，min 800×600；主窗口单 INI 而不是 TypeName 窗体矩阵。", "WPF 最小高度新增600、Avalonia 最小宽600未分平台；尚未测 macOS/Linux/原版双窗口，布局/关闭/隐藏差异见 root/runtime。", [NATIVE, DESKTOP, SHELL], "修窗口生命周期与 UiItem 持久化后按平台/DPI验收，别把 Win32 默认尺寸等同所有窗口对齐。", ["ROOT-04", "ROOT-09"])
            elif name == "MessageBoxDialog":
                item = detail("Avalonia 默认宽420、可滚动消息、默认/取消按钮与 owner 居中。", "Flutter 复用局部 AlertDialog，没有本轮多平台实测。", "只确认有弹窗类型，不能对跨平台尺寸/键盘/焦点作已验证判断。", ["apps/desktop/lib/features/profiles/profile_actions.dart", "apps/desktop/lib/features/subs/subs_actions.dart"], "共用确认弹窗语义并按平台实际运行，原版 owner/键盘合同保留。", ["ROOT-10"])
            else:
                item = detail(f"{name} 的原版应用资源/主题/localization 与初始化合同", "Flutter MaterialApp/buildAppTheme 汇聚资源；Win32 runner/Main Dart 初始化替代 WPF/Avalonia App。", "不需要逐文件照搬 XAML，但语言实际消费、固定字号、缺生命周期与适用平台行为没有全部迁移。", [APP, "apps/desktop/lib/shared/theme/app_theme.dart", "apps/desktop/lib/main.dart"], "按资源实际消费者与启动/关闭流程逐项恢复；本轮除 Windows 六场景外未验多平台。", ["ROOT-07", "ROOT-06"])
        else:
            raise ValueError(row["key"])
        item = dict(item)
        refs = source.get("evidence", [])
        refs = refs or source.get("source_file", []) or [source.get("file", row["ledger"])]
        item.update(key=row["key"], id=ident, kind=kind, label=row["label"],
                    upstream_refs=[UPSTREAM + r if r.startswith("v2rayN/") else r for r in refs],
                    source_contract=source, ledger_status=row["ledger_status"],
                    evidence_level="source_trace_and_current_ui" if REAL_UI in item["tests_run"] else "source_trace",
                    priority="P1" if item["finding_ids"] else None,
                    limitations=["原版预期由冻结源码追踪；本轮未启动原版双窗口逐事件比较。真实 Windows 测试仅证明记录的当前场景，不覆盖其它功能/重开/所有内核。macOS/Linux/ARM64 未验证。"])
        result.append(item)
    assert len(result) == 62
    assert {x["key"] for x in result} == {x["key"] for x in rows}
    replacements = {
        "custom_editor_dialog.dart": "custom_editor_dialog.dart",
        "group_editor_dialog.dart": "group_editor_dialog.dart",
        "app/shell/main_shell.dart": "app/shell/main_shell.dart",
        "app/shell/status_bar_view.dart": "app/shell/status_bar_view.dart",
        "bridge/bridge_port.dart": "bridge/bridge_port.dart",
        "features/routing/routing_windows.dart": "features/routing/routing_windows.dart",
        "features/update/update_controller.dart": "features/update/update_controller.dart",
    }
    enum_findings = {"ENUM-006": "PR-15", "ENUM-007": "PR-19", "ENUM-011": "PR-22", "ENUM-012": "PR-16"}
    for item in result:
        item["current_refs"] = [next((ref.replace(a, b) for a, b in replacements.items() if a in ref), ref) for ref in item["current_refs"]]
        if item["id"] in enum_findings:
            item["finding_ids"] = [enum_findings[item["id"]]]
        if item["id"] == "ACT-MAIN-016":
            item["current_behavior"] = "调用 importFromClipboard→importFromText；合成 VLESS 分享实测新增 1 条；合法完整 Xray JSON 实测 0 条，1 个保存失败（E_FIELD_REQUIRED），UI 另提示 1 行未识别。第一轮不合法夹具已排除。"
        if item["kind"] in ("layout", "window") and item["finding_ids"] == ["ROOT-10"]:
            item["priority"] = "P2"
    (DEST / "root-items.json").write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf8")
    print(f"root rows: {len(result)}; keys exact")


if __name__ == "__main__":
    build()
