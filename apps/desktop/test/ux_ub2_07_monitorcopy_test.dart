import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 't15a_connections_test.dart' show pumpConnections;
import 't15a_logs_test.dart' show pumpLogs;

void main() {
  testWidgets('UB2-07: log toggles explain freeze vs collect', (tester) async {
    await pumpLogs(tester);

    expect(find.byTooltip('关闭后画面冻结，后台采集继续'), findsOneWidget);
    expect(find.byTooltip('只决定是否跟随到最新一行，不影响采集'), findsOneWidget);
    expect(find.byTooltip('暂停后新日志不再进入（采集真正停止），已收录的不动'), findsOneWidget);

    // Unmount while the container is alive: LogsView.dispose touches the
    // notifier, which is dead after tearDown (cf. UB2-04).
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('UB2-07: connections autofit explains restore and save', (
    tester,
  ) async {
    await pumpConnections(tester);

    expect(find.byTooltip('恢复默认列宽并保存，下次打开仍是这一套'), findsOneWidget);

    // Same unmount-before-dispose guard as above (ConnectionsView.dispose
    // also calls setPageVisible on the notifier).
    await tester.pumpWidget(const SizedBox());
  });
}
