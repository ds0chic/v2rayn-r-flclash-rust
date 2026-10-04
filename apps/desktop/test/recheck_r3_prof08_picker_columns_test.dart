// R3-PROF-08: the node picker joins `ProfileExItem`/speedtest results and
// subscription remarks, defaults to the main window's current group, sorts like
// the node table and searches remarks/address only. Synthetic data; no native
// bridge, no network. One page build per file (locked Flutter resource leak).
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/api/speedtest.dart' as speedtest;

import 'support/node_picker_harness.dart';

speedtest.SpeedTestResultDto _result(String id, int delay, double speed) =>
    speedtest.SpeedTestResultDto(
      indexId: id,
      delay: delay,
      speed: speed,
      message: '',
      ipInfo: '',
    );

void main() {
  testWidgets('columns join delay/speed/subremarks and default to group', (
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
        pickerNode(
          'c3',
          remarks: 'HK-2',
          subid: 'sub-1',
          address: '10.0.0.3',
          port: 8443,
        ),
        pickerNode(
          'c4',
          remarks: 'HK-3',
          subid: 'sub-1',
          address: '10.0.0.4',
          port: 9000,
        ),
      ],
      subItems: <c.SubItemDto>[
        pickerSub('sub-1', 'A组'),
        pickerSub('sub-2', 'B组'),
      ],
      currentGroupSubId: 'sub-1',
      speedResults: <speedtest.SpeedTestResultDto>[
        _result('c1', 10, 5.0),
        _result('c3', -1, 0),
      ],
      onResult: (_) {},
    );

    // Default group is the main window's current group; the sub-2 node is
    // hidden until the group is switched.
    expect(find.byKey(const ValueKey('group-pick-node-c1')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-node-c3')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-node-c4')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-node-c2')), findsNothing);

    // Real delay/speed values; failed and untested follow the main-table rule.
    expect(find.textContaining('10 ms'), findsOneWidget);
    expect(find.textContaining('5.0 MB/s'), findsOneWidget);
    expect(find.textContaining('失败'), findsOneWidget);
    // Subscription column shows SubRemarks, not the raw subid.
    expect(find.text('A组'), findsWidgets);
    expect(find.text('sub-1'), findsNothing);

    await tester.tap(find.byKey(const ValueKey('group-pick-group-sub-2')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-pick-node-c2')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-node-c1')), findsNothing);
    await tester.tap(find.byKey(const ValueKey('group-pick-group-all')));
    await tester.pumpAndSettle();

    // Header delay sort sinks failed/untested rows (never first). The wide
    // table scrolls horizontally, so bring the header into view first.
    final delayHeader = find.byKey(const ValueKey('group-pick-sort-delay'));
    await tester.ensureVisible(delayHeader);
    await tester.pumpAndSettle();
    await tester.tap(delayHeader);
    await tester.pumpAndSettle();
    double dy(String id) =>
        tester.getTopLeft(find.byKey(ValueKey('group-pick-node-$id'))).dy;
    expect(dy('c1'), lessThan(dy('c2')));
    expect(dy('c1'), lessThan(dy('c3')));
    expect(dy('c1'), lessThan(dy('c4')));
    expect(dy('c3'), lessThan(dy('c4')));

    // Search matches remarks/address only: the port never matches.
    await tester.enterText(
      find.byKey(const ValueKey('group-pick-search')),
      '8443',
    );
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-pick-empty')), findsOneWidget);
    await tester.enterText(
      find.byKey(const ValueKey('group-pick-search')),
      '10.0.0.2',
    );
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-pick-node-c2')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-node-c1')), findsNothing);
  });
}
