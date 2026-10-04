// R3-VISUAL-DPI-TRAY: the main shell (toolbar, status bar, node table) must
// lay out without overflow/truncation at 100/125/150/200% DPI scaling, in all
// three layouts, and the key controls must stay reachable at the 800x600 floor.
//
// Host DPI is simulated through `tester.view.devicePixelRatio` + physical size;
// no system display setting is touched. One page build per file (locked
// flutter_tester resource leak).
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/main_shell.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';

import 'support/dpi_assertions.dart';
import 'support/profiles_harness.dart';

void main() {
  testWidgets('main shell survives the 100-200% DPI matrix', (tester) async {
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

    final shell = container.read(uiShellControllerProvider.notifier);
    for (final scale in kDpiScales) {
      for (final logical in kLogicalSizes) {
        applyDpi(tester, scale, logical);
        for (final layout in AppLayoutMode.values) {
          shell.setLayout(layout);
          await tester.pump();
          await tester.pump(const Duration(milliseconds: 20));
          expectNoLayoutProblems(
            tester,
            'main shell ${dpiLabel(scale, logical)} ${layout.name}',
            // Node-table data cells ellipsize by design; chrome is checked
            // explicitly below.
            ignoreText: (text) => true,
          );
          expectChromeNotTruncated(
            tester,
            'main shell ${dpiLabel(scale, logical)} ${layout.name}',
          );
        }
      }
    }

    // Toolbar / status-bar controls stay hittable at the logical floor.
    applyDpi(tester, 1.0, const Size(800, 600));
    shell.setLayout(AppLayoutMode.vertical);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 20));
    for (final key in <String>[
      'layout-selector',
      'theme-toggle',
      'filter-field',
      'status-proxy-speed',
      'status-inbound',
      'tun-toggle',
    ]) {
      final finder = find.byKey(ValueKey<String>(key));
      expect(finder, findsWidgets, reason: 'missing $key at 800x600');
      await tester.ensureVisible(finder.first);
      await tester.pump();
      expect(
        finder.hitTestable(),
        findsWidgets,
        reason: 'unreachable $key at 800x600',
      );
    }
  });
}
