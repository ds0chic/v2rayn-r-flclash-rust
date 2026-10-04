// R3-PROF-08: double-clicking a picker row confirms the selection
// (upstream `ProfilesSelectWindow.LstProfiles_MouseDoubleClick` ->
// `SelectFinish`).
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;

import 'support/node_picker_harness.dart';

void main() {
  testWidgets('double-click confirms the row', (tester) async {
    List<String>? result;
    await pumpNodePicker(
      tester,
      candidates: <c.ProfileDto>[
        pickerNode('c1', remarks: 'HK-1', subid: 'sub-1'),
      ],
      subItems: <c.SubItemDto>[pickerSub('sub-1', 'A组')],
      onResult: (r) => result = r,
    );

    final node = find.byKey(const ValueKey('group-pick-node-c1'));
    await tester.tap(node);
    await tester.pump(const Duration(milliseconds: 50));
    await tester.tap(node);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-picker')), findsNothing);
    expect(result, <String>['c1']);
  });
}
