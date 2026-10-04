// RE-PROF-11: caller ConfigType constraint with `exclude: true` (upstream
// `AddGroupServerViewModel.AddChildAsync` excludes Custom). Single page build.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';

import 'support/node_picker_harness.dart';

void main() {
  testWidgets('exclude filter hides the constrained type', (tester) async {
    await pumpNodePicker(
      tester,
      candidates: <c.ProfileDto>[
        pickerNode('c1', remarks: 'HK-1', type: ConfigType.vless),
        pickerNode('c2', remarks: 'US-1', type: ConfigType.vmess),
      ],
      filterConfigTypes: const <ConfigType>[ConfigType.vmess],
      filterExclude: true,
      onResult: (_) {},
    );
    expect(find.byKey(const ValueKey('group-pick-node-c1')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-node-c2')), findsNothing);
  });
}
