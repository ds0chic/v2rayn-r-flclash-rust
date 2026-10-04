// RE-PROF-11: multi-select mode returns the picked ids in candidate order.
// Single page build.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;

import 'support/node_picker_harness.dart';

void main() {
  testWidgets('select-all confirms in candidate order', (tester) async {
    List<String>? captured;
    await pumpNodePicker(
      tester,
      candidates: <c.ProfileDto>[
        pickerNode('c1', remarks: 'HK-1'),
        pickerNode('c2', remarks: 'US-1'),
      ],
      onResult: (r) => captured = r,
    );
    await tester.tap(find.byKey(const ValueKey('group-pick-select-all')));
    await tester.pumpAndSettle();
    expect(find.textContaining('已选 2/2'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('group-pick-ok')));
    await tester.pumpAndSettle();
    expect(captured, <String>['c1', 'c2']);
  });
}
