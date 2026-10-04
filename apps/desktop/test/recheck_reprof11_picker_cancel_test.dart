// RE-PROF-11: cancel returns null and applies nothing. Single page build.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;

import 'support/node_picker_harness.dart';

void main() {
  testWidgets('cancel returns null', (tester) async {
    var called = false;
    List<String>? captured;
    await pumpNodePicker(
      tester,
      candidates: <c.ProfileDto>[pickerNode('c1', remarks: 'HK-1')],
      onResult: (r) {
        called = true;
        captured = r;
      },
    );
    await tester.tap(find.byKey(const ValueKey('group-pick-node-c1')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('group-pick-cancel')));
    await tester.pumpAndSettle();
    expect(called, isTrue);
    expect(captured, isNull);
  });
}
