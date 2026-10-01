/// Node table context menu, mirrored from upstream `ProfilesView.xaml`
/// `DataGrid.ContextMenu` (compat/layouts.yaml LAY-PROFILES-004) and the
/// `ACT-PROF-*` entries in compat/actions.yaml.
enum ContextActionKind {
  selectAll,
  moveTop,
  moveUp,
  moveDown,
  moveBottom,
  edit,
  copy,
  delete,
  activate,
  remarks,
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
  });

  final String label;
  final String actionId;
  final String? shortcut;
  final ContextActionKind kind;
  final List<ContextMenuEntry> submenu;
  final bool separatorAfter;

  bool get isSubmenu => submenu.isNotEmpty;
}

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
    shortcut: 'Backspace',
    kind: ContextActionKind.delete,
  ),
  ContextMenuEntry(label: '移除重复', actionId: 'ACT-PROF-003'),
  ContextMenuEntry(label: '按测试结果移除无效', actionId: 'ACT-PROF-021'),
  ContextMenuEntry(
    label: '测试延迟 Tcping (多选)',
    actionId: 'ACT-PROF-016',
    shortcut: 'Ctrl+O',
    separatorAfter: false,
  ),
  ContextMenuEntry(
    label: '测试真连接延迟 (多选)',
    actionId: 'ACT-PROF-017',
    shortcut: 'Ctrl+R',
  ),
  ContextMenuEntry(label: '测试 UDP 延迟 (多选)', actionId: 'ACT-PROF-018'),
  ContextMenuEntry(
    label: '测试速度 (多选)',
    actionId: 'ACT-PROF-019',
    shortcut: 'Ctrl+T',
  ),
  ContextMenuEntry(label: '按测试结果排序', actionId: 'ACT-PROF-020'),
  ContextMenuEntry(
    label: '移至订阅分组',
    actionId: 'ACT-PROF-013',
    separatorAfter: false,
  ),
  ContextMenuEntry(
    label: '移至上下',
    actionId: 'ACT-PROF-009',
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
  ContextMenuEntry(label: '分享', actionId: 'ACT-PROF-006', shortcut: 'Ctrl+F'),
  ContextMenuEntry(
    label: '导出',
    actionId: 'ACT-PROF-022',
    submenu: <ContextMenuEntry>[
      ContextMenuEntry(label: '导出所选完整配置', actionId: 'ACT-PROF-022'),
      ContextMenuEntry(label: '导出所选完整配置至剪贴板', actionId: 'ACT-PROF-023'),
      ContextMenuEntry(
        label: '导出分享链接至剪贴板 (多选)',
        actionId: 'ACT-PROF-024',
        shortcut: 'Ctrl+C',
      ),
      ContextMenuEntry(
        label: '导出分享链接至剪贴板 (多选) Base64 编码',
        actionId: 'ACT-PROF-025',
      ),
      ContextMenuEntry(
        label: '导出 v2rayN 内部分享链接至剪贴板 (多选)',
        actionId: 'ACT-PROF-026',
      ),
    ],
  ),
  ContextMenuEntry(
    label: '一键生成策略组',
    actionId: 'ACT-PROF-007',
    submenu: <ContextMenuEntry>[
      ContextMenuEntry(label: '全部配置项', actionId: 'ACT-PROF-007'),
      ContextMenuEntry(label: '按地区分组', actionId: 'ACT-PROF-008'),
    ],
  ),
];
