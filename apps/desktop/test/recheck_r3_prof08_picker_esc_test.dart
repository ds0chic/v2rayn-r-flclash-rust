// R3-PROF-08: Esc cancels the picker without a result.
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;

import 'support/node_picker_harness.dart';

void main() {
  testWidgets('Esc cancels with a null result', (tester) async {
    List<String>? result;
    await pumpNodePicker(
      tester,
      candidates: <c.ProfileDto>[
        pickerNode('c1', remarks: 'HK-1', subid: 'sub-1'),
      ],
      subItems: <c.SubItemDto>[pickerSub('sub-1', 'A组')],
      onResult: (r) => result = r,
    );

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-picker')), findsNothing);
    expect(result, isNull);
  });
}
