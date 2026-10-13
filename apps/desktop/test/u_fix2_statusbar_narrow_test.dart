// U-FIX2: the status strip must not overflow below the 800px minimum window.
// r4_07's edge step (700x520) caught Row A (55px) and the Row B error-notice
// row (77px) overflowing once labels grew. Any RenderFlex overflow fails the
// test automatically; the key assertions below only pin the strip presence.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('status strip fits a 700px viewport with no overflow', (
    tester,
  ) async {
    await pumpApp(tester, rows: 20, width: 700, height: 520);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('status-badge')), findsOneWidget);
    expect(find.byKey(const ValueKey('status-proxy-speed')), findsOneWidget);
    expect(find.byKey(const ValueKey('status-today-traffic')), findsOneWidget);
  });
}
