// RE-PROF-11: caller ConfigType constraint with include mode
// (`SetConfigTypeFilter(types)`). Single page build.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';

import 'support/node_picker_harness.dart';

void main() {
  testWidgets('include filter keeps only the listed type', (tester) async {
    await pumpNodePicker(
      tester,
      candidates: <c.ProfileDto>[
        pickerNode('c1', remarks: 'HK-1', type: ConfigType.vless),
        pickerNode('c2', remarks: 'US-1', type: ConfigType.vmess),
      ],
      filterConfigTypes: const <ConfigType>[ConfigType.vmess],
      filterExclude: false,
      onResult: (_) {},
    );
    expect(find.byKey(const ValueKey('group-pick-node-c1')), findsNothing);
    expect(find.byKey(const ValueKey('group-pick-node-c2')), findsOneWidget);
  });
}
