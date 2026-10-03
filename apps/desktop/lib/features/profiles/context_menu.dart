/// Node table context menu, mirrored from upstream `ProfilesView.xaml`
/// `DataGrid.ContextMenu` at the frozen commit `7d6a967`
/// (compat/layouts.yaml LAY-PROFILES-004; actions compat/actions.yaml
/// ACT-PROF-001..032/038). The root entry order, four separators, grouping and
/// localized Chinese headers are restored from `upstream-menu-structure.json`
/// and `ResUI.zh-Hans.resx`; do not reorder or drop entries to make a test pass.
enum ContextActionKind {
  selectAll,
  moveTop,
  moveUp,
  moveDown,
  moveBottom,
  moveToGroup,
  edit,
  copy,
  delete,
  removeDuplicate,
  activate,
  share,
  exportClientConfig,
  exportClientConfigClipboard,
  exportShare,
  exportShareBase64,
  exportInner,
  tcping,
  realping,
  speedtest,
  udpTest,
  sortResult,
  removeInvalid,
  genGroupAll,
  genGroupRegion,
  notImplemented,
}

class ContextMenuEntry {
  const ContextMenuEntry({
    required this.label,
    required this.actionId,
    this.shortcut,
    this.kind = ContextActionKind.notImplemented,
    this.submenu = const <ContextMenuEntry>[],
    this.separatorAfter = false,
    this.enabled = true,
    this.helpTooltip,
    this.targetSubId,
  });

  final String label;
  final String actionId;
  final String? shortcut;
  final ContextActionKind kind;
  final List<ContextMenuEntry> submenu;
  final bool separatorAfter;

  /// Stable target subscription id for `移至订阅分组` entries. Carried as data
  /// instead of parsing the display label, so remark/prefix collisions can no
  /// longer move nodes into the wrong group (ACT-PROF-013). `''` means the
  /// "无分组" bucket; `null` for every other entry.
  final String? targetSubId;

  /// A disabled entry stays visible (layout parity) but never runs.
  final bool enabled;

  /// Extra hover explanation for entries that are visible but not actionable
  /// because the backend capability is missing. It supplements (never replaces)
  /// the entry label; the label itself must stay the upstream product text.
  final String? helpTooltip;

  bool get isSubmenu => submenu.isNotEmpty;
}

/// Resolve the localized Chinese header text from `ResUI.zh-Hans.resx`
/// (frozen fixture). Kept as a lookup so evidence can diff resource key ->
/// current label without depending on the GBK-mojibake `localizedHeader`
/// column in `upstream-menu-structure.json`.
const Map<String, String> contextMenuLabels = <String, String>{
  'menuSetDefaultServer': '设为活动',
  'menuEditServer': '编辑',
  'menuCopyServer': '克隆所选',
  'menuRemoveServer': '移除所选 (多选)',
  'menuRemoveDuplicateServer': '移除重复',
  'menuRemoveInvalidServerResult': '按测试结果移除无效',
  'menuTcpingServer': '测试延迟 Tcping (多选)',
  'menuRealPingServer': '测试真连接延迟 (多选)',
  'menuUdpTestServer': '测试 UDP 延迟 (多选)',
  'menuSpeedServer': '测试速度 (多选)',
  'menuSortServerResult': '按测试结果排序',
  'menuMoveToGroup': '移至订阅分组',
  'menuSubscription': '订阅分组',
  'menuMoveTo': '移至上下',
  'menuMoveTop': '上移至顶',
  'menuMoveUp': '上移',
  'menuMoveDown': '下移',
  'menuMoveBottom': '下移至底',
  'menuSelectAll': '全选',
  'menuShareServer': '分享',
  'menuExportConfig': '导出',
  'menuExport2ClientConfig': '导出所选完整配置',
  'menuExport2ClientConfigClipboard': '导出所选完整配置至剪贴板',
  'menuExport2ShareUrl': '导出分享链接至剪贴板 (多选)',
  'menuExport2ShareUrlBase64': '导出分享链接至剪贴板 (多选) Base64 编码',
  'menuExport2InnerUri': '导出 v2rayN 内部分享链接至剪贴板 (多选)',
  'menuGenGroupServer': '一键生成策略组',
  'menuAllServers': '全部配置项',
  'menuGenRegionGroup': '按地区分组',
};

/// The upstream context menu, root order and grouping restored.
///
/// `menuMoveToGroup` is filled at runtime with one entry per subscription
/// group plus a "无分组" entry; the constant below carries an empty submenu
/// that [ProfilesTable] replaces with the live group list.
const List<ContextMenuEntry> profilesContextMenu = <ContextMenuEntry>[
  ContextMenuEntry(
    label: '设为活动',
    actionId: 'ACT-PROF-005',
    shortcut: 'Enter',
    kind: ContextActionKind.activate,
  ),
  ContextMenuEntry(
    label: '编辑',
    actionId: 'ACT-PROF-001',
    shortcut: 'Ctrl+D',
    kind: ContextActionKind.edit,
  ),
  ContextMenuEntry(
    label: '克隆所选',
    actionId: 'ACT-PROF-004',
    kind: ContextActionKind.copy,
  ),
  ContextMenuEntry(
    label: '移除所选 (多选)',
    actionId: 'ACT-PROF-002',
    shortcut: 'Back',
    kind: ContextActionKind.delete,
  ),
  ContextMenuEntry(
    label: '移除重复',
    actionId: 'ACT-PROF-003',
    kind: ContextActionKind.removeDuplicate,
  ),
  ContextMenuEntry(
    label: '按测试结果移除无效',
    actionId: 'ACT-PROF-021',
    kind: ContextActionKind.removeInvalid,
    separatorAfter: true,
  ),
  ContextMenuEntry(
    label: '测试延迟 Tcping (多选)',
    actionId: 'ACT-PROF-016',
    shortcut: 'Ctrl+O',
    kind: ContextActionKind.tcping,
  ),
  ContextMenuEntry(
    label: '测试真连接延迟 (多选)',
    actionId: 'ACT-PROF-017',
    shortcut: 'Ctrl+R',
    kind: ContextActionKind.realping,
  ),
  ContextMenuEntry(
    label: '测试 UDP 延迟 (多选)',
    actionId: 'ACT-PROF-018',
    kind: ContextActionKind.udpTest,
    enabled: false,
    helpTooltip: '后端 SpeedTestSupport.udp=false，受限范围未实现',
  ),
  ContextMenuEntry(
    label: '测试速度 (多选)',
    actionId: 'ACT-PROF-019',
    shortcut: 'Ctrl+T',
    kind: ContextActionKind.speedtest,
  ),
  ContextMenuEntry(
    label: '按测试结果排序',
    actionId: 'ACT-PROF-020',
    kind: ContextActionKind.sortResult,
    separatorAfter: true,
  ),
  ContextMenuEntry(
    label: '移至订阅分组',
    actionId: 'ACT-PROF-013',
    kind: ContextActionKind.moveToGroup,
    // Runtime-populated from the live subscription list; the constant carries
    // an empty list so the entry is detected as `isSubmenu` only after the
    // table injects children.
    submenu: <ContextMenuEntry>[],
  ),
  ContextMenuEntry(
    label: '移至上下',
    actionId: 'ACT-PROF-009',
    kind: ContextActionKind.notImplemented,
    submenu: <ContextMenuEntry>[
      ContextMenuEntry(
        label: '上移至顶',
        actionId: 'ACT-PROF-009',
        shortcut: 'T',
        kind: ContextActionKind.moveTop,
      ),
      ContextMenuEntry(
        label: '上移',
        actionId: 'ACT-PROF-010',
        shortcut: 'U',
        kind: ContextActionKind.moveUp,
      ),
      ContextMenuEntry(
        label: '下移',
        actionId: 'ACT-PROF-011',
        shortcut: 'D',
        kind: ContextActionKind.moveDown,
      ),
      ContextMenuEntry(
        label: '下移至底',
        actionId: 'ACT-PROF-012',
        shortcut: 'B',
        kind: ContextActionKind.moveBottom,
      ),
    ],
  ),
  ContextMenuEntry(
    label: '全选',
    actionId: 'ACT-PROF-032',
    shortcut: 'Ctrl+A',
    kind: ContextActionKind.selectAll,
    separatorAfter: true,
  ),
  ContextMenuEntry(
    label: '分享',
    actionId: 'ACT-PROF-006',
    shortcut: 'Ctrl+F',
    kind: ContextActionKind.share,
  ),
  ContextMenuEntry(
    label: '导出',
    actionId: 'ACT-PROF-022',
    kind: ContextActionKind.notImplemented,
    submenu: <ContextMenuEntry>[
      ContextMenuEntry(
        label: '导出所选完整配置',
        actionId: 'ACT-PROF-022',
        kind: ContextActionKind.exportClientConfig,
        enabled: false,
        helpTooltip: '客户端完整配置导出桥接未提供；仅分享 URI 可用',
      ),
      ContextMenuEntry(
        label: '导出所选完整配置至剪贴板',
        actionId: 'ACT-PROF-023',
        kind: ContextActionKind.exportClientConfigClipboard,
        enabled: false,
        helpTooltip: '客户端完整配置导出桥接未提供',
      ),
      ContextMenuEntry(
        label: '导出分享链接至剪贴板 (多选)',
        actionId: 'ACT-PROF-024',
        shortcut: 'Ctrl+C',
        kind: ContextActionKind.exportShare,
        separatorAfter: true,
      ),
      ContextMenuEntry(
        label: '导出分享链接至剪贴板 (多选) Base64 编码',
        actionId: 'ACT-PROF-025',
        kind: ContextActionKind.exportShareBase64,
      ),
      ContextMenuEntry(
        label: '导出 v2rayN 内部分享链接至剪贴板 (多选)',
        actionId: 'ACT-PROF-026',
        kind: ContextActionKind.exportInner,
      ),
    ],
    separatorAfter: true,
  ),
  ContextMenuEntry(
    label: '一键生成策略组',
    actionId: 'ACT-PROF-007',
    kind: ContextActionKind.notImplemented,
    submenu: <ContextMenuEntry>[
      ContextMenuEntry(
        label: '全部配置项',
        actionId: 'ACT-PROF-007',
        kind: ContextActionKind.genGroupAll,
      ),
      ContextMenuEntry(
        label: '按地区分组',
        actionId: 'ACT-PROF-008',
        kind: ContextActionKind.genGroupRegion,
      ),
    ],
  ),
];
