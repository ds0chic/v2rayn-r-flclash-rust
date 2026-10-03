import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/context_menu.dart';

void main() {
  test('context menu entries match LAY-PROFILES-004 / ACT-PROF-*', () {
    final ids = <String>{};
    void walk(List<ContextMenuEntry> entries) {
      for (final entry in entries) {
        ids.add(entry.actionId);
        walk(entry.submenu);
      }
    }

    walk(profilesContextMenu);

    expect(
      ids.containsAll(<String>{
        'ACT-PROF-001', // edit
        'ACT-PROF-002', // remove selected
        'ACT-PROF-005', // set default / activate
        'ACT-PROF-006', // share
        'ACT-PROF-007', // generate group
        'ACT-PROF-009', // move top
        'ACT-PROF-010', // move up
        'ACT-PROF-011', // move down
        'ACT-PROF-012', // move bottom
        'ACT-PROF-016', // tcping
        'ACT-PROF-017', // real ping
        'ACT-PROF-019', // speed test
        'ACT-PROF-024', // export share url
        'ACT-PROF-032', // select all
      }),
      isTrue,
    );

    // Scope rule: select-all and moves are real UI actions; the rest are
    // backend-gated and must be flagged notImplemented.
    final moves = <ContextMenuEntry>[
      for (final entry in profilesContextMenu)
        if (entry.label == '移至上下') ...entry.submenu,
    ];
    expect(moves.length, 4);
    expect(
      moves.every((e) => e.kind != ContextActionKind.notImplemented),
      isTrue,
    );
    final selectAll = profilesContextMenu.firstWhere(
      (e) => e.actionId == 'ACT-PROF-032',
    );
    expect(selectAll.kind, ContextActionKind.selectAll);
  });

  test('restores the frozen upstream 17-root / 4-separator structure', () {
    // Frozen ProfilesView.xaml `DataGrid.ContextMenu` order, labels resolved
    // from ResUI.zh-Hans.resx (see upstream-menu-structure.json). The two
    // extra roots 快速真延迟 / 混合测试 must not reappear.
    const expectedOrder = <String>[
      '设为活动',
      '编辑',
      '克隆所选',
      '移除所选 (多选)',
      '移除重复',
      '按测试结果移除无效',
      '测试延迟 Tcping (多选)',
      '测试真连接延迟 (多选)',
      '测试 UDP 延迟 (多选)',
      '测试速度 (多选)',
      '按测试结果排序',
      '移至订阅分组',
      '移至上下',
      '全选',
      '分享',
      '导出',
      '一键生成策略组',
    ];
    expect(profilesContextMenu.length, 17);
    expect(profilesContextMenu.map((e) => e.label).toList(), expectedOrder);
    final separators = profilesContextMenu
        .where((e) => e.separatorAfter)
        .length;
    expect(separators, 4);

    // The extra roots are gone from the node table menu (their toolbar entries
    // are untouched).
    expect(profilesContextMenu.any((e) => e.label == '快速真延迟'), isFalse);
    expect(profilesContextMenu.any((e) => e.label == '混合测试 (真连接+测速)'), isFalse);

    // FIX-10 restored the real dedup action; UDP testing stays gated.
    final removeDuplicate = profilesContextMenu.firstWhere(
      (e) => e.actionId == 'ACT-PROF-003',
    );
    expect(removeDuplicate.enabled, isTrue);
    final udp = profilesContextMenu.firstWhere(
      (e) => e.actionId == 'ACT-PROF-018',
    );
    expect(udp.kind, ContextActionKind.udpTest);
    expect(udp.enabled, isFalse);

    // The labels come from the real ResUI resources, not the mojibake copy.
    for (final entry in profilesContextMenu) {
      expect(contextMenuLabels.values, contains(entry.label));
    }
  });
}
