// R3-WPF-ROUTING: the routing settings window's visible structure/text follows
// the frozen WPF RoutingSettingWindow.xaml: toolbar (添加规则集 / 一键导入规则集),
// strategy labels, the 预定义规则集列表 block title, the five upstream columns
// and the absence of the non-upstream bottom action bar.
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/routing/routing_windows.dart';

import 'support/profiles_harness.dart';

Future<void> _open(WidgetTester tester) async {
  tester.view.physicalSize = const Size(1400, 1000);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final container = makeContainer();
  addTearDown(container.dispose);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: RoutingSettingWindow())),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('renders the upstream title, toolbar, block title and columns', (
    tester,
  ) async {
    await _open(tester);

    expect(
      find.byKey(const ValueKey('routing-setting-window')),
      findsOneWidget,
    );
    expect(find.text('路由设置'), findsOneWidget);

    // Upstream ToolBarTray.
    expect(find.text('添加规则集'), findsOneWidget);
    expect(find.text('一键导入规则集'), findsOneWidget);
    expect(find.byKey(const ValueKey('routing-add')), findsOneWidget);
    expect(
      find.byKey(const ValueKey('routing-import-builtin')),
      findsOneWidget,
    );

    // Upstream strategy labels.
    expect(find.text('域名解析策略'), findsOneWidget);
    expect(find.text('sing-box 域名解析策略'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('routing-domain-strategy')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey('routing-domain-strategy-sbox')),
      findsOneWidget,
    );

    // Upstream TabItem header.
    expect(find.byKey(const ValueKey('routing-block-title')), findsOneWidget);
    expect(find.text('预定义规则集列表'), findsOneWidget);

    // Upstream DataGrid columns: Remarks/Count/Sort/Url/CustomIcon.
    expect(find.text('别名'), findsOneWidget);
    expect(find.text('数量'), findsOneWidget);
    expect(find.text('排序'), findsOneWidget);
    expect(find.text('可选地址 (Url)'), findsOneWidget);
    expect(find.text('自定义图标'), findsOneWidget);

    // Old non-upstream column headers and bottom actions are gone.
    expect(find.text('备注'), findsNothing);
    expect(find.text('规则数'), findsNothing);
    expect(find.text('状态'), findsNothing);
    expect(find.byKey(const ValueKey('routing-apply')), findsNothing);
    expect(find.byKey(const ValueKey('routing-remove')), findsNothing);
    expect(find.byKey(const ValueKey('routing-set-default')), findsNothing);

    // Only 关闭 remains to dismiss the embedded dialog.
    expect(find.byKey(const ValueKey('routing-close')), findsOneWidget);
    expect(find.text('关闭'), findsOneWidget);
  });

  testWidgets('row context menu mirrors the upstream DataGrid context menu', (
    tester,
  ) async {
    await _open(tester);

    await tester.tap(
      find.text('V4-绕过大陆(Whitelist)'),
      buttons: kSecondaryButton,
    );
    await tester.pumpAndSettle();

    expect(find.text('添加规则集'), findsNWidgets(2));
    expect(find.text('移除所选规则'), findsOneWidget);
    expect(find.text('设为活动规则'), findsOneWidget);
    expect(find.text('一键导入规则集'), findsNWidgets(2));
  });
}
