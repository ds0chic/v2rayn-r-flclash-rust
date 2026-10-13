import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/main_shell.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';

import 'support/dpi_assertions.dart';
import 'support/profiles_harness.dart';

/// U-FIX (54px overflow family): the status bar must fit 1280/1024/800 logical
/// widths with no RenderFlex overflow. At 800 the captioned selectors shrink
/// (captions ellipsize, faces keep full text under tooltips); chrome text
/// never truncates.
void main() {
  testWidgets('status bar fits 1280/1024/800 without overflow', (tester) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final container = makeContainer(rows: 6);
    addTearDown(container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: MainShell()),
      ),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 20));
    container
        .read(uiShellControllerProvider.notifier)
        .setLayout(AppLayoutMode.horizontal);
    for (final width in <double>[1280, 1024, 800]) {
      applyDpi(tester, 1.0, Size(width, 800));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 20));
      expectNoLayoutProblems(
        tester,
        'status bar ${width.toInt()}px',
        ignoreText: (text) => true,
      );
      expectChromeNotTruncated(tester, 'status bar ${width.toInt()}px');
    }
    // The group captions stay in the tree at every width.
    expect(find.byKey(const ValueKey('label-sysproxy')), findsOneWidget);
    expect(find.byKey(const ValueKey('label-rule-mode')), findsOneWidget);
    expect(find.byKey(const ValueKey('label-routing-scheme')), findsOneWidget);
  });
}
