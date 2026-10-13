// UC1-00: shared routing help bits (HelpText/HelpTip/FoldSection/LegacyRow).
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/routing/help_bits.dart';

Widget _wrap(Widget child) => MaterialApp(home: Scaffold(body: child));

void main() {
  testWidgets('HelpText renders non-empty copy with helper styling', (
    tester,
  ) async {
    await tester.pumpWidget(_wrap(const HelpText('示例说明')));
    expect(find.text('示例说明'), findsOneWidget);
  });

  testWidgets('HelpTip renders an info affordance with tooltip message', (
    tester,
  ) async {
    await tester.pumpWidget(_wrap(const HelpTip('提示内容')));
    expect(find.byTooltip('提示内容'), findsOneWidget);
  });

  testWidgets('FoldSection starts collapsed and reveals children on expand', (
    tester,
  ) async {
    await tester.pumpWidget(
      _wrap(const FoldSection(title: '专家匹配条件', child: Text('内藏项'))),
    );
    expect(find.text('专家匹配条件'), findsOneWidget);
    expect(find.text('内藏项'), findsNothing);
    await tester.tap(find.text('专家匹配条件'));
    await tester.pumpAndSettle();
    expect(find.text('内藏项'), findsOneWidget);
  });

  testWidgets('FoldSection shows badge and expands when initiallyExpanded', (
    tester,
  ) async {
    await tester.pumpWidget(
      _wrap(
        const FoldSection(
          title: '专家匹配条件',
          badge: '已设置 2 项',
          initiallyExpanded: true,
          child: Text('内藏项'),
        ),
      ),
    );
    expect(find.text('已设置 2 项'), findsOneWidget);
    expect(find.text('内藏项'), findsOneWidget);
  });

  testWidgets('LegacyRow shows read-only value with pending tag', (
    tester,
  ) async {
    await tester.pumpWidget(
      _wrap(const LegacyRow(label: '方案级域名策略', value: 'UseIP')),
    );
    expect(find.textContaining('方案级域名策略'), findsOneWidget);
    expect(find.textContaining('UseIP'), findsOneWidget);
    expect(find.textContaining('待核实'), findsOneWidget);
    expect(find.byType(TextField), findsNothing);
    expect(find.byType(DropdownButton<String>), findsNothing);
  });
}
