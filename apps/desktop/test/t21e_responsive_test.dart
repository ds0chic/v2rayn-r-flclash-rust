import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/main_shell.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';

import 'support/profiles_harness.dart';

/// T21-E responsive contract: the shell, its toolbars/table/status bar must lay
/// out without RenderFlex overflow at every supported logical size, and the
/// trailing toolbar/menu controls must stay reachable at the narrowest size.
///
/// One `pumpWidget` per process: the locked flutter_tester leaks native
/// resources per page build, so the size/layout matrix is driven by
/// `setSurfaceSize` + provider state instead of re-pumping the app.
void main() {
  const sizes = <Size>[
    Size(1000, 700),
    Size(1200, 800),
    Size(1600, 900),
    Size(1920, 1080),
  ];

  group('fitted column widths', () {
    test('key columns absorb spare width', () {
      final columns = defaultProfileColumns();
      final widths = fittedColumnWidths(columns, 2400, fixedChrome: 48);
      final remarks = widths[columns.indexWhere((c) => c.key == 'Remarks')];
      expect(remarks, greaterThan(150));
    });

    test('narrow viewport keeps minimums and overflows for scrolling', () {
      final columns = defaultProfileColumns();
      final widths = fittedColumnWidths(columns, 600, fixedChrome: 48);
      final index = columns.indexWhere((c) => c.key == 'Remarks');
      expect(widths[index], columns[index].minWidth);
      final total = widths.fold<double>(0, (a, b) => a + b) + 48;
      expect(total, greaterThan(600));
    });
  });

  testWidgets('shell adapts without overflow at all supported sizes', (
    tester,
  ) async {
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.binding.setSurfaceSize(sizes.first);

    final container = makeContainer(rows: 0);
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
    for (final size in sizes) {
      await tester.binding.setSurfaceSize(size);
      for (final layout in <AppLayoutMode>[
        AppLayoutMode.vertical,
        AppLayoutMode.horizontal,
        AppLayoutMode.tab,
      ]) {
        shell.setLayout(layout);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 20));
        expect(
          tester.takeException(),
          isNull,
          reason: 'overflow at ${size.width}x${size.height} layout $layout',
        );
      }
    }

    // Trailing controls must stay reachable at the narrowest supported width.
    await tester.binding.setSurfaceSize(const Size(1000, 700));
    shell.setLayout(AppLayoutMode.vertical);
    await tester.pump();
    await tester.ensureVisible(find.byKey(const ValueKey('toolbar-自动列宽')));
    await tester.pump();
    expect(
      find.byKey(const ValueKey('toolbar-自动列宽')).hitTestable(),
      findsOneWidget,
    );
    await tester.ensureVisible(find.byKey(const ValueKey('menu-关闭')));
    await tester.pump();
    expect(find.byKey(const ValueKey('menu-关闭')).hitTestable(), findsOneWidget);
  });
}
