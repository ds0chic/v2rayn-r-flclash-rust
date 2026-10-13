// D-LEFT: the http subscription warning is a persistent inline banner, not a
// one-shot snackbar. Non-blocking: saving still works.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/features/subs/sub_edit_window.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

import 'support/subs_harness.dart';

const _warnKey = ValueKey('sub-http-warn');

c.SubItemDto _draftWithUrl(String url) => c.SubItemDto(
  id: '',
  remarks: 'r',
  url: url,
  moreUrl: '',
  enabled: true,
  userAgent: '',
  sort: 0,
  autoUpdateInterval: 0,
  updateTime: 0,
);

Future<void> _pumpEditor(
  WidgetTester tester,
  ProviderContainer container,
  String url, {
  required double width,
  required Brightness brightness,
}) async {
  tester.view.physicalSize = Size(width, 900);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(
        theme: brightness == Brightness.dark
            ? ThemeData.dark()
            : ThemeData.light(),
        home: Scaffold(body: SubEditWindow(initial: _draftWithUrl(url))),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

Text _bannerText(WidgetTester tester) => tester.widget<Text>(
  find.descendant(of: find.byKey(_warnKey), matching: find.byType(Text)),
);

void main() {
  testWidgets('public http URL shows the banner at 800 wide, light', (
    tester,
  ) async {
    final container = makeSubsContainer();
    addTearDown(container.dispose);
    await _pumpEditor(
      tester,
      container,
      'http://8.8.8.8/x',
      width: 800,
      brightness: Brightness.light,
    );
    expect(find.byKey(_warnKey), findsOneWidget);
    expect(tester.takeException(), isNull);
    final ctx = tester.element(find.byKey(_warnKey));
    expect(_bannerText(tester).style!.color, ctx.semantics.warning);
  });

  testWidgets('public http URL shows the banner at 1024 wide, dark', (
    tester,
  ) async {
    final container = makeSubsContainer();
    addTearDown(container.dispose);
    await _pumpEditor(
      tester,
      container,
      'http://8.8.8.8/x',
      width: 1024,
      brightness: Brightness.dark,
    );
    expect(find.byKey(_warnKey), findsOneWidget);
    expect(tester.takeException(), isNull);
    final ctx = tester.element(find.byKey(_warnKey));
    expect(_bannerText(tester).style!.color, ctx.semantics.warning);
  });

  testWidgets('private, https and empty URLs show no banner', (tester) async {
    for (final url in <String>[
      'http://192.168.1.1/x',
      'http://127.0.0.1:11808/x',
      'https://8.8.8.8/x',
      '',
    ]) {
      final container = makeSubsContainer();
      addTearDown(container.dispose);
      await _pumpEditor(
        tester,
        container,
        url,
        width: 800,
        brightness: Brightness.light,
      );
      expect(find.byKey(_warnKey), findsNothing, reason: url);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(Container());
    }
  });
}
