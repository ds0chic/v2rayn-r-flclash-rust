# compat 台账模式（T00 冻结版）

本文件定义 `compat/` 下四份台账与平台矩阵的条目模式。所有台账共享以下通用字段：

| 字段 | 必填 | 含义 |
|---|---|---|
| `id` | 是 | 稳定 ID。格式：`F-<家族>-NNN` / `FLD-<组>-NNN` / `ACT-<域>-NNN` / `LAY-<视图>-NNN` |
| `name` | 是 | 原版符号或原版功能的可读名称（保留原始大小写与下划线） |
| `status` | 是 | 仅允许：`identified` / `implemented` / `verified` / `preserved_only` / `blocked` / `not_applicable`。T00 全部为 `identified` 或 `not_applicable`（须附证据） |
| `platforms` | 是 | `[windows, macos, linux]` 的子集；原生界面基准单独注明 `wpf` / `avalonia` |
| `source_commit` | 是 | `7d6a967c18c697f28dc6917122ed3a4993fcf336` |
| `source_file` | 是 | 相对仓库根的源码路径（冻结 commit 内） |
| `source_symbol` | 建议 | C#/XAML 符号（类、属性、命令、控件 x:Name） |
| `evidence` | 是 | `["<file>:<line>", ...]` 实际读取到的证据位置 |
| `unresolved` | 否 | 无法核实的事实清单；禁止用“默认应该如此”填充 |
| `notes` | 否 | 补充说明 |

不允许：无证据的 `verified`、无上游依据的 `not_applicable`、把推测的默认值当作已验证（推测值必须标 `default_verified: false` 并写入 `unresolved`）。

## features.yaml

一条 = 一个独立功能。

```yaml
- id: F-PROFILE-001
  name: 添加普通节点
  family: profile            # profile/subscription/import-export/test/routing/dns/core/system-proxy/tun/monitor/desktop/app-mgmt/backup/help
  summary: 从菜单选择 11 类基础协议之一新建节点
  platforms: [windows, macos, linux]
  upstream_entry:
    window: MainWindow
    menu_path: 添加[VMess/VLESS/...]节点
    symbol: MainWindowViewModel.AddVmessServerCmd
    files: [v2rayN/ServiceLib/ViewModels/MainWindowViewModel.cs]
  depends_on_cores: [xray, sing-box]
  depends_on_features: []
  preconditions: []
  normal_behavior: 打开 AddServerWindow，编辑草稿，保存后写入 guiNDB.db ProfileItem
  failure_behavior: 校验失败显示字段错误，不落库
  cancel_behavior: 关闭窗口丢弃草稿
  acceptance: <可执行的验收方法>
  source_commit: 7d6a967c18c697f28dc6917122ed3a4993fcf336
  implementation_location: null
  test_ids: []
  evidence: []
  status: identified
```

## fields.yaml

一条 = 一个设置或持久化字段。

```yaml
- id: FLD-CORE-001
  name: LogEnabled
  display_key: ResUI.EnableLog     # 翻译资源键（如可定位）
  page_group: 参数设置/核心:基础设置
  order: 1
  type: bool
  default_value: true
  default_verified: true           # 仅当追踪过 LoadConfig/初始化/迁移后可标 true
  null_handling: 缺省视为 false；null 不恢复默认
  range: null
  enum_values: []
  linked_fields: []
  storage:
    kind: guiNConfig.json          # guiNConfig.json | guiNDB.db | external_file
    key: GuiItem.LogEnabled        # 或表.列
  persisted_entity: AppSettings    # 建议的 Rust 域模型归属
  generated_into:                  # 实际进入哪些内核配置（无则空）
    xray: [log.loglevel 相关]
    sing-box: [log]
  apply_timing: restart_core       # immediate | save | restart_core | restart_app | next_launch
  ui_control: CheckBox
  ui_binding_symbol: GuiItem.LogEnabled
  platform_scope: all
  classification: user_setting     # user_setting | internal_state | ui_pref | legacy_migration | obsolete_candidate
  source_commit: 7d6a967c18c697f28dc6917122ed3a4993fcf336
  source_file: v2rayN/ServiceLib/Models/Configs/ConfigItems.cs
  source_symbol: CoreBasicItem.LogEnabled
  evidence: []
  unresolved: []
  notes: null
  status: identified
```

## actions.yaml

一条 = 一个用户动作（菜单、快捷键、键鼠、托盘、定时任务、取消行为）。

```yaml
- id: ACT-TABLE-001
  name: 编辑选中节点
  domain: profiles             # main-menu/profiles/subscriptions/routing/dns/settings/tray/hotkey/statusbar/testing/backup
  entry:
    menu_path: 节点右键/编辑
    shortcut: Ctrl+D
    mouse: 双击（当 DoubleClick2Activate=false）
    tray: null
    hotkey: null
  scope: 节点表选中项（单选/多选规则见 selection_rule）
  selection_rule: 右键保留已有多选；无选中时命令禁用
  confirm: 无
  cancel: 关闭编辑窗口丢弃草稿，不落库不重启内核
  async: { job: false, cancellable: false }
  feedback: 保存成功刷新行；失败显示字段错误
  scheduled: null              # 定时任务写触发间隔与来源
  upstream_symbol: ProfilesViewModel.EditServerCmd
  source_file: [v2rayN/ServiceLib/ViewModels/ProfilesViewModel.cs]
  evidence: []
  status: identified
```

## layouts.yaml

一条 = 一个窗口/区域/布局状态。

```yaml
- id: LAY-MAIN-001
  name: 主窗口-垂直布局
  window: MainWindow
  platform: windows-wpf         # windows-wpf | windows-avalonia | macos-avalonia | linux-avalonia
  layout_mode: vertical         # horizontal | vertical | tab
  structure: 上方节点表 + 下方信息/代理/连接标签；底部状态栏
  anchors: []                   # 关键锚点与控件层级
  default_size: { width: 1200, height: 800 }
  min_size: { width: 800, height: null }
  splitter: { horizontal: MainGirdHeight1, vertical: MainGirdHeight2 }
  table:
    columns: []                 # 默认列顺序/列宽来源
    virtualization: unknown     # 上游行为；Flutter 侧另行验收
  persistence_keys: [UIItem.MainGirdHeight1, UIItem.MainGirdHeight2, UIItem.MainGirdOrientation, WindowSizeItem]
  screenshots: []               # 本环境不能采集时写 unverified 并给采集步骤
  source_file: [v2rayN/v2rayN/Views/MainWindow.xaml]
  evidence: []
  status: identified
```

## platform-matrix.md 结构

- 六交付单元（Win x64/ARM64、macOS x64/ARM64、Linux x64/ARM64）与上游额外架构（Windows x86、Linux riscv64/loong64）差异表。
- WPF 与 Avalonia 可见性/行为差异（菜单、主题入口、平台条件分支）。
- Flutter 标准引擎支持矩阵与差异处理状态（T01 验证前标 unverified）。
