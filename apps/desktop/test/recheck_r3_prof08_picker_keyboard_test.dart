// R3-PROF-08: the picker keyboard contract - Ctrl+A selects every visible row
// and Enter confirms with the visible selection (upstream
// `ProfilesSelectWindow.LstProfiles_PreviewKeyDown` / `SelectFinish`).
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;

import 'support/node_picker_harness.dart';

void main() {
  testWidgets('Ctrl+A selects all visible and Enter confirms', (tester) async {
    List<String>? result;
    await pumpNodePicker(
      tester,
      candidates: <c.ProfileDto>[
        pickerNode('c1', remarks: 'HK-1', subid: 'sub-1'),
        pickerNode('c2', remarks: 'HK-2', subid: 'sub-1'),
      ],
      subItems: <c.SubItemDto>[pickerSub('sub-1', 'A组')],
      currentGroupSubId: 'sub-1',
      onResult: (r) => result = r,
    );

    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pump();
    expect(find.textContaining('已选 2/2'), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-picker')), findsNothing);
    expect(result, <String>['c1', 'c2']);
  });
}
