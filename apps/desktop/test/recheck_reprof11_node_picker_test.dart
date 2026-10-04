// RE-PROF-11: the picker renders the restored `ProfilesSelectWindow` chrome
// (group switch, search, columns, autofit) and its group/search/sort behavior
// works. A single page build keeps it clear of the locked Flutter resource
// leak. Synthetic profiles only; no native bridge, no network.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';

import 'support/node_picker_harness.dart';

void main() {
  testWidgets('renders chrome, then group/search/sort/autofit respond', (
    tester,
  ) async {
    await pumpNodePicker(
      tester,
      candidates: <c.ProfileDto>[
        pickerNode(
          'c1',
          remarks: 'HK-1',
          subid: 'sub-1',
          address: '10.0.0.1',
          port: 443,
          tls: 'tls',
        ),
        pickerNode(
          'c2',
          remarks: 'US-1',
          type: ConfigType.vmess,
          subid: 'sub-2',
          address: '10.0.0.2',
          port: 80,
          network: 'ws',
        ),
      ],
      subItems: <c.SubItemDto>[
        pickerSub('sub-1', 'A组'),
        pickerSub('sub-2', 'B组'),
      ],
      onResult: (_) {},
    );

    expect(find.byKey(const ValueKey('group-picker')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-group-all')), findsOneWidget);
    expect(
      find.byKey(const ValueKey('group-pick-group-sub-1')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey('group-pick-group-sub-2')),
      findsOneWidget,
    );
    expect(find.byKey(const ValueKey('group-pick-search')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-autofit')), findsOneWidget);
    for (final key in <String>[
      'address',
      'port',
      'network',
      'delay',
      'speed',
    ]) {
      expect(
        find.byKey(ValueKey('group-pick-sort-$key')),
        findsOneWidget,
        reason: 'missing $key column header',
      );
    }
    expect(find.textContaining('地址'), findsWidgets);
    expect(find.textContaining('延迟'), findsWidgets);
    expect(find.textContaining('速度'), findsWidgets);
    expect(find.byKey(const ValueKey('group-pick-node-c1')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-node-c2')), findsOneWidget);

    // Group switch.
    await tester.tap(find.byKey(const ValueKey('group-pick-group-sub-1')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-pick-node-c1')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-node-c2')), findsNothing);
    await tester.tap(find.byKey(const ValueKey('group-pick-group-all')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-pick-node-c2')), findsOneWidget);

    // Search commits on Enter, clears immediately.
    await tester.enterText(
      find.byKey(const ValueKey('group-pick-search')),
      'US',
    );
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-pick-node-c2')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-node-c1')), findsNothing);
    await tester.enterText(find.byKey(const ValueKey('group-pick-search')), '');
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-pick-node-c1')), findsOneWidget);

    // Header sort toggles the direction indicator.
    await tester.tap(find.byKey(const ValueKey('group-pick-sort-port')));
    await tester.pumpAndSettle();
    expect(find.textContaining('端口 ▲'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('group-pick-sort-port')));
    await tester.pumpAndSettle();
    expect(find.textContaining('端口 ▼'), findsOneWidget);

    // Auto column width runs.
    await tester.tap(find.byKey(const ValueKey('group-pick-autofit')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-picker')), findsOneWidget);
  });
}
