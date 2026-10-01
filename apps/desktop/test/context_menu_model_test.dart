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
}
