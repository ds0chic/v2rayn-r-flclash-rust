// R3-PROF-08: a group switch prunes hidden picks, so the visible count and the
// confirmed result contain visible nodes only.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;

import 'support/node_picker_harness.dart';

void main() {
  testWidgets('group switch clears hidden picks from the confirm', (
    tester,
  ) async {
    List<String>? result;
    await pumpNodePicker(
      tester,
      candidates: <c.ProfileDto>[
        pickerNode('c1', remarks: 'HK-1', subid: 'sub-1'),
        pickerNode('c2', remarks: 'US-1', subid: 'sub-2'),
      ],
      subItems: <c.SubItemDto>[
        pickerSub('sub-1', 'A组'),
        pickerSub('sub-2', 'B组'),
      ],
      currentGroupSubId: 'sub-1',
      onResult: (r) => result = r,
    );

    await tester.tap(find.byKey(const ValueKey('group-pick-node-c1')));
    await tester.pump();
    expect(find.textContaining('已选 1/1'), findsOneWidget);

    // c1 is no longer visible after the group switch; its pick is dropped.
    await tester.tap(find.byKey(const ValueKey('group-pick-group-sub-2')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-pick-node-c1')), findsNothing);
    expect(find.textContaining('已选 0/1'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('group-pick-node-c2')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('group-pick-ok')));
    await tester.pumpAndSettle();
    expect(result, <String>['c2']);
  });
}
