// B13 D1-02 residual: the node picker default order must hold untested rows
// in their store slots (stable merge, like the main table) instead of sinking
// them below every tested row.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/speedtest.dart' as speedtest;

import 'support/node_picker_harness.dart';

void main() {
  testWidgets('picker keeps untested rows in place with partial results', (
    tester,
  ) async {
    List<String>? captured;
    await pumpNodePicker(
      tester,
      candidates: <c.ProfileDto>[
        pickerNode('c1', remarks: 'n1'),
        pickerNode('c2', remarks: 'n2'),
        pickerNode('c3', remarks: 'n3'),
        pickerNode('c4', remarks: 'n4'),
        pickerNode('c5', remarks: 'n5'),
      ],
      speedResults: <speedtest.SpeedTestResultDto>[
        for (final id in <String>['c4', 'c5'])
          speedtest.SpeedTestResultDto(
            indexId: id,
            delay: 100,
            speed: 0,
            message: '',
            ipInfo: '',
          ),
      ],
      onResult: (r) => captured = r,
    );
    // Select-all + confirm returns ids in visible order.
    await tester.tap(find.byKey(const ValueKey('group-pick-select-all')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('group-pick-ok')));
    await tester.pumpAndSettle();
    expect(captured, <String>['c1', 'c2', 'c3', 'c4', 'c5']);
  });
}
