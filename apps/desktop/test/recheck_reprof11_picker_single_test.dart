// RE-PROF-11: single-select mode hides select-all, gates OK on a choice and
// returns exactly one id. Single page build.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;

import 'support/node_picker_harness.dart';

void main() {
  testWidgets('single-select returns one row', (tester) async {
    List<String>? captured;
    await pumpNodePicker(
      tester,
      candidates: <c.ProfileDto>[
        pickerNode('c1', remarks: 'HK-1'),
        pickerNode('c2', remarks: 'US-1'),
      ],
      multiSelect: false,
      onResult: (r) => captured = r,
    );
    expect(find.byKey(const ValueKey('group-pick-select-all')), findsNothing);
    final ok = tester.widget<FilledButton>(
      find.byKey(const ValueKey('group-pick-ok')),
    );
    expect(ok.onPressed, isNull);
    await tester.tap(find.byKey(const ValueKey('group-pick-node-c2')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('group-pick-ok')));
    await tester.pumpAndSettle();
    expect(captured, <String>['c2']);
  });
}
